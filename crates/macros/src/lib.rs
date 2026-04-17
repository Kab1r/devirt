//! Proc-macro attribute for [`devirt`](https://docs.rs/devirt).
//!
//! Provides `#[devirt]` as a proc-macro attribute that emits the
//! devirtualization dispatch code directly via `quote!`. This crate
//! is an implementation detail of `devirt` and should not be used
//! directly.

use std::collections::{HashMap, HashSet};

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::visit::Visit;
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
    for item in &trait_item.items {
        match item {
            syn::TraitItem::Const(c) => {
                return Err(syn::Error::new_spanned(
                    c,
                    "#[devirt] does not support associated constants — \
                     they make a trait not dyn-compatible",
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

struct AssocTypeInfo {
    names: HashSet<String>,
    idents: Vec<syn::Ident>,
    generics: Vec<syn::Ident>,
    decls: Vec<proc_macro2::TokenStream>,
    rewrites: HashMap<String, syn::Ident>,
}

fn collect_assoc_types(trait_item: &syn::ItemTrait) -> AssocTypeInfo {
    let items: Vec<&syn::TraitItemType> = trait_item
        .items
        .iter()
        .filter_map(|item| {
            if let syn::TraitItem::Type(t) = item {
                Some(t)
            } else {
                None
            }
        })
        .collect();
    let names = items.iter().map(|t| t.ident.to_string()).collect();
    let idents: Vec<syn::Ident> = items.iter().map(|t| t.ident.clone()).collect();
    let generics: Vec<syn::Ident> =
        idents.iter().map(|id| format_ident!("__{id}")).collect();
    let decls = items
        .iter()
        .map(|t| {
            let t = *t;
            quote! { #t }
        })
        .collect();
    let rewrites = idents
        .iter()
        .zip(generics.iter())
        .map(|(id, gp)| (id.to_string(), gp.clone()))
        .collect();
    AssocTypeInfo { names, idents, generics, decls, rewrites }
}

fn build_trait_dyn_ref(
    name: &syn::Ident,
    trait_generic_params: &Punctuated<syn::GenericParam, Token![,]>,
    assoc_type_idents: &[syn::Ident],
    assoc_type_generics: &[syn::Ident],
) -> proc_macro2::TokenStream {
    let mut type_args: Vec<proc_macro2::TokenStream> = Vec::new();
    for param in trait_generic_params {
        match param {
            syn::GenericParam::Type(t) => {
                let id = &t.ident;
                type_args.push(quote! { #id });
            }
            syn::GenericParam::Lifetime(l) => {
                let lt = &l.lifetime;
                type_args.push(quote! { #lt });
            }
            syn::GenericParam::Const(c) => {
                let id = &c.ident;
                type_args.push(quote! { #id });
            }
        }
    }
    for (id, gp) in assoc_type_idents.iter().zip(assoc_type_generics.iter()) {
        type_args.push(quote! { #id = #gp });
    }
    if type_args.is_empty() {
        quote! { #name }
    } else {
        quote! { #name<#(#type_args),*> }
    }
}

fn build_fat_ptr_assertion(
    trait_item: &syn::ItemTrait,
) -> proc_macro2::TokenStream {
    let name = &trait_item.ident;
    let params = &trait_item.generics.params;

    let assoc_types: Vec<&syn::TraitItemType> = trait_item
        .items
        .iter()
        .filter_map(|item| {
            if let syn::TraitItem::Type(t) = item {
                Some(t)
            } else {
                None
            }
        })
        .collect();

    if params.is_empty() && assoc_types.is_empty() {
        return quote! {
            const _: () = assert!(
                ::core::mem::size_of::<*const dyn #name>()
                    == 2 * ::core::mem::size_of::<usize>()
            );
        };
    }

    let mut fn_params: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut dyn_args: Vec<proc_macro2::TokenStream> = Vec::new();

    for param in params {
        fn_params.push(strip_param_defaults(param));
        match param {
            syn::GenericParam::Type(t) => {
                let id = &t.ident;
                dyn_args.push(quote! { #id });
            }
            syn::GenericParam::Lifetime(l) => {
                let lt = &l.lifetime;
                dyn_args.push(quote! { #lt });
            }
            syn::GenericParam::Const(c) => {
                let id = &c.ident;
                dyn_args.push(quote! { #id });
            }
        }
    }

    for assoc in &assoc_types {
        let id = &assoc.ident;
        let assoc_param = format_ident!("__Assoc{}", id);
        let bounds = &assoc.bounds;
        if bounds.is_empty() {
            fn_params.push(quote! { #assoc_param });
        } else {
            fn_params.push(quote! { #assoc_param: #bounds });
        }
        dyn_args.push(quote! { #id = #assoc_param });
    }

    let where_preds: Vec<_> = trait_item
        .generics
        .where_clause
        .as_ref()
        .map(|wc| {
            wc.predicates
                .iter()
                .filter(|pred| !predicate_references_self(pred))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let where_clause = if where_preds.is_empty() {
        quote! {}
    } else {
        quote! { where #(#where_preds),* }
    };

    quote! {
        const _: () = {
            fn __devirt_assert<#(#fn_params),*>() #where_clause {
                assert!(
                    ::core::mem::size_of::<*const dyn #name<#(#dyn_args),*>>()
                        == 2 * ::core::mem::size_of::<usize>()
                );
            }
        };
    }
}

fn strip_param_defaults(param: &syn::GenericParam) -> proc_macro2::TokenStream {
    match param {
        syn::GenericParam::Type(t) => {
            let id = &t.ident;
            let bounds = &t.bounds;
            if bounds.is_empty() {
                quote! { #id }
            } else {
                quote! { #id: #bounds }
            }
        }
        syn::GenericParam::Lifetime(l) => quote! { #l },
        syn::GenericParam::Const(c) => {
            let id = &c.ident;
            let ty = &c.ty;
            quote! { const #id: #ty }
        }
    }
}

fn predicate_references_self(pred: &syn::WherePredicate) -> bool {
    if let syn::WherePredicate::Type(pt) = pred
        && let syn::Type::Path(tp) = &pt.bounded_ty
        && let Some(first) = tp.path.segments.first()
    {
        return first.ident == "Self";
    }
    false
}

fn build_vtable_helpers(
    can_devirt: bool,
    name: &syn::Ident,
    inner_name: &syn::Ident,
    inherent_impl_generics: &proc_macro2::TokenStream,
    trait_dyn_ref: &proc_macro2::TokenStream,
    assoc_type_idents: &[syn::Ident],
) -> proc_macro2::TokenStream {
    if !can_devirt {
        return quote! {};
    }
    let vtable_coerce_ty = if assoc_type_idents.is_empty() {
        quote! { *const Self }
    } else {
        quote! {
            *const dyn #name<
                #(#assoc_type_idents =
                    <__DevirtT as #inner_name>::#assoc_type_idents),*
            >
        }
    };
    quote! {
        impl #inherent_impl_generics dyn #trait_dyn_ref + '__devirt {
            #[doc(hidden)]
            #[inline(always)]
            pub fn __devirt_raw_parts(this: &Self) -> [usize; 2] {
                unsafe {
                    ::core::mem::transmute::<&Self, [usize; 2]>(this)
                }
            }

            #[doc(hidden)]
            #[inline(always)]
            pub fn __devirt_vtable_for<
                __DevirtT: #inner_name + 'static,
            >() -> usize {
                let fake: *const __DevirtT =
                    ::core::ptr::without_provenance(
                        ::core::mem::align_of::<__DevirtT>(),
                    );
                let fat: #vtable_coerce_ty = fake;
                let __parts: [usize; 2] = unsafe {
                    ::core::mem::transmute::<
                        #vtable_coerce_ty, [usize; 2]
                    >(fat)
                };
                __parts[1]
            }
        }
    }
}

fn build_blanket_impl(
    unsafety: Option<&syn::token::Unsafe>,
    has_trait_generics: bool,
    name: &syn::Ident,
    inner_name: &syn::Ident,
    trait_generic_params: &Punctuated<syn::GenericParam, Token![,]>,
    trait_ty_generics: &syn::TypeGenerics<'_>,
    trait_where_clause: Option<&syn::WhereClause>,
) -> proc_macro2::TokenStream {
    if has_trait_generics {
        quote! {
            #unsafety impl<
                __DevirtT: #inner_name #trait_ty_generics + ?Sized,
                #trait_generic_params
            > #name #trait_ty_generics for __DevirtT #trait_where_clause {}
        }
    } else {
        quote! {
            #unsafety impl<__DevirtT: #inner_name + ?Sized> #name
                for __DevirtT #trait_where_clause {}
        }
    }
}

fn build_dispatch_methods(
    trait_item: &syn::ItemTrait,
    can_devirt: bool,
    assoc_info: &AssocTypeInfo,
    inner_name: &syn::Ident,
    trait_dyn_ref: &proc_macro2::TokenStream,
    hot_types: &[syn::Type],
) -> Vec<proc_macro2::TokenStream> {
    trait_item
        .items
        .iter()
        .filter_map(|item| {
            let syn::TraitItem::Fn(m) = item else {
                return None;
            };
            let references_assoc =
                method_references_assoc_types(&m.sig, &assoc_info.names);
            if !can_devirt || references_assoc {
                Some(generate_fallback_method(m, inner_name, &assoc_info.rewrites))
            } else {
                Some(generate_dispatch_method(
                    m, trait_dyn_ref, inner_name, hot_types, &assoc_info.rewrites,
                ))
            }
        })
        .collect()
}

fn build_delegating_methods(
    trait_item: &syn::ItemTrait,
    trait_dyn_ref: &proc_macro2::TokenStream,
    assoc_rewrites: &HashMap<String, syn::Ident>,
) -> Vec<proc_macro2::TokenStream> {
    trait_item
        .items
        .iter()
        .filter_map(|item| {
            let syn::TraitItem::Fn(m) = item else {
                return None;
            };
            Some(generate_delegating_method(m, trait_dyn_ref, assoc_rewrites))
        })
        .collect()
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

    // ── Trait-level generics ──────────────────────────────────────
    let has_trait_generics = !trait_item.generics.params.is_empty();
    let trait_generic_params = &trait_item.generics.params;
    let trait_where_clause = &trait_item.generics.where_clause;
    let (_, trait_ty_generics, _) = trait_item.generics.split_for_impl();

    let assoc_info = collect_assoc_types(trait_item);
    let can_devirt = !has_trait_generics;

    let trait_dyn_ref = build_trait_dyn_ref(
        name,
        trait_generic_params,
        &assoc_info.idents,
        &assoc_info.generics,
    );
    let spec_decls = generate_spec_decls(trait_item);
    let dispatch_methods = build_dispatch_methods(
        trait_item, can_devirt, &assoc_info, &inner_name, &trait_dyn_ref, hot_types,
    );
    let delegating_methods = build_delegating_methods(
        trait_item, &trait_dyn_ref, &assoc_info.rewrites,
    );

    let inner_supers = if supertraits.is_empty() {
        quote! {}
    } else {
        quote! { : #supertraits }
    };
    let public_supers = if supertraits.is_empty() {
        quote! { #inner_name #trait_ty_generics }
    } else {
        quote! { #inner_name #trait_ty_generics + #supertraits }
    };

    let mut extra_params: Vec<proc_macro2::TokenStream> = Vec::new();
    for param in trait_generic_params {
        extra_params.push(quote! { #param });
    }
    for gp in &assoc_info.generics {
        extra_params.push(quote! { #gp });
    }
    let inherent_impl_generics = if extra_params.is_empty() {
        quote! { <'__devirt> }
    } else {
        quote! { <'__devirt, #(#extra_params),*> }
    };
    let trait_def_generics = if has_trait_generics {
        quote! { <#trait_generic_params> }
    } else {
        quote! {}
    };

    let fat_ptr_assertion = build_fat_ptr_assertion(trait_item);
    let vtable_helpers = build_vtable_helpers(
        can_devirt, name, &inner_name, &inherent_impl_generics,
        &trait_dyn_ref, &assoc_info.idents,
    );
    let blanket_impl = build_blanket_impl(
        unsafety.as_ref(), has_trait_generics, name, &inner_name,
        trait_generic_params, &trait_ty_generics, trait_where_clause.as_ref(),
    );
    let assoc_type_decls = &assoc_info.decls;

    quote! {
        #[doc(hidden)]
        #vis #unsafety trait #inner_name #trait_def_generics
            #inner_supers #trait_where_clause
        { #(#assoc_type_decls)* #(#spec_decls)* }

        #fat_ptr_assertion

        #vtable_helpers

        impl #inherent_impl_generics dyn #trait_dyn_ref + '__devirt {
            #(#dispatch_methods)*
        }
        impl #inherent_impl_generics
            dyn #trait_dyn_ref + ::core::marker::Send + '__devirt
        { #(#delegating_methods)* }
        impl #inherent_impl_generics
            dyn #trait_dyn_ref + ::core::marker::Sync + '__devirt
        { #(#delegating_methods)* }
        impl #inherent_impl_generics
            dyn #trait_dyn_ref + ::core::marker::Send
            + ::core::marker::Sync + '__devirt
        { #(#delegating_methods)* }

        #(#outer_attrs)*
        #vis #unsafety trait #name #trait_def_generics
            : #public_supers #trait_where_clause {}
        #blanket_impl
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

// ── Associated type helpers ────────────────────────────────────────────────

struct AssocTypeFinder<'a> {
    assoc_names: &'a HashSet<String>,
    found: bool,
}

impl Visit<'_> for AssocTypeFinder<'_> {
    fn visit_path(&mut self, i: &syn::Path) {
        if i.segments.len() >= 2
            && i.segments[0].ident == "Self"
            && self
                .assoc_names
                .contains(&i.segments[1].ident.to_string())
        {
            self.found = true;
        }
        syn::visit::visit_path(self, i);
    }
}

fn method_references_assoc_types(
    sig: &syn::Signature,
    assoc_names: &HashSet<String>,
) -> bool {
    if assoc_names.is_empty() {
        return false;
    }
    let mut finder = AssocTypeFinder { assoc_names, found: false };
    syn::visit::visit_signature(&mut finder, sig);
    finder.found
}

struct RewriteSelfAssocTypes {
    rewrites: HashMap<String, syn::Ident>,
}

impl VisitMut for RewriteSelfAssocTypes {
    fn visit_path_mut(&mut self, i: &mut syn::Path) {
        syn::visit_mut::visit_path_mut(self, i);
        if i.segments.len() >= 2
            && i.segments[0].ident == "Self"
        {
            let name = i.segments[1].ident.to_string();
            if let Some(replacement) = self.rewrites.get(&name) {
                let remaining: Vec<syn::PathSegment> =
                    i.segments.iter().skip(2).cloned().collect();
                let mut first = syn::PathSegment::from(replacement.clone());
                first.arguments = i.segments[1].arguments.clone();
                let mut new_segments = Punctuated::new();
                new_segments.push(first);
                for seg in remaining {
                    new_segments.push(seg);
                }
                i.segments = new_segments;
            }
        }
    }
}

fn generate_fallback_method(
    method: &syn::TraitItemFn,
    inner_name: &syn::Ident,
    assoc_rewrites: &HashMap<String, syn::Ident>,
) -> proc_macro2::TokenStream {
    let sig = &method.sig;
    let attrs = &method.attrs;
    let spec_name = format_ident!("__spec_{}", sig.ident);
    let is_unsafe = sig.unsafety.is_some();

    let (mut dispatch_sig, arg_names) = rewrite_sig_with_named_args(sig);
    if !assoc_rewrites.is_empty() {
        let mut rewriter = RewriteSelfAssocTypes {
            rewrites: assoc_rewrites.clone(),
        };
        rewriter.visit_signature_mut(&mut dispatch_sig);
    }

    let call = quote! { #inner_name::#spec_name(self, #(#arg_names),*) };
    let body = if is_unsafe {
        quote! { unsafe { #call } }
    } else {
        call
    };

    quote! {
        #(#attrs)*
        #[doc(hidden)]
        #[inline]
        pub #dispatch_sig {
            #body
        }
    }
}

// ── Dispatch method generation ──────────────────────────────────────────────

fn generate_dispatch_method(
    method: &syn::TraitItemFn,
    trait_dyn_ref: &proc_macro2::TokenStream,
    inner_name: &syn::Ident,
    hot_types: &[syn::Type],
    assoc_rewrites: &HashMap<String, syn::Ident>,
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

    let (mut dispatch_sig, arg_names) = rewrite_sig_with_named_args(sig);
    if !assoc_rewrites.is_empty() {
        let mut rewriter = RewriteSelfAssocTypes {
            rewrites: assoc_rewrites.clone(),
        };
        rewriter.visit_signature_mut(&mut dispatch_sig);
    }

    let raw_parts = if is_mut {
        quote! { let __raw = <dyn #trait_dyn_ref>::__devirt_raw_parts(&*self); }
    } else {
        quote! { let __raw = <dyn #trait_dyn_ref>::__devirt_raw_parts(self); }
    };

    let hot_checks = gen_hot_checks(
        hot_types, trait_dyn_ref, &spec_name, &arg_names, is_mut,
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
    trait_dyn_ref: &proc_macro2::TokenStream,
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
                        == <dyn #trait_dyn_ref>::__devirt_vtable_for::<#hot>()
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
                        == <dyn #trait_dyn_ref>::__devirt_vtable_for::<#hot>()
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
    trait_dyn_ref: &proc_macro2::TokenStream,
    assoc_rewrites: &HashMap<String, syn::Ident>,
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
    let (mut dispatch_sig, arg_names) = rewrite_sig_with_named_args(sig);
    if !assoc_rewrites.is_empty() {
        let mut rewriter = RewriteSelfAssocTypes {
            rewrites: assoc_rewrites.clone(),
        };
        rewriter.visit_signature_mut(&mut dispatch_sig);
    }

    let coerce_and_call = if is_mut {
        quote! {
            let __devirt_base: &mut (dyn #trait_dyn_ref + '__devirt) = self;
            __devirt_base.#method_name(#(#arg_names),*)
        }
    } else {
        quote! {
            let __devirt_base: &(dyn #trait_dyn_ref + '__devirt) = self;
            __devirt_base.#method_name(#(#arg_names),*)
        }
    };

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
    let trait_segment = trait_path
        .segments
        .last()
        .expect("validated: path non-empty");
    let trait_name = &trait_segment.ident;
    let inner_name = format_ident!("__{trait_name}Impl");
    let trait_args = &trait_segment.arguments;
    let ty = &impl_item.self_ty;
    let (impl_generics, _, where_clause) =
        impl_item.generics.split_for_impl();

    let type_items: Vec<_> = impl_item
        .items
        .iter()
        .filter_map(|item| {
            if let syn::ImplItem::Type(t) = item {
                Some(quote! { #t })
            } else {
                None
            }
        })
        .collect();

    // Collect method names so sibling calls in impl bodies
    // (e.g. `self.area()`) are rewritten to `self.__spec_area()`.
    let method_names: HashSet<String> = impl_item
        .items
        .iter()
        .filter_map(|item| {
            if let syn::ImplItem::Fn(m) = item {
                Some(m.sig.ident.to_string())
            } else {
                None
            }
        })
        .collect();

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
            let mut block = m.block.clone();
            let mut rewriter = RewriteSelfCalls {
                method_names: method_names.clone(),
            };
            rewriter.visit_block_mut(&mut block);
            Some(quote! {
                #(#attrs)*
                #[inline]
                #[allow(clippy::unnecessary_literal_bound)]
                #spec_sig #block
            })
        })
        .collect();

    quote! {
        #unsafety impl #impl_generics #inner_name #trait_args
            for #ty #where_clause
        {
            #(#type_items)*
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
