// Copyright (c) 2022 Jonathan "Razordor" Alan Thomason
#![feature(proc_macro_quote)]
use proc_macro::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy)]
enum Consume {
    Visibility,
    Signature,
}

#[proc_macro_attribute]
pub fn dylink(attr: TokenStream, item: TokenStream) -> TokenStream {
    // TODO: provide alternative parse for function form
    let mut item_iter = item.into_iter();
    if let TokenTree::Ident(kw) = item_iter.next().unwrap() {
        assert!(kw.to_string() == "extern");
    } else {
        return "compile_error!(\"expected keyword `extern`\");"
            .parse::<TokenStream>()
            .unwrap();
    }
    let call_conv = if let Some(literal @ TokenTree::Literal(_)) = item_iter.next() {
        TokenStream::from(literal)
    } else {
        return "compile_error!(\"expected literal\");"
            .parse::<TokenStream>()
            .unwrap();
    };
    if let TokenTree::Group(group) = item_iter.next().unwrap() {
        parse_fn_list(group.stream(), get_link_type(attr).unwrap(), call_conv)
    } else {
        "compile_error!(\"expected group\");"
            .parse::<TokenStream>()
            .unwrap()
    }
}

fn get_link_type(attr: TokenStream) -> Result<TokenStream, LexError> {
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

    match lib_name.as_str() {
        "vulkan" => "LinkType::Vulkan".to_owned(),
        "opengl" => "LinkType::OpenGL".to_owned(),
        lib_name => {
            format!("LinkType::General{{library: {lib_name}.dll\"}}")
        }
    }
    .parse()
}

fn parse_fn_list(list: TokenStream, link_type: TokenStream, call_conv: TokenStream) -> TokenStream {
    let mut item_ret = TokenStream::new();
    let mut function_name = TokenStream::new();
    let mut signature = TokenStream::new();
    let mut vis = TokenStream::new();
    let mut command = Consume::Visibility;

    let lazyfn_path: TokenStream = "dylink::lazyfn".parse().unwrap();

    let mut tk_list = list.into_iter().peekable();
    while let Some(token) = tk_list.next() {
        match &token {
            TokenTree::Ident(ident) => {
                if "fn" == ident.to_string() {
                    if let Some(name) = tk_list.next() {
                        function_name.extend(quote!($name));
                    } else {
                        unreachable!("ill formed function signature");
                    }
                    command = Consume::Signature;
                    continue;
                }
            }
            TokenTree::Punct(punct) => {
                // if semicolon, then finish off parsing
                if ';' == punct.as_char() {
                    let mut last_dash = false;
                    let mut ret_type = TokenStream::new();
                    let mut param_types = Vec::<TokenStream>::new();
                    for meta in signature.into_iter() {
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
                                            TokenTree::Punct(ref punct) => match punct.as_char() {
                                                ',' => {
                                                    last_comma = true;
                                                    next_param_type = false;
                                                }
                                                ':' => {
                                                    if last_comma && last_ident {
                                                        last_ident = false;
                                                        last_comma = false;
                                                        next_param_type = true;
                                                        param_types.push(TokenStream::new());
                                                        continue;
                                                    }
                                                }
                                                _ => (),
                                            },
                                            _ => last_ident = false,
                                        }
                                        if next_param_type {
                                            param_types.last_mut().unwrap().extend(quote!($arg));
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
                    signature = TokenStream::new();

                    let mut params_no_type = TokenStream::new();
                    let mut params_with_type = TokenStream::new();
                    for (i, data_type) in param_types.into_iter().enumerate() {
                        let param_name = format!("p{i}").parse::<TokenStream>().unwrap();
                        params_no_type.extend(quote!($param_name,));
                        params_with_type.extend(quote!($param_name : $data_type,))
                    }

                    static MOD_COUNT: AtomicU64 = AtomicU64::new(0);

                    let initial_fn: TokenStream = format!(
                        "__dylink_initializer{}",
                        MOD_COUNT.fetch_add(1, Ordering::SeqCst)
                    )
                    .parse()
                    .unwrap();

                    item_ret.extend(quote! {
                        #[doc(hidden)]
                        unsafe extern $call_conv fn $initial_fn($params_with_type) $ret_type {
                            $function_name.link_addr($lazyfn_path::$link_type).unwrap();
                            $function_name($params_no_type)
                        }
                        #[allow(non_upper_case_globals)]
                        $vis static $function_name
                        : $lazyfn_path::LazyFn<unsafe extern $call_conv fn($params_with_type) $ret_type>
                        = $lazyfn_path::LazyFn::new(stringify!($function_name), $initial_fn);
                    });

                    // CLEAN UP
                    function_name = TokenStream::new();

                    vis = TokenStream::new();
                    command = Consume::Visibility;
                    continue;
                }
            }
            _ => {}
        }
        match command {
            Consume::Visibility => vis.extend(quote!($token)),
            Consume::Signature => signature.extend(quote!($token)),
        }
    }
    item_ret
}
