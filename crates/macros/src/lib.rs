//! Proc-macro attribute for [`devirt`](https://docs.rs/devirt).
//!
//! Provides `#[devirt]` as a proc-macro attribute that emits the
//! devirtualization dispatch code directly via `quote!`. This crate
//! is an implementation detail of `devirt` and should not be used
//! directly.

use std::collections::HashSet;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::visit_mut::VisitMut;
use syn::{Token, parse_macro_input};

/// Proc-macro attribute for transparent devirtualization.
///
/// # On a trait definition
///
/// ```ignore
/// #[devirt::devirt(Circle, Rect)]
/// pub trait Shape: Debug {
///     fn area(&self) -> f64;
///     fn scale(&mut self, factor: f64);
/// }
/// ```
///
/// # On an impl block
///
/// ```ignore
/// #[devirt::devirt]
/// impl Shape for Circle {
///     fn area(&self) -> f64 { PI * self.radius * self.radius }
///     fn scale(&mut self, factor: f64) { self.radius *= factor; }
/// }
/// ```
#[proc_macro_attribute]
pub fn devirt(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Try parsing as a trait first, then as an impl.
    if let Ok(trait_item) = syn::parse::<syn::ItemTrait>(item.clone()) {
        return expand_trait(attr, &trait_item);
    }
    if let Ok(impl_item) = syn::parse::<syn::ItemImpl>(item) {
        return expand_impl(&attr, &impl_item);
    }
    syn::Error::new(
        proc_macro2::Span::call_site(),
        "#[devirt] can only be applied to trait definitions or impl blocks",
    )
    .to_compile_error()
    .into()
}

// ── Trait expansion ─────────────────────────────────────────────────────────

fn expand_trait(attr: TokenStream, trait_item: &syn::ItemTrait) -> TokenStream {
    if attr.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "expected hot types: #[devirt(Type1, Type2)]",
        )
        .to_compile_error()
        .into();
    }

    if let Err(e) = validate_trait(trait_item) {
        return e.to_compile_error().into();
    }

    let hot_types: Vec<syn::Type> =
        parse_macro_input!(attr with Punctuated::<syn::Type, Token![,]>::parse_terminated)
            .into_iter()
            .collect();

    emit_trait_expansion(trait_item, &hot_types)
}

fn validate_trait(trait_item: &syn::ItemTrait) -> Result<(), syn::Error> {
    if !trait_item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &trait_item.generics,
            "#[devirt] does not support generic traits",
        ));
    }
    if let Some(wc) = &trait_item.generics.where_clause {
        return Err(syn::Error::new_spanned(
            wc,
            "#[devirt] does not support where clauses on traits",
        ));
    }
    for item in &trait_item.items {
        match item {
            syn::TraitItem::Type(t) => {
                return Err(syn::Error::new_spanned(
                    t,
                    "#[devirt] does not support associated types",
                ));
            }
            syn::TraitItem::Const(c) => {
                return Err(syn::Error::new_spanned(
                    c,
                    "#[devirt] does not support associated constants",
                ));
            }
            syn::TraitItem::Fn(f) => validate_trait_method(f)?,
            _ => {}
        }
    }
    Ok(())
}

fn validate_trait_method(f: &syn::TraitItemFn) -> Result<(), syn::Error> {
    if f.sig.asyncness.is_some() {
        return Err(syn::Error::new_spanned(
            &f.sig,
            "#[devirt] does not support async methods",
        ));
    }
    let Some(recv) = f.sig.inputs.first().and_then(|a| {
        if let syn::FnArg::Receiver(r) = a {
            Some(r)
        } else {
            None
        }
    }) else {
        return Err(syn::Error::new_spanned(
            &f.sig,
            "#[devirt] methods must have a `&self` or `&mut self` receiver",
        ));
    };
    if recv.reference.is_none() {
        return Err(syn::Error::new_spanned(
            recv,
            "#[devirt] does not support owned self or custom self types; \
             use `&self` or `&mut self`",
        ));
    }
    // Validate argument patterns are named (ident or wildcard).
    for arg in &f.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg
            && !matches!(&*pat_type.pat, syn::Pat::Ident(_) | syn::Pat::Wild(_))
        {
            return Err(syn::Error::new_spanned(
                &pat_type.pat,
                "#[devirt] requires named parameters (use `name: Type` \
                 instead of a destructuring pattern)",
            ));
        }
    }
    Ok(())
}

