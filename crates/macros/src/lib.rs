//! Proc-macro attribute for [`devirt`](https://docs.rs/devirt).
//!
//! Provides `#[devirt]` as a proc-macro attribute that delegates to
//! `devirt::__devirt_define!`. This crate is an implementation detail
//! of `devirt` and should not be used directly.

use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::punctuated::Punctuated;
use syn::{Token, parse_macro_input};

/// Proc-macro attribute for transparent devirtualization.
///
/// # On a trait definition
///
/// ```ignore
/// #[devirt::devirt(Circle, Rect)]
/// pub trait Shape {
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

fn expand_trait(attr: TokenStream, trait_item: &syn::ItemTrait) -> TokenStream {
    if attr.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "expected hot types: #[devirt(Type1, Type2)]",
        )
        .to_compile_error()
        .into();
    }

    // Reject unsupported trait features.
    if !trait_item.generics.params.is_empty() {
        return syn::Error::new_spanned(
            &trait_item.generics,
            "#[devirt] does not support generic traits",
        )
        .to_compile_error()
        .into();
    }
    if let Some(where_clause) = &trait_item.generics.where_clause {
        return syn::Error::new_spanned(
            where_clause,
            "#[devirt] does not support where clauses on traits",
        )
        .to_compile_error()
        .into();
    }
    if !trait_item.supertraits.is_empty() {
        return syn::Error::new_spanned(
            &trait_item.supertraits,
            "#[devirt] does not support supertraits",
        )
        .to_compile_error()
        .into();
    }
    for item in &trait_item.items {
        match item {
            syn::TraitItem::Type(t) => {
                return syn::Error::new_spanned(
                    t,
                    "#[devirt] does not support associated types",
                )
                .to_compile_error()
                .into();
            }
            syn::TraitItem::Const(c) => {
                return syn::Error::new_spanned(
                    c,
                    "#[devirt] does not support associated constants",
                )
                .to_compile_error()
                .into();
            }
            syn::TraitItem::Fn(f) => {
                if f.default.is_some() {
                    return syn::Error::new_spanned(
                        f,
                        "#[devirt] does not support default method bodies",
                    )
                    .to_compile_error()
                    .into();
                }
            }
            _ => {}
        }
    }

    let hot_types: Vec<syn::Type> =
        parse_macro_input!(attr with Punctuated::<syn::Type, Token![,]>::parse_terminated)
            .into_iter()
            .collect();

    let unsafety = &trait_item.unsafety;
    let vis = &trait_item.vis;
    let name = &trait_item.ident;

    // Extract method signatures as token streams, stripping default bodies.
    let mut methods_tokens = proc_macro2::TokenStream::new();
    for item in &trait_item.items {
        if let syn::TraitItem::Fn(m) = item {
            let sig = &m.sig;
            for a in &m.attrs {
                a.to_tokens(&mut methods_tokens);
            }
            sig.to_tokens(&mut methods_tokens);
            methods_tokens.extend(quote! { ; });
        }
    }

    let mut outer_attrs = proc_macro2::TokenStream::new();
    for a in &trait_item.attrs {
        a.to_tokens(&mut outer_attrs);
    }

    quote! {
        ::devirt::__devirt_define! {
            @trait [#unsafety]
            #outer_attrs
            #vis #name [#(#hot_types),*] {
                #methods_tokens
            }
        }
    }
    .into()
}

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

    // Reject unsupported impl features.
    if !impl_item.generics.params.is_empty() {
        return syn::Error::new_spanned(
            &impl_item.generics,
            "#[devirt] does not support generic impl blocks",
        )
        .to_compile_error()
        .into();
    }
    if let Some(where_clause) = &impl_item.generics.where_clause {
        return syn::Error::new_spanned(
            where_clause,
            "#[devirt] does not support where clauses on impl blocks",
        )
        .to_compile_error()
        .into();
    }

    let unsafety = &impl_item.unsafety;
    let trait_name = &trait_path.segments.last().expect("trait path is empty").ident;
    let ty = &impl_item.self_ty;

    let mut method_bodies = proc_macro2::TokenStream::new();
    for item in &impl_item.items {
        if let syn::ImplItem::Fn(m) = item {
            for a in &m.attrs {
                a.to_tokens(&mut method_bodies);
            }
            m.sig.to_tokens(&mut method_bodies);
            m.block.to_tokens(&mut method_bodies);
        }
    }

    quote! {
        ::devirt::__devirt_define! {
            @impl [#unsafety]
            #trait_name for #ty {
                #method_bodies
            }
        }
    }
    .into()
}
