// Copyright (c) 2022 Jonathan "Razordor" Alan Thomason
#![feature(proc_macro_quote)]
use proc_macro::*;
use std::fmt::format;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy)]
enum Consume {
    Visibility,
    Prefix,
    Param,
}

#[proc_macro_attribute]
pub fn dylink(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut is_vulkan = false;
    let mut is_opengl = false;
    let mut default_attr = 0;
    let mut custom_lib = String::new();
    for (i, token) in attr.into_iter().enumerate() {
        match token {
            TokenTree::Ident(ident) => {
                assert_eq!(i, 0);
                match ident.to_string().as_str() {
                    "vulkan" => is_vulkan = true,
                    "opengl" => is_opengl = true,
                    "name" => default_attr = 1,
                    _ => (),
                }
            },
            TokenTree::Punct(punct) => {
                assert_eq!(i, 1);
                if punct.as_char() == '=' {
                    default_attr += 1;
                }
            },
            TokenTree::Literal(library) => {
                assert_eq!(i, 2);
                default_attr += 1;
                custom_lib = library.to_string();
                custom_lib.pop().unwrap();
            }
            _ => panic!(),
        }
    }
    assert!(is_vulkan || is_opengl || default_attr == 3, "invalid attribute parameter");

    let mut call_conv = TokenStream::new();
    let mut item_ret = TokenStream::new();
    for token in item.into_iter() {
        match token {
            TokenTree::Ident(kw) => assert!(kw.to_string() == "extern"),
            literal @ TokenTree::Literal(_) => call_conv = literal.into(),
            TokenTree::Group(fn_list) => {
                let mut function_name = TokenStream::new();
                let mut signature = TokenStream::new();
                let mut vis = TokenStream::new();
                let mut command = Consume::Visibility;
                for token in fn_list.stream() {
                    match &token {
                        TokenTree::Ident(ident) => {
                            if "fn" == ident.to_string() {
                                command = Consume::Prefix;
                                continue;
                            }
                        }
                        TokenTree::Punct(punct) => {
                            // if semicolon, then finish off parsing
                            if ';' == punct.as_char() {
                                let linker_type = if is_vulkan {
                                    format!("dylink::vkloader(\"{function_name}\")")
                                } else if is_opengl {
                                    format!("dylink::glloader(\"{function_name}\")")
                                } else {
                                    
                                    format!("dylink::loader({custom_lib}.dll\",\"{function_name}\")")
                                }
                                .parse::<TokenStream>()
                                .unwrap();

                                let mut last_dash = false;
                                let mut ret_type = TokenStream::new();
                                let mut param_types = Vec::<TokenStream>::new();
                                for meta in signature.clone().into_iter() {
                                    if !ret_type.is_empty() {
                                        ret_type.extend(quote!($meta));
                                    } else {
                                        match meta {
                                            TokenTree::Group(group) => {
                                                let mut last_ident = false;
                                                let mut last_comma = true;
                                                let mut next_param_type = false;
                                                for arg in group.stream() {
                                                    match arg {
                                                        TokenTree::Ident(_) => {
                                                            last_ident = true;
                                                        }
                                                        TokenTree::Punct(ref punct) => {
                                                            match punct.as_char() {
                                                                ',' => {
                                                                    last_comma = true;
                                                                    next_param_type = false;
                                                                }
                                                                ':' => {
                                                                    if last_comma && last_ident {
                                                                        last_ident = false;
                                                                        last_comma = false;
                                                                        next_param_type = true;
                                                                        param_types
                                                                            .push(TokenStream::new());
                                                                        continue;
                                                                    }
                                                                }
                                                                _ => (),
                                                            }
                                                        }
                                                        _ => last_ident = false,
                                                    }
                                                    if next_param_type {
                                                        param_types
                                                            .last_mut()
                                                            .unwrap()
                                                            .extend(quote!($arg));
                                                    }
                                                }
                                            }
                                            TokenTree::Punct(punct) => {
                                                let punct_char = punct.as_char();
                                                if punct_char == '-' {
                                                    last_dash = true;
                                                } else if punct_char == '>' && last_dash {
                                                    last_dash = false;
                                                    ret_type.extend(quote!(->));
                                                }
                                            }
                                            _ => (),
                                        }
                                    }
                                }

                                let mut params_no_type = TokenStream::new();
                                let mut params_with_type = TokenStream::new();
                                for (i, data_type) in param_types.into_iter().enumerate() {                               
                                    let param_name = format!("p{i}").parse::<TokenStream>().unwrap();
                                    params_no_type.extend(quote!($param_name,));
                                    params_with_type.extend(quote!($param_name : $data_type,))
                                }

                                static MOD_COUNT: AtomicU64 = AtomicU64::new(0);

                                let initial_fn = format!(
                                    "__dylink_initializer{}",
                                    MOD_COUNT.fetch_add(1, Ordering::SeqCst)
                                )
                                .parse::<TokenStream>()
                                .unwrap();

                                let error_msg =
                                    format!("\"Dylink Error: `{function_name}` function not found\"")
                                        .parse::<TokenStream>()
                                        .unwrap();

                                item_ret.extend(quote!{
                                    #[doc(hidden)]
                                    unsafe extern $call_conv fn $initial_fn($params_with_type) $ret_type {
                                        static START: std::sync::Once = std::sync::Once::new();
                                        $function_name.update(&START, ||std::mem::transmute($linker_type.expect($error_msg)));
                                        $function_name($params_no_type)
                                    }
                                    #[allow(non_upper_case_globals)]
                                    $vis static $function_name
                                    : dylink::LazyFn<unsafe extern $call_conv fn $signature>
                                    = dylink::LazyFn::new($initial_fn);
                                });

                                // CLEAN UP
                                function_name = TokenStream::new();
                                signature = TokenStream::new();
                                vis = TokenStream::new();
                                command = Consume::Visibility;
                                continue;
                            }
                        }
                        _ => {}
                    }
                    match command {
                        Consume::Visibility => vis.extend(quote!($token)),
                        Consume::Prefix => {
                            function_name.extend(quote!($token));
                            command = Consume::Param;
                        }
                        Consume::Param => signature.extend(quote!($token)),
                    }
                }
            }
            // this shouldn't happen, but check it anyway
            _ => {
                item_ret = "compile_error!(\"expected group\");"
                    .parse::<TokenStream>()
                    .unwrap();
                break;
            }
        }
    }
    item_ret
}