fn emit_trait_expansion(
    trait_item: &syn::ItemTrait,
    hot_types: &[syn::Type],
) -> TokenStream {
    let unsafety = &trait_item.unsafety;
    let vis = &trait_item.vis;
    let name = &trait_item.ident;
    let outer_attrs = &trait_item.attrs;
    let supertraits = &trait_item.supertraits;
    let inner_name = format_ident!("__{name}Impl");

    // __spec_* method declarations for the inner trait (with default
    // bodies rewritten so `self.method()` → `self.__spec_method()`).
    let spec_decls = generate_spec_decls(trait_item);

    // Dispatch methods for the inherent impl on `dyn Trait`.
    let dispatch_methods: Vec<_> = trait_item
        .items
        .iter()
        .filter_map(|item| {
            let syn::TraitItem::Fn(m) = item else {
                return None;
            };
            Some(generate_dispatch_method(m, name, &inner_name, hot_types))
        })
        .collect();

    // Delegating methods for auto-trait inherent impls (Send, Sync,
    // Send + Sync).  Each method coerces `self` to the base `dyn Trait`
    // and calls the dispatch method.
    let delegating_methods: Vec<_> = trait_item
        .items
        .iter()
        .filter_map(|item| {
            let syn::TraitItem::Fn(m) = item else {
                return None;
            };
            Some(generate_delegating_method(m, name))
        })
        .collect();

    // Inner trait supertraits: `__FooImpl: Debug + Clone`
    let inner_supers = if supertraits.is_empty() {
        quote! {}
    } else {
        quote! { : #supertraits }
    };

    // Public trait supertraits: `Foo: __FooImpl + Debug + Clone`
    // The `+ Debug + Clone` is redundant (implied by `__FooImpl`) but
    // makes the bounds visible in rustdoc and compiler diagnostics.
    let public_supers = if supertraits.is_empty() {
        quote! { #inner_name }
    } else {
        quote! { #inner_name + #supertraits }
    };

    quote! {
        // (1) Hidden inner trait — carries __spec_* methods.
        #[doc(hidden)]
        #vis #unsafety trait #inner_name #inner_supers {
            #(#spec_decls)*
        }

        // (2) Compile-time fat pointer assertion.
        const _: () = assert!(
            ::core::mem::size_of::<*const dyn #name>()
                == 2 * ::core::mem::size_of::<usize>()
        );

        // (3) Vtable helpers on `dyn Trait`.
        impl<'__devirt> dyn #name + '__devirt {
            /// Split a fat pointer into `[data, vtable]`.
            #[doc(hidden)]
            #[inline(always)]
            pub fn __devirt_raw_parts(this: &Self) -> [usize; 2] {
                // SAFETY: `&dyn Trait` is a two-`usize` fat pointer
                // (verified by the compile-time assertion above).
                unsafe { ::core::mem::transmute::<&Self, [usize; 2]>(this) }
            }

            /// Vtable pointer for the `(T, Trait)` pair.
            #[doc(hidden)]
            #[inline(always)]
            pub fn __devirt_vtable_for<
                __DevirtT: #inner_name + 'static,
            >() -> usize {
                let fake: *const __DevirtT =
                    ::core::ptr::without_provenance(
                        ::core::mem::align_of::<__DevirtT>(),
                    );
                let fat: *const Self = fake;
                // SAFETY: `*const dyn Trait` is two `usize`s. We read
                // only the vtable half; the dangling data half is
                // discarded.
                let __parts: [usize; 2] = unsafe {
                    ::core::mem::transmute::<*const Self, [usize; 2]>(fat)
                };
                __parts[1]
            }
        }

        // (4) Inherent dispatch methods.
        impl<'__devirt> dyn #name + '__devirt {
            #(#dispatch_methods)*
        }

        // (4a) dyn Trait + Send — delegate to base dispatch.
        impl<'__devirt> dyn #name + ::core::marker::Send + '__devirt {
            #(#delegating_methods)*
        }

        // (4b) dyn Trait + Sync — delegate to base dispatch.
        impl<'__devirt> dyn #name + ::core::marker::Sync + '__devirt {
            #(#delegating_methods)*
        }

        // (4c) dyn Trait + Send + Sync — delegate to base dispatch.
        impl<'__devirt> dyn #name + ::core::marker::Send + ::core::marker::Sync + '__devirt {
            #(#delegating_methods)*
        }

        // (5) Public marker trait.
        #(#outer_attrs)*
        #vis #unsafety trait #name: #public_supers {}

        // (6) Blanket impl.
        #unsafety impl<__DevirtT: #inner_name + ?Sized> #name
            for __DevirtT {}
    }
    .into()
}

// ── Default-body spec declarations ─────────────────────────────────────────

/// Generate `__spec_*` method declarations for the inner trait.
///
/// Methods without a default body become required (`__spec_foo(...);`).
/// Methods with a default body get the body rewritten so that
/// `self.method()` calls become `self.__spec_method()`, then emitted
/// as provided methods on the inner trait.
fn generate_spec_decls(
    trait_item: &syn::ItemTrait,
) -> Vec<proc_macro2::TokenStream> {
    let method_names: HashSet<String> = trait_item
        .items
        .iter()
        .filter_map(|item| {
            if let syn::TraitItem::Fn(m) = item {
                Some(m.sig.ident.to_string())
            } else {
                None
            }
        })
        .collect();

    trait_item
        .items
        .iter()
        .filter_map(|item| {
            let syn::TraitItem::Fn(m) = item else {
                return None;
            };
            let mut spec_sig = m.sig.clone();
            spec_sig.ident = format_ident!("__spec_{}", spec_sig.ident);
            let attrs = &m.attrs;

            m.default.as_ref().map_or_else(
                || Some(quote! { #(#attrs)* #spec_sig; }),
                |default_body| {
                    let mut body = default_body.clone();
                    let mut rewriter = RewriteSelfCalls {
                        method_names: method_names.clone(),
                    };
                    rewriter.visit_block_mut(&mut body);
                    Some(quote! { #(#attrs)* #spec_sig #body })
                },
            )
        })
        .collect()
}

// ── Shared helpers ──────────────────────────────────────────────────────────

/// Clone a method signature, replacing wildcard `_` patterns with
/// generated names so arguments can be forwarded.  Returns the
/// rewritten signature and a list of argument identifiers (excluding
/// `self`).
fn rewrite_sig_with_named_args(
    sig: &syn::Signature,
) -> (syn::Signature, Vec<syn::Ident>) {
    let mut sig = sig.clone();
    let mut arg_names = Vec::new();
    for (idx, arg) in sig.inputs.iter_mut().enumerate() {
        if let syn::FnArg::Typed(pat_type) = arg {
            match &*pat_type.pat {
                syn::Pat::Ident(pat_ident) => {
                    arg_names.push(pat_ident.ident.clone());
                }
                syn::Pat::Wild(_) => {
                    let generated = format_ident!("__devirt_arg{idx}");
                    *pat_type.pat = syn::Pat::Ident(syn::PatIdent {
                        attrs: vec![],
                        by_ref: None,
                        mutability: None,
                        ident: generated.clone(),
                        subpat: None,
                    });
                    arg_names.push(generated);
                }
                _ => {
                    // Validation already rejects this case, but
                    // generate a name defensively.
                    let generated = format_ident!("__devirt_arg{idx}");
                    *pat_type.pat = syn::Pat::Ident(syn::PatIdent {
                        attrs: vec![],
                        by_ref: None,
                        mutability: None,
                        ident: generated.clone(),
                        subpat: None,
                    });
                    arg_names.push(generated);
                }
            }
        }
    }
    (sig, arg_names)
}

// ── Dispatch method generation ──────────────────────────────────────────────

fn generate_dispatch_method(
    method: &syn::TraitItemFn,
    trait_name: &syn::Ident,
    inner_name: &syn::Ident,
    hot_types: &[syn::Type],
) -> proc_macro2::TokenStream {
    let sig = &method.sig;
    let attrs = &method.attrs;
    let spec_name = format_ident!("__spec_{}", sig.ident);

    let receiver = sig
        .inputs
        .first()
        .and_then(|a| {
            if let syn::FnArg::Receiver(r) = a {
                Some(r)
            } else {
                None
            }
        })
        .expect("validated: method has receiver");

    let is_mut = receiver.mutability.is_some();
    let is_unsafe = sig.unsafety.is_some();

    let (dispatch_sig, arg_names) = rewrite_sig_with_named_args(sig);

    let raw_parts = if is_mut {
        quote! { let __raw = <dyn #trait_name>::__devirt_raw_parts(&*self); }
    } else {
        quote! { let __raw = <dyn #trait_name>::__devirt_raw_parts(self); }
    };

    let hot_checks = gen_hot_checks(
        hot_types, trait_name, &spec_name, &arg_names, is_mut,
    );

    let fallback = if is_unsafe {
        quote! { unsafe { #inner_name::#spec_name(self, #(#arg_names),*) } }
    } else {
        quote! { #inner_name::#spec_name(self, #(#arg_names),*) }
    };

    quote! {
        #(#attrs)*
        #[doc(hidden)]
        #[inline]
        pub #dispatch_sig {
            #raw_parts
            #(#hot_checks)*
            #fallback
        }
    }
}

fn gen_hot_checks(
    hot_types: &[syn::Type],
    trait_name: &syn::Ident,
    spec_name: &syn::Ident,
    arg_names: &[syn::Ident],
    is_mut: bool,
) -> Vec<proc_macro2::TokenStream> {
    hot_types
        .iter()
        .map(|hot| {
            if is_mut {
                quote! {
                    if __raw[1]
                        == <dyn #trait_name>::__devirt_vtable_for::<#hot>()
                    {
                        let __p: *mut #hot = __raw[0] as *mut #hot;
                        // SAFETY: vtable identity implies type identity.
                        // The `&mut` reborrow is scoped to this method
                        // call and released before the enclosing `&mut
                        // dyn Trait` is used again.
                        return unsafe {
                            (&mut *__p).#spec_name(#(#arg_names),*)
                        };
                    }
                }
            } else {
                quote! {
                    if __raw[1]
                        == <dyn #trait_name>::__devirt_vtable_for::<#hot>()
                    {
                        let __p: *const #hot = __raw[0] as *const #hot;
                        // SAFETY: vtable identity implies type identity.
                        // The data half is the original `&HotType` the
                        // caller coerced into the fat pointer.
                        return unsafe {
                            (&*__p).#spec_name(#(#arg_names),*)
                        };
                    }
                }
            }
        })
        .collect()
}

// ── Delegating method generation (auto-trait impls) ─────────────────────────

/// Generate a delegating shim that coerces `&(dyn Trait + Send)` (or
/// `Sync`, `Send + Sync`) to `&dyn Trait` and calls the base dispatch
/// method.  Auto traits do not change the vtable layout, so the
/// coercion is zero-cost and LLVM eliminates the delegation entirely
/// with `#[inline(always)]`.
fn generate_delegating_method(
    method: &syn::TraitItemFn,
    trait_name: &syn::Ident,
) -> proc_macro2::TokenStream {
    let sig = &method.sig;
    let method_name = &sig.ident;
    let attrs = &method.attrs;

    let receiver = sig
        .inputs
        .first()
        .and_then(|a| {
            if let syn::FnArg::Receiver(r) = a {
                Some(r)
            } else {
                None
            }
        })
        .expect("validated: method has receiver");

    let is_mut = receiver.mutability.is_some();
    let is_unsafe = sig.unsafety.is_some();
    let (dispatch_sig, arg_names) = rewrite_sig_with_named_args(sig);

    // Coerce to the base `dyn Trait` and call the dispatch method.
    let coerce_and_call = if is_mut {
        quote! {
            let __devirt_base: &mut (dyn #trait_name + '__devirt) = self;
            __devirt_base.#method_name(#(#arg_names),*)
        }
    } else {
        quote! {
            let __devirt_base: &(dyn #trait_name + '__devirt) = self;
            __devirt_base.#method_name(#(#arg_names),*)
        }
    };

    // When the method is `unsafe fn`, the base dispatch method is also
    // `unsafe fn`, so the call must be inside an `unsafe` block.
    let delegation = if is_unsafe {
        quote! { unsafe { #coerce_and_call } }
    } else {
        coerce_and_call
    };

    quote! {
        #(#attrs)*
        #[doc(hidden)]
        #[inline(always)]
        pub #dispatch_sig {
            #delegation
        }
    }
}

// ── Impl expansion ──────────────────────────────────────────────────────────

fn expand_impl(attr: &TokenStream, impl_item: &syn::ItemImpl) -> TokenStream {
    if !attr.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "hot types are specified on the trait definition, not the impl block",
        )
        .to_compile_error()
        .into();
    }

    let Some((_, trait_path, _)) = &impl_item.trait_ else {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[devirt] requires `impl Trait for Type`, not a bare impl block",
        )
        .to_compile_error()
        .into();
    };

    if !impl_item.generics.params.is_empty() {
        return syn::Error::new_spanned(
            &impl_item.generics,
            "#[devirt] does not support generic impl blocks",
        )
        .to_compile_error()
        .into();
    }
    if let Some(wc) = &impl_item.generics.where_clause {
        return syn::Error::new_spanned(
            wc,
            "#[devirt] does not support where clauses on impl blocks",
        )
        .to_compile_error()
        .into();
    }

    // Reject qualified paths — we need a plain ident to construct
    // the __TraitNameImpl identifier.
    if trait_path.leading_colon.is_some() || trait_path.segments.len() > 1 {
        return syn::Error::new_spanned(
            trait_path,
            "#[devirt] requires a plain trait name, not a qualified path \
             (e.g., `impl MyTrait for T`, not `impl super::MyTrait for T`)",
        )
        .to_compile_error()
        .into();
    }

    let unsafety = &impl_item.unsafety;
    let trait_name = &trait_path
        .segments
        .last()
        .expect("validated: path non-empty")
        .ident;
    let inner_name = format_ident!("__{trait_name}Impl");
    let ty = &impl_item.self_ty;

    let spec_methods: Vec<_> = impl_item
        .items
        .iter()
        .filter_map(|item| {
            let syn::ImplItem::Fn(m) = item else {
                return None;
            };
            let mut spec_sig = m.sig.clone();
            spec_sig.ident = format_ident!("__spec_{}", spec_sig.ident);
            let attrs = &m.attrs;
            let block = &m.block;
            Some(quote! {
                #(#attrs)*
                #[inline]
                #[allow(clippy::unnecessary_literal_bound)]
                #spec_sig #block
            })
        })
        .collect();

    quote! {
        #unsafety impl #inner_name for #ty {
            #(#spec_methods)*
        }
    }
    .into()
}

// ── AST rewriting for default method bodies ────────────────────────────────

/// Rewrites `self.method()` calls in default method bodies to
/// `self.__spec_method()` so they resolve within the inner trait.
struct RewriteSelfCalls {
    /// Original method names defined on this trait.
    method_names: HashSet<String>,
}

impl VisitMut for RewriteSelfCalls {
    fn visit_expr_method_call_mut(&mut self, i: &mut syn::ExprMethodCall) {
        // Recurse into sub-expressions first.
        syn::visit_mut::visit_expr_method_call_mut(self, i);

        // Only rewrite direct `self.method()` calls.
        if is_self_expr(&i.receiver) {
            let name = i.method.to_string();
            if self.method_names.contains(&name) {
                i.method = format_ident!("__spec_{}", i.method);
            }
        }
    }

    fn visit_macro_mut(&mut self, i: &mut syn::Macro) {
        // `syn::visit_mut` does not descend into macro token streams,
        // so we do a token-level rewrite for `self.method()` patterns
        // inside macro invocations (e.g. `format!`, `write!`).
        i.tokens =
            rewrite_self_calls_in_tokens(&self.method_names, i.tokens.clone());
    }
}

fn is_self_expr(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Path(p) => p.path.is_ident("self"),
        // Handle `(self)` — parenthesized.
        syn::Expr::Paren(p) => is_self_expr(&p.expr),
        _ => false,
    }
}

/// Token-level rewrite of `self . method_name` → `self . __spec_method_name`
/// inside macro invocations, where `syn::visit_mut` cannot descend.
fn rewrite_self_calls_in_tokens(
    method_names: &HashSet<String>,
    tokens: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let tts: Vec<proc_macro2::TokenTree> = tokens.into_iter().collect();
    let mut out = Vec::with_capacity(tts.len());
    let mut i = 0;
    while i < tts.len() {
        // Match pattern: Ident("self")  Punct('.')  Ident(method_name)
        if i + 2 < tts.len()
            && let proc_macro2::TokenTree::Ident(ref id) = tts[i]
            && *id == "self"
            && let proc_macro2::TokenTree::Punct(ref dot) = tts[i + 1]
            && dot.as_char() == '.'
            && let proc_macro2::TokenTree::Ident(ref method) = tts[i + 2]
            && method_names.contains(&method.to_string())
        {
            out.push(tts[i].clone());
            out.push(tts[i + 1].clone());
            out.push(proc_macro2::TokenTree::Ident(
                proc_macro2::Ident::new(
                    &format!("__spec_{method}"),
                    method.span(),
                ),
            ));
            i += 3;
            continue;
        }
        // Recurse into groups (parenthesized, braced, bracketed).
        if let proc_macro2::TokenTree::Group(ref g) = tts[i] {
            let inner =
                rewrite_self_calls_in_tokens(method_names, g.stream());
            let mut ng = proc_macro2::Group::new(g.delimiter(), inner);
            ng.set_span(g.span());
            out.push(proc_macro2::TokenTree::Group(ng));
        } else {
            out.push(tts[i].clone());
        }
        i += 1;
    }
    out.into_iter().collect()
}
