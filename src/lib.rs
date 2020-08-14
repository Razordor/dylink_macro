// Copyright (c) 2020 Jonathan
use proc_macro::{TokenStream, TokenTree, Ident, Span};

macro_rules! tk_error_msg {
    ($name:literal, &$expected:ident, &$found:ident) => {
        let mut error = String::from("compile_error!(\"");
        error.push_str($name);
        error.push_str(" error: expected `");
        error.push_str(& $expected);
        error.push_str("`, but found `");
        error.push_str(& $found);
        error.push_str("`.\");");
        return error.parse().unwrap()
    };
    ($name:literal, $expected:ident, $found:ident) => {
        let mut error = String::from("compile_error!(\"");
        error.push_str($name);
        error.push_str(" error: expected `");
        error.push_str(&$expected.to_string());
        error.push_str("`, but found `");
        error.push_str(&$found.to_string());
        error.push_str("`.\");");
        return error.parse().unwrap()
    };
}

macro_rules! tk_assert_eq {
    ($stream:ident [$index:literal] , Ident($val:literal)) => {
        if let TokenTree::Ident(id) = &($stream.clone().into_iter()).nth($index).expect("expected identifier, found nothing") {
            let found = id.to_string();
            let expected = $val.to_string();
            if found != expected { 
                tk_error_msg!("identifier", &expected, &found);
            }
        } else {panic!("expected identifier, but found `{:?}`", $stream.clone().into_iter().clone().nth($index).unwrap())}
    };
    ($stream:ident [$index:literal] , Punct($val:literal)) => {
        if let TokenTree::Punct(punct) = &($stream.clone().into_iter()).nth($index).expect("expected punctuation, found nothing") { 
            let expected = $val;
            let found = punct.as_char();
            if found != $val { 
                tk_error_msg!("punctuation", expected, found);
            }
        } else {panic!("expected punctuation, but found `{:?}`", $stream.clone().into_iter().clone().nth($index).unwrap())}
    };
    ($stream:ident [$index:literal] , Literal($val:literal)) => {
        if let TokenTree::Literal(li) = &($stream.clone().into_iter()).nth($index).expect("expected literal, found nothing") {            
            let expected = $val.to_string();
            let found = li.to_string();
            if expected != found { 
                tk_error_msg!("literal", &expected, &found);
            }
        } else {panic!("expected literal, found `{:?}`", $stream.clone().into_iter().clone().nth($index).unwrap())}
    };
    ($stream:ident [$index:literal] , Ident(_)) => {
        if let TokenTree::Ident(_) = &($stream.clone().into_iter()).nth($index).expect("expected identifier, found nothing") { 
        } else {panic!("expected identifier, but found `{:?}`", $stream.clone().into_iter().clone().nth($index).unwrap())}
    };
    ($stream:ident [$index:literal] , Punct(_)) => {
        if let TokenTree::Punct(_) = &($stream.clone().into_iter()).nth($index).expect("expected punctuation, found nothing") { 
        } else {panic!("expected punctuation, but found `{:?}`", $stream.clone().into_iter().clone().nth($index).unwrap())}
    };
    ($stream:ident [$index:literal] , Literal(_)) => {
        if let TokenTree::Literal(_) = &($stream.clone().into_iter()).nth($index).expect("expected literal, found nothing") {
        } else {
            return "compile_error!(\"expected literal\");".parse().unwrap();
        }
    };
    ($stream:ident [$index:literal] , Group(_)) => {
        if let TokenTree::Group(_) = &($stream.clone().into_iter()).nth($index).expect("expected group, found nothing") {
        } else {           
            return "compile_error!(\"expected group\");".parse().unwrap();
        }
    };
}

macro_rules! tk_parse {
    ($lexemes:literal) => {$lexemes.parse::<TokenStream>().unwrap()};    
    ($stream:ident [$index:literal]) => {TokenStream::from($stream.clone().into_iter().clone().nth($index).unwrap())};
    ($token:ident) => {TokenStream::from($token.clone())};
}

macro_rules! expand_ident {
    ($token:ident) => {if let TokenTree::Ident(id) = $token.clone() {id} else {panic!()}}
}

#[derive(Clone, Copy)]
enum Consume {    
    Visibility,
    Skip,
    Prefix,
    Param
}

