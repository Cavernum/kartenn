//! `#[kartenn_testing::pg_test]` (ADR 0028): the test receives a pool on a database cloned
//! from a migrated template instead of one migrated from scratch.
//!
//! ```ignore
//! #[kartenn_testing::pg_test]
//! async fn a_test(pool: PgPool) { … }
//!
//! // Another migrator than `kartenn_store_pg::MIGRATOR`:
//! #[kartenn_testing::pg_test(migrator = "crate::MIGRATOR")]
//! async fn another(pool: PgPool) -> sqlx::Result<()> { … }
//! ```
//!
//! It expands to a `#[test]` that keeps the other attributes (`#[ignore]`,
//! `#[should_panic]`…) and the test path of `#[sqlx::test]`
//! (`module_path!()::name`), and hands the async function to `kartenn_testing::run`.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Expr, ExprLit, ItemFn, Lit, Meta, Token, parse_macro_input};

/// The migrator of the template when the attribute names none.
const DEFAULT_MIGRATOR: &str = "kartenn_store_pg::MIGRATOR";

#[proc_macro_attribute]
pub fn pg_test(args: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    match expand(args.into(), input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand(args: proc_macro2::TokenStream, input: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    let migrator = migrator(args)?;
    if input.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            input.sig.fn_token,
            "#[pg_test] needs an async fn taking a PgPool",
        ));
    }
    if input.sig.inputs.len() != 1 {
        return Err(syn::Error::new_spanned(
            &input.sig.inputs,
            "#[pg_test] needs exactly one argument, the PgPool",
        ));
    }
    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = input;
    let name = &sig.ident;
    let output = match &sig.output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    };
    // `#[sqlx::test]` hid the test function from these style lints, which a scenario often
    // trips (longer than 100 lines, `a1`/`a2`, `a`…`e`); this attribute does the same, so
    // that the bodies do not change (ADR 0028).
    Ok(quote! {
        #[::core::prelude::v1::test]
        #[allow(
            clippy::too_many_lines,
            clippy::similar_names,
            clippy::many_single_char_names
        )]
        #(#attrs)*
        #vis fn #name() -> #output {
            #sig #block
            ::kartenn_testing::run(
                &#migrator,
                ::core::concat!(::core::module_path!(), "::", ::core::stringify!(#name)),
                #name,
            )
        }
    })
}

/// `migrator = "path"`, or the default.
fn migrator(args: proc_macro2::TokenStream) -> syn::Result<syn::Path> {
    let metas = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(args)?;
    let mut path = None;
    for meta in metas {
        match meta {
            Meta::NameValue(nv) if nv.path.is_ident("migrator") => match nv.value {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) => path = Some(s.parse::<syn::Path>()?),
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "migrator = \"path::TO::MIGRATOR\"",
                    ));
                }
            },
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "unknown #[pg_test] argument",
                ));
            }
        }
    }
    path.map_or_else(|| syn::parse_str(DEFAULT_MIGRATOR), Ok)
}
