// Copyright (c) 2022 Jonathan "Razordor" Alan Thomason
#![feature(proc_macro_quote)]
#![feature(proc_macro_diagnostic)]
#![feature(proc_macro_span)]

use quote::*;
use std::sync::atomic::{AtomicU64, Ordering};

use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{TokenStream as TokenStream2, *};

mod diag;

#[proc_macro_attribute]
pub fn dylink(args: TokenStream1, input: TokenStream1) -> TokenStream1 {
    let args = TokenStream2::from(args);
    let input = TokenStream2::from(input);

    let foreign_mod: syn::ItemForeignMod = syn::parse2(input).unwrap();

    diag::foreign_mod_warn(&foreign_mod);

    let link_type = get_link_type(args).unwrap();
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

fn parse_fn(abi: &syn::Abi, fn_item: syn::ForeignItemFn, link_type: &str) -> TokenStream2 {
    let fn_name = fn_item.sig.ident.into_token_stream();
    let abi = abi.into_token_stream();
    let vis = fn_item.vis.into_token_stream();
    let output = fn_item.sig.output.into_token_stream();
    let link_type: TokenStream2 = link_type.parse().unwrap();

    let mut fn_attrs = TokenStream2::new();
    for attr in fn_item.attrs {
        fn_attrs.extend(attr.into_token_stream());
    }

    let lazyfn_path: TokenStream2 = "dylink::lazyfn".parse().unwrap();

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

    static MOD_COUNT: AtomicU64 = AtomicU64::new(0);
    let initial_fn: TokenStream2 = format!(
        "__dylink_initializer{}",
        MOD_COUNT.fetch_add(1, Ordering::SeqCst)
    )
    .parse()
    .unwrap();

    let unsafety: TokenStream2 = fn_item.sig.unsafety.map_or(TokenStream2::new(), |r| r.to_token_stream());

    quote! {
        #[doc(hidden)]
        #unsafety #abi fn #initial_fn(#params_with_type) #output {
            match #fn_name.link_addr(#lazyfn_path::#link_type) {
                Ok(function) => function(#params_no_type),
                Err(err) => panic!("{}", err),
            }
        }

        #[allow(non_upper_case_globals)]
        #fn_attrs
        #vis static #fn_name
        : #lazyfn_path::LazyFn<#unsafety #abi fn(#params_default) #output>
        = #lazyfn_path::LazyFn::new(stringify!(#fn_name), #initial_fn);
    }
}

fn get_link_type(args: TokenStream2) -> Result<String, LexError> {
    let mut default_attr = 0;
    let mut lib_name = String::new();
    for (i, token) in args.into_iter().enumerate() {
        match token {
            TokenTree::Ident(ident) => {
                assert_eq!(i, 0);
                match ident.to_string().as_str() {
                    "name" => default_attr = 1,
                    name => lib_name = name.to_string(),
                }
            }
            TokenTree::Punct(punct) => {
                assert_eq!(i, 1);
                if punct.as_char() == '=' {
                    default_attr += 1;
                }
            }
            TokenTree::Literal(library) => {
                assert_eq!(i, 2);
                default_attr += 1;
                lib_name = library.to_string();
                lib_name.pop().unwrap();
            }
            _ => panic!(),
        }
    }
    assert!(
        lib_name == "vulkan" || lib_name == "opengl" || default_attr == 3,
        "invalid attribute parameter"
    );

    let ret = match lib_name.as_str() {
        "vulkan" => "LinkType::Vulkan".to_owned(),
        "opengl" => "LinkType::OpenGL".to_owned(),
        lib_name => {
            format!("LinkType::General{{library: {lib_name}.dll\"}}")
        }
    };
    Ok(ret)
}