#[proc_macro_attribute]
pub fn dylink (attr: TokenStream, item: TokenStream) -> TokenStream{    
    tk_assert_eq!(attr[0], Ident("name"));
    tk_assert_eq!(attr[1], Punct('='));
    tk_assert_eq!(attr[2], Literal(_));    

    tk_assert_eq!(item[0], Ident("extern"));
    tk_assert_eq!(item[1], Literal(_));
    tk_assert_eq!(item[2], Group(_));
    
    let mut item_ret = TokenStream::new();
    if let TokenTree::Group(group) = item.clone().into_iter().nth(2).expect("p-0") {        
        let mut use_after_skip = Consume::Skip;
        let mut function_name: Ident = Ident::new("uninitialized", Span::call_site());
        let mut begin_declaration = true;
        let mut command = Consume::Visibility;
        let mut param_list = TokenStream::new();
        for token in group.stream().clone() {
            if begin_declaration {
                item_ret.extend(tk_parse!("#[allow(non_upper_case_globals)]"));
                begin_declaration = false;
            }
            match &token {
                TokenTree::Ident(identifier) => if "fn" == identifier.clone().to_string() {                    
                    command = Consume::Skip;
                    use_after_skip = Consume::Prefix;
                },
                TokenTree::Punct(punct) => if ';' == punct.clone().as_char() {                                                            
                    item_ret.extend(tk_parse!("> ="));               

                    // BEGIN BODY                    
                    let mut init_function_block = "dylink::Lazy::new(|| unsafe {std::mem::transmute(".to_string();
                    if let TokenTree::Literal(li) = &(attr.clone().into_iter()).nth(2).expect("attribute: range error (diagnostic not implemented)") {
                        let mut li = li.to_string();
                        li.make_ascii_lowercase();
                        if li == "\"vulkan-1\"" {
                            init_function_block.push_str("dylink::vkloader(\"");
                            init_function_block.push_str(&function_name.to_string());
                            if let Option::Some(TokenTree::Punct(punct)) = &(attr.clone().into_iter()).nth(3) {                                 
                                if punct.to_string() != "," {
                                    let mut error = String::from("compile_error!(\"punctuation error: expected `,`, but found `");
                                    error.push_str(&punct.to_string());
                                    error.push_str("`.\");");
                                    return error.parse().unwrap();
                                } else {
                                    tk_assert_eq!(attr[4], Ident("context"));
                                    tk_assert_eq!(attr[5], Punct('='));
                                    init_function_block.push_str("\",");
                                    let mut index = 6;
                                    loop {
                                        if let Option::Some(token) = &(attr.clone().into_iter()).nth(index) {
                                            init_function_block.push_str(&token.to_string());
                                            index += 1;
                                        } else {break;}                                        
                                    }
                                    init_function_block.push_str("))});");
                                    
                                }
                            } else {
                                init_function_block.push_str("\", dylink::Context::new()))});");
                            }
                        } else if li == "\"opengl32\"" {
                            init_function_block.push_str("dylink::glloader(\"");
                            init_function_block.push_str(&function_name.to_string());
                            init_function_block.push_str("\"))});");
                        }
                        else {
                            init_function_block.push_str("dylink::loader(");
                            init_function_block.push_str(&li.to_string()); // library name
                            init_function_block.pop();// remove '\"' from the end
                            init_function_block.push_str(".dll\",\"");
                            init_function_block.push_str(&function_name.to_string()); // function name     
                            init_function_block.push_str("\"))});");                       
                        }
                    }
                    // END BODY

                    // PARSE BODY
                    item_ret.extend(init_function_block.parse::<TokenStream>().unwrap());


                    param_list = TokenStream::new();
                    command = Consume::Skip;
                    use_after_skip = Consume::Visibility;
                    begin_declaration = true;
                },
                _ => {}
            }
            match command {
                Consume::Visibility => item_ret.extend(tk_parse!(token)),
                Consume::Prefix => {
                    function_name = expand_ident!(token);
                    item_ret.extend(tk_parse!("static"));
                    item_ret.extend(tk_parse!(token));
                    item_ret.extend(tk_parse!(": dylink::Lazy<unsafe extern"));
                    item_ret.extend(tk_parse!(item[1]));
                    item_ret.extend(tk_parse!("fn"));
                    command = Consume::Param;
                }
                Consume::Param => {
                    item_ret.extend(tk_parse!(token));
                    param_list.extend(tk_parse!(token));
                }
                Consume::Skip => command = use_after_skip               
            }
        }
        
    } else {return tk_parse!("compile_error!(\"expected group\");");}
    //panic!("{}",item_ret.clone().to_string());
    item_ret
}

