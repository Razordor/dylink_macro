// Copyright (c) 2022 Jonathan "Razordor" Alan Thomason
#![feature(proc_macro_quote)]
use proc_macro::*;
use std::fmt::format;

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
        let mut param_list = TokenStream::new();
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
                        // BEGIN BODY
                        let mut init_function_block =
                            "".to_string();
                        if let TokenTree::Literal(ref li) = lib_name {
                            let mut li = li.to_string();
                            li.make_ascii_lowercase();
                            match li.as_str() {
                                "\"vulkan-1\"" => {
                                    init_function_block.push_str("dylink::vkloader(\"");
                                    init_function_block.push_str(&function_name.to_string());
                                }
                                "\"opengl32\"" => {
                                    init_function_block.push_str(
                                        format(format_args!(
                                            "dylink::glloader(\"{}",
                                            function_name
                                        ))
                                        .as_str(),
                                    );
                                }
                                other => {
                                    init_function_block.push_str("dylink::loader(");
                                    init_function_block.push_str(other); // library name
                                    init_function_block.pop(); // remove '\"' from the end
                                    init_function_block.push_str(".dll\",\"");
                                    init_function_block.push_str(&function_name.to_string());
                                    // function name
                                }
                            }
                        }
                        init_function_block.push_str("\")");
                        // END BODY

                        // PARSE BODY
                        let body = init_function_block.parse::<TokenStream>().unwrap();

                        item_ret.extend(quote!{
                            #[allow(non_upper_case_globals)]
                            $vis static $function_name: dylink::Lazy<unsafe extern $call_conv fn $param_list> 
                            = dylink::Lazy::new(|| unsafe {
                                std::mem::transmute($body)
                            });
                        });

                        // CLEAN UP
                        function_name = TokenStream::new();
                        param_list = TokenStream::new();
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
                Consume::Param => param_list.extend(quote!($token)),
            }
        }
        item_ret
    } else {
        "compile_error!(\"expected group\");"
            .parse::<TokenStream>()
            .unwrap()
    }
}
