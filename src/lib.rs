// Copyright (c) 2022 Jonathan "Razordor" Alan Thomason
#![feature(proc_macro_quote)]
use proc_macro::*;
use std::fmt::format;
use std::sync::atomic::{AtomicU64, Ordering};

macro_rules! tk_error_msg {
    ($name:literal, &$expected:ident, &$found:ident) => {
        return format(format_args!(
            "compile_error!(\"{} error: expected `{}`, but found `{}`.\");",
            $name, $expected, $found
        ))
        .parse()
        .unwrap();
    };
    ($name:literal, $expected:ident, $found:ident) => {
        return format(format_args!(
            "compile_error!(\"{} error: expected `{}`, but found `{}`.\");",
            $name, $expected, $found
        ))
        .parse()
        .unwrap();
    };
}

macro_rules! tk_assert_eq {
    ($stream:ident [$index:literal] , Ident($val:literal)) => {
        if let TokenTree::Ident(id) = &($stream.clone().into_iter())
            .nth($index)
            .expect("expected identifier, found nothing")
        {
            let found = id.to_string();
            let expected = $val.to_string();
            if found != expected {
                tk_error_msg!("identifier", &expected, &found);
            }
        } else {
            panic!(
                "expected identifier, but found `{:?}`",
                $stream.clone().into_iter().clone().nth($index).unwrap()
            )
        }
    };
    ($stream:ident [$index:literal] , Punct($val:literal)) => {
        if let TokenTree::Punct(punct) = &($stream.clone().into_iter())
            .nth($index)
            .expect("expected punctuation, found nothing")
        {
            let expected = $val;
            let found = punct.as_char();
            if found != $val {
                tk_error_msg!("punctuation", expected, found);
            }
        } else {
            panic!(
                "expected punctuation, but found `{:?}`",
                $stream.clone().into_iter().clone().nth($index).unwrap()
            )
        }
    };
    ($stream:ident [$index:literal] , Literal(_)) => {
        if let TokenTree::Literal(_) = &($stream.clone().into_iter())
            .nth($index)
            .expect("expected literal, found nothing")
        {
        } else {
            return "compile_error!(\"expected literal\");".parse().unwrap();
        }
    };
    ($stream:ident [$index:literal] , Group(_)) => {
        if let TokenTree::Group(_) = &($stream.clone().into_iter())
            .nth($index)
            .expect("expected group, found nothing")
        {
        } else {
            return "compile_error!(\"expected group\");".parse().unwrap();
        }
    };
}

#[derive(Clone, Copy)]
enum Consume {
    Visibility,
    Prefix,
    Param,
}

#[proc_macro_attribute]
pub fn dylink(attr: TokenStream, item: TokenStream) -> TokenStream {
    tk_assert_eq!(attr[0], Ident("name"));
    tk_assert_eq!(attr[1], Punct('='));
    tk_assert_eq!(attr[2], Literal(_));
    let mut item_iter = item.into_iter();
    let lib_name = attr.into_iter().nth(2).unwrap();
    let call_conv = TokenStream::from(item_iter.clone().nth(1).unwrap());
    if let TokenTree::Group(group) = item_iter.nth(2).expect("p-0") {
        let mut item_ret = TokenStream::new();
        let mut function_name = TokenStream::new();
        let mut signature = TokenStream::new();
        let mut vis = TokenStream::new();
        let mut command = Consume::Visibility;
        for token in group.stream() {
            match &token {
                TokenTree::Ident(identifier) => {
                    if "fn" == identifier.clone().to_string() {
                        command = Consume::Prefix;
                        continue;
                    }
                }
                TokenTree::Punct(punct) => {
                    // if semicolon, then finish off parsing
                    if ';' == punct.as_char() {
                        // BEGIN SELECT LINKER TYPE
                        let mut init_function_block = "dylink::".to_string();
                        if let TokenTree::Literal(ref li) = lib_name {
                            let mut li = li.to_string();
                            li.make_ascii_lowercase();
                            match li.as_str() {
                                "\"vulkan-1\"" => {
                                    init_function_block
                                        .push_str(&format!("vkloader(\"{function_name}\")"));
                                }
                                "\"opengl32\"" => {
                                    init_function_block
                                        .push_str(&format!("glloader(\"{function_name}\")"));
                                }
                                dll_name => {
                                    init_function_block.push_str(&format!("loader({dll_name}"));
                                    init_function_block.pop(); // remove '\"' from the end
                                    init_function_block
                                        .push_str(&format!(".dll\",\"{function_name}\")"));
                                    // function name
                                }
                            }
                        } else {
                            panic!();
                        }
                        // END SELECT LINKER TYPE

                        // PARSE LINKER TYPE
                        let call_linker = init_function_block.parse::<TokenStream>().unwrap();

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
                        let param_count = param_types.len();
                        for (data_type, i) in param_types.into_iter().zip(0..param_count) {
                            let param_name = "p".to_string() + &i.to_string();
                            let param_name = param_name.parse::<TokenStream>().unwrap();
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
                                $function_name.update(&START, ||std::mem::transmute($call_linker.expect($error_msg)));
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
        item_ret
    } else {
        "compile_error!(\"expected group\");"
            .parse::<TokenStream>()
            .unwrap()
    }
}
