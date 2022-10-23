// Copyright (c) 2022 Jonathan "Razordor" Alan Thomason
#![feature(proc_macro_quote)]
use proc_macro::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[proc_macro_attribute]
pub fn dylink(attr: TokenStream, items: TokenStream) -> TokenStream {
    let foreign_mod: syn::ItemForeignMod = syn::parse(items).unwrap();
    let link_type = get_link_type(attr).unwrap();
    let mut ret = TokenStream::new();
    for item in foreign_mod.items {
        use syn::ForeignItem;
        match item {
            ForeignItem::Fn(fn_item) => ret.extend(parse_fn(&foreign_mod.abi, fn_item, &link_type)),
            _ => panic!(),
        }
    }
    ret
}

fn parse_fn(abi: &syn::Abi, fn_item: syn::ForeignItemFn, link_type: &str) -> TokenStream {
    use quote::ToTokens;
    let fn_name = fn_item.sig.ident.into_token_stream();
    let abi = abi.into_token_stream();
    let vis = fn_item.vis.into_token_stream();
    let output = fn_item.sig.output.into_token_stream();
    let link_type: TokenStream = link_type.parse().unwrap();

    let lazyfn_path: TokenStream = "dylink::lazyfn".parse().unwrap();

    let mut params_no_type = TokenStream::new();
    let mut params_with_type = TokenStream::new();
    for (i, arg) in fn_item.sig.inputs.iter().enumerate() {
        if let syn::FnArg::Typed(pat) = arg {
            let ty = pat.ty.to_token_stream();
            let param_name = format!("p{i}").parse::<TokenStream>().unwrap();
            params_no_type.extend(quote!($param_name,));
            params_with_type.extend(quote!($param_name : $ty,))
        } else {
            unreachable!("self arguments make no sense")
        }
    }

    static MOD_COUNT: AtomicU64 = AtomicU64::new(0);
    let initial_fn: TokenStream = format!(
        "__dylink_initializer{}",
        MOD_COUNT.fetch_add(1, Ordering::SeqCst)
    )
    .parse()
    .unwrap();
    quote! {
        #[doc(hidden)]
        unsafe $abi fn $initial_fn($params_with_type) $output {
            match $fn_name.link_addr($lazyfn_path::$link_type) {
                Ok(function) => function($params_no_type),
                Err(err) => panic!("{err}"),
            }
        }
        #[allow(non_upper_case_globals)]
        $vis static $fn_name
        : $lazyfn_path::LazyFn<unsafe $abi fn($params_with_type) $output>
        = $lazyfn_path::LazyFn::new(stringify!($fn_name), $initial_fn);
    }
}

fn get_link_type(attr: TokenStream) -> Result<String, LexError> {
    let mut default_attr = 0;
    let mut lib_name = String::new();
    for (i, token) in attr.into_iter().enumerate() {
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
