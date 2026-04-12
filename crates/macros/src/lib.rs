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

    let hot_types: Vec<syn::Type> =
        parse_macro_input!(attr with Punctuated::<syn::Type, Token![,]>::parse_terminated)
            .into_iter()
            .collect();

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
            @trait
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

    let trait_name = &trait_path.segments.last().expect("trait path is empty").ident;
    let ty = &impl_item.self_ty;

    let method_bodies: Vec<_> = impl_item
        .items
        .iter()
        .filter_map(|item| {
            if let syn::ImplItem::Fn(m) = item {
                let sig = &m.sig;
                let block = &m.block;
                Some(quote! { #sig #block })
            } else {
                None
            }
        })
        .collect();

    quote! {
        ::devirt::__devirt_define! {
            @impl
            #trait_name for #ty {
                #(#method_bodies)*
            }
        }
    }
    .into()
}
