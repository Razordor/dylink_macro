// Copyright (c) 2022 Jonathan "Razordor" Alan Thomason
#![feature(proc_macro_quote)]
#![feature(proc_macro_diagnostic)]
#![feature(proc_macro_span)]

use quote::*;

use proc_macro::TokenStream as TokenStream1;
use proc_macro2::TokenStream as TokenStream2;
use syn::{parse_macro_input, spanned::Spanned};

mod diagnostic;

#[proc_macro_attribute]
pub fn dylink(args: TokenStream1, input: TokenStream1) -> TokenStream1 {
    let args = TokenStream2::from(args);
    let foreign_mod = parse_macro_input!(input as syn::ItemForeignMod);

    #[cfg(feature = "diagnostic")]
    diagnostic::foreign_mod_diag(&foreign_mod);

    let link_type = match get_link_type(syn::parse2(args).unwrap()) {
        Ok(tk) => tk,
        Err(e) => {
            return e.into_compile_error().into();
        }
    };
    let mut ret = TokenStream2::new();
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
    let vis = fn_item.vis.into_token_stream();
    let output = fn_item.sig.output.into_token_stream();

    let fn_attrs: Vec<TokenStream2> = fn_item
        .attrs
        .iter()
        .map(syn::Attribute::to_token_stream)
        .collect();

    let mut param_list = Vec::new();
    let mut param_ty_list = Vec::new();
    let params_default = fn_item.sig.inputs.to_token_stream();
    for (i, arg) in fn_item.sig.inputs.iter().enumerate() {
        match arg {
            syn::FnArg::Typed(pat_type) => {
                let ty = pat_type.ty.to_token_stream();
                let param_name = match pat_type.pat.as_ref() {
                    syn::Pat::Wild(_) => format!("p{i}").parse::<TokenStream2>().unwrap(),
                    syn::Pat::Ident(pat_id) => pat_id.ident.to_token_stream(),
                    _ => unreachable!(),
                };
                param_list.push(param_name.clone());
                param_ty_list.push(quote!(#param_name : #ty));
            }
            syn::FnArg::Receiver(rec) => {
                return syn::Error::new(rec.span(), "`self` arguments are unsupported")
                    .into_compile_error();
            }
        }
    }

    // TODO: turn this into a diagnostic
    let unsafety = if cfg!(feature = "force_unsafe") {
        quote!(unsafe)
    } else {
        fn_item
            .sig
            .unsafety
            .map_or(TokenStream2::new(), |r| r.to_token_stream())
    };

    let call_dyn_func = if fn_name.to_string() == "vkCreateInstance" {
        let inst_param = &param_list[2];
        quote! {
            let result = function(#(#param_list),*);
            unsafe {
                // whatever type the user uses must match this ABI, so transmute will always work
                dylink::use_instance(*std::mem::transmute::<_, &mut dylink::VkInstance>(#inst_param));
            }
            result
        }
    } else if fn_name.to_string() == "vkDestroyInstance" {
        quote! {
            dylink::use_instance(ptr::null());
            function(#(#param_list),*)            
        }
    } else {
        quote!(function(#(#param_list),*))
    };

    quote! {
        #(#fn_attrs)*
        #[allow(non_snake_case)]
        #[inline]
        #vis #unsafety #abi fn #fn_name (#(#param_ty_list),*) #output {
            #abi fn initial_fn (#(#param_ty_list),*) #output {
                match DYN_FUNC.link() {
                    Ok(function) => {#call_dyn_func},
                    Err(err) => panic!("{}", err),
                }
            }
            static DYN_FUNC
            : dylink::lazyfn::LazyFn<#abi fn (#params_default) #output>
            = dylink::lazyfn::LazyFn::new(stringify!(#fn_name), initial_fn, dylink::lazyfn::#link_type);

            DYN_FUNC(#(#param_list),*)
        }
    }
}

fn get_link_type(args: syn::Expr) -> Result<TokenStream2, syn::Error> {
    use std::str::FromStr;
    use syn::*;
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
