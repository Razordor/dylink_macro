// Copyright (c) 2022 Jonathan "Razordor" Alan Thomason
#![feature(proc_macro_quote)]
#![feature(proc_macro_diagnostic)]
#![feature(proc_macro_span)]

use quote::*;
use std::sync::atomic::{AtomicU64, Ordering};

use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{TokenStream as TokenStream2, *};
use syn::parse_macro_input;

mod diag;

// TODO: add a derive macro that deals with the dylink vulkan trait

#[proc_macro_attribute]
pub fn dylink(args: TokenStream1, input: TokenStream1) -> TokenStream1 {
    let args = TokenStream2::from(args);
    let foreign_mod = parse_macro_input!(input as syn::ItemForeignMod);

    #[cfg(feature = "warn_diag")]
    diag::foreign_mod_warn(&foreign_mod);

    let link_type = match get_link_type(syn::parse2(args).unwrap()) {
        Ok(tk) => tk,
        Err(e) => {
            return e.into_compile_error().into();
        }
    };
    let mut ret = TokenStream::new();
    for item in foreign_mod.items {
        use syn::ForeignItem;
        match item {
            ForeignItem::Fn(fn_item) => ret.extend(parse_fn(&foreign_mod.abi, fn_item, &link_type)),
            _ => panic!(),
        }
    }
    TokenStream1::from(ret)
}

fn parse_fn(abi: &syn::Abi, fn_item: syn::ForeignItemFn, link_type: &TokenStream2) -> TokenStream2 {
    let fn_name = fn_item.sig.ident.into_token_stream();
    let abi = abi.into_token_stream();
    //let generics = fn_item.sig.generics.into_token_stream();
    let vis = fn_item.vis.into_token_stream();
    let output = fn_item.sig.output.into_token_stream();

    let mut fn_attrs = TokenStream2::new();
    for attr in fn_item.attrs {
        fn_attrs.extend(attr.into_token_stream());
    }

    let mut params_no_type = TokenStream2::new();
    let mut params_with_type = TokenStream2::new();
    let params_default = fn_item.sig.inputs.to_token_stream();
    for (i, arg) in fn_item.sig.inputs.iter().enumerate() {
        if let syn::FnArg::Typed(pat) = arg {
            let ty = pat.ty.to_token_stream();
            let param_name = format!("p{i}").parse::<TokenStream2>().unwrap();
            params_no_type.extend(quote!(#param_name,));
            params_with_type.extend(quote!(#param_name : #ty,));
        } else {
            unreachable!("self arguments make no sense")
        }
    }

    // TODO: replace with `Span::def_site()` when stable, but
    // until then, dylink_macro will be disgustingly unhygienic
    let initial_fn_span = Span::mixed_site();

    static MOD_COUNT: AtomicU64 = AtomicU64::new(0);
    let initial_fn_tree: TokenTree = Ident::new(
        &format!(
            "__dylink_initializer{}",
            MOD_COUNT.fetch_add(1, Ordering::SeqCst)
        ),
        initial_fn_span,
    )
    .into();
    let initial_fn: TokenStream2 = syn::parse2(initial_fn_tree.into()).unwrap();

    let unsafety = if cfg!(feature = "force_unsafe") {
        quote!(unsafe)
    } else {
        fn_item
            .sig
            .unsafety
            .map_or(TokenStream2::new(), |r| r.to_token_stream())
    };

    quote! {
        #[doc(hidden)]
        #[inline(never)]
        #unsafety #abi fn #initial_fn (#params_with_type) #output {
            match #fn_name.link_lib(dylink::lazyfn::#link_type) {
                Ok(function) => function(#params_no_type),
                Err(err) => panic!("{}", err),
            }
        }

        #[allow(non_upper_case_globals)]
        #fn_attrs
        #vis static #fn_name
        : dylink::lazyfn::LazyFn<#unsafety #abi fn (#params_default) #output>
        = dylink::lazyfn::LazyFn::new(stringify!(#fn_name), #initial_fn);
    }
}

fn get_link_type(args: syn::Expr) -> Result<TokenStream2, syn::Error> {
    use std::str::FromStr;
    use syn::{spanned::Spanned, *};
    match args {
        Expr::Path(ExprPath { path, .. }) => {
            if path.is_ident("vulkan") {
                Ok(TokenStream2::from_str("LinkType::Vulkan").unwrap())
            } else if path.is_ident("opengl") {
                Ok(TokenStream2::from_str("LinkType::OpenGL").unwrap())
            } else {
                Err(Error::new(
                    path.span(),
                    "expected `vulkan`, `opengl`, or `name`",
                ))
            }
        }
        Expr::Assign(assign) => {
            if let Expr::Path(ExprPath { path, .. }) = *assign.left {
                if !path.is_ident("name") {
                    return Err(Error::new(path.span(), "expected `name`"));
                }
            } else {
                panic!()
            }
            if let Expr::Lit(ExprLit { lit, .. }) = *assign.right {
                if let Lit::Str(lib) = lit {
                    Ok(format!("LinkType::Normal(\"{}\")", lib.value())
                        .parse()
                        .unwrap())
                } else {
                    Err(Error::new(lit.span(), "expected `name`"))
                }
            } else {
                panic!()
            }
        }
        _ => panic!(),
    }
}
