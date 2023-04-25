// Copyright (c) 2023 Jonathan "Razordor" Alan Thomason
use quote::*;

use proc_macro::TokenStream as TokenStream1;
use proc_macro2::TokenStream as TokenStream2;
use syn::{parse_macro_input, spanned::Spanned};

mod diagnostic;
mod link_ty;
use link_ty::LinkType;

#[proc_macro_attribute]
pub fn dylink(args: TokenStream1, input: TokenStream1) -> TokenStream1 {
    let args = TokenStream2::from(args);
    let foreign_mod = parse_macro_input!(input as syn::ItemForeignMod);

    #[cfg(feature = "warnings")]
    diagnostic::foreign_mod_diag(&foreign_mod);

    let args_span = args.span();
    let expr = match syn::parse2::<syn::Expr>(args) {
        Ok(expr) => expr,
        _ => {
            return syn::Error::new(args_span, "expected expression")
                .into_compile_error()
                .into()
        }
    };
    let link_type = match LinkType::try_from(expr) {
        Ok(tk) => tk,
        Err(e) => {
            return syn::Error::into_compile_error(e).into();
        }
    };
    let mut ret = TokenStream2::new();
    for item in foreign_mod.items {
        use syn::ForeignItem;
        let abi = &foreign_mod.abi;
        match item {
            ForeignItem::Fn(fn_item) => ret.extend(parse_fn(abi, fn_item, &link_type)),
            other => ret.extend(quote!(#abi {#other})),
        }
    }
    TokenStream1::from(ret)
}

fn parse_fn(abi: &syn::Abi, fn_item: syn::ForeignItemFn, link_type: &LinkType) -> TokenStream2 {
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
    let is_checked = *link_type == LinkType::Vulkan && !cfg!(feature = "no_lifetimes");
    let call_dyn_func = if is_checked && fn_name.to_string() == "vkCreateInstance" {
        let inst_param = &param_list[2];
        quote! {
            let result = DYN_FUNC(#(#param_list),*);
            unsafe {
                dylink::Global.insert_instance(
                    *std::mem::transmute::<_, *mut dylink::VkInstance>(#inst_param)
                );
            }
            result
        }
    } else if is_checked && fn_name.to_string() == "vkDestroyInstance" {
        let inst_param = &param_list[0];
        quote! {
            let result = DYN_FUNC(#(#param_list),*);
            unsafe {
                dylink::Global.remove_instance(&std::mem::transmute::<_, dylink::VkInstance>(#inst_param));
            }
            result
        }
    } else if is_checked && fn_name.to_string() == "vkCreateDevice" {
        let inst_param = &param_list[3];
        quote! {
            let result = DYN_FUNC(#(#param_list),*);
            unsafe {
                dylink::Global.insert_device(*std::mem::transmute::<_, *mut dylink::VkDevice>(#inst_param));
            }
            result
        }
    } else if is_checked && fn_name.to_string() == "vkDestroyDevice" {
        let inst_param = &param_list[0];
        quote! {
            let result = DYN_FUNC(#(#param_list),*);
            unsafe {
                dylink::Global.remove_device(&std::mem::transmute::<_, dylink::VkDevice>(#inst_param));
            }
            result
        }
    } else {
        quote!(DYN_FUNC(#(#param_list),*))
    };

    // According to "The Rustonomicon" foreign functions are assumed unsafe,
    // so functions are implicitly prepended with `unsafe`
    //
    // All declarations are wrapped in a function to support macro hygiene.
    quote! {
        #(#fn_attrs)*
        #[allow(non_snake_case)]
        #[inline]
        #vis unsafe #abi fn #fn_name (#(#param_ty_list),*) #output {
            #abi fn initial_fn (#(#param_ty_list),*) #output {
                use std::ffi::CStr;
                const FN_NAME: &'static CStr = unsafe {CStr::from_bytes_with_nul_unchecked(concat!(stringify!(#fn_name), '\0').as_bytes())};
                match DYN_FUNC.load(FN_NAME, dylink::#link_type) {
                    Ok(function) => {function(#(#param_list),*)},
                    Err(err) => panic!("{}", err),
                }
            }
            static DYN_FUNC_REF: &'static #abi fn (#params_default) #output = &(initial_fn as #abi fn (#params_default) #output);
            static DYN_FUNC
            : dylink::LazyFn<#abi fn (#params_default) #output>
            = dylink::LazyFn::new(
                std::sync::atomic::AtomicPtr::new(unsafe {std::mem::transmute(DYN_FUNC_REF)})
            );

            #call_dyn_func
        }
    }
}
