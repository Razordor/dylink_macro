use proc_macro2::TokenStream as TokenStream2;
use std::str::FromStr;
use syn::spanned::Spanned;
use syn::*;

#[derive(PartialEq)]
pub enum LinkType {
    Vulkan,
    // note: dylink_macro must use an owned string instead of `&'static [u8]` since it's reading from the source code.
    Normal(Vec<String>),
}

impl quote::ToTokens for LinkType {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        unsafe {
            match self {
                LinkType::Vulkan => {
                    tokens.extend(TokenStream2::from_str("LinkType::Vulkan").unwrap_unchecked())
                }
                LinkType::Normal(lib_list) => {
                    let mut lib_array = String::from("&[");
                    for name in lib_list {
                        lib_array.push_str(&format!("\"{name}\\0\","))
                    }
                    lib_array.push(']');
                    tokens.extend(
                        TokenStream2::from_str(&format!("LinkType::Normal({lib_array})"))
                            .unwrap_unchecked(),
                    )
                },
            }
        }
    }
}

impl TryFrom<syn::Expr> for LinkType {
    type Error = syn::Error;
    fn try_from(value: syn::Expr) -> std::result::Result<Self, Self::Error> {
        match value {
            Expr::Path(ExprPath { path, .. }) => {
                if path.is_ident("vulkan") {
                    Ok(LinkType::Vulkan)
                } else {
                    Err(Error::new(
                        path.span(),
                        "expected `vulkan`, or `name`",
                    ))
                }
            }
            // TODO: replace panic branches with `Error` returns
            Expr::Assign(assign) => {
                if let Expr::Path(ExprPath { path, .. }) = assign.left.as_ref() {
                    if !path.is_ident("name") {
                        return Err(Error::new(path.span(), "expected `name`"));
                    }
                } else {
                    panic!()
                }
                if let Expr::Lit(ExprLit { lit, .. }) = assign.right.as_ref() {
                    if let Lit::Str(lib) = lit {
                        Ok(LinkType::Normal(vec![lib.value()]))
                    } else {
                        Err(Error::new(lit.span(), "expected `name`"))
                    }
                } else {
                    panic!()
                }
            }
            Expr::Call(call) => {
                // TODO: convert to syn::Error if false
                assert!(matches!(*call.func, Expr::Path(ExprPath { path, .. }) if path.is_ident("any")));

                let mut lib_list = Vec::new();
                for item in call.args.iter() {
                    match item {
                        Expr::Assign(assign) => {
                            if let Expr::Path(ExprPath { path, .. }) = assign.left.as_ref() {
                                if !path.is_ident("name") {
                                    return Err(Error::new(path.span(), "expected `name`"));
                                }
                            } else {
                                panic!()
                            }
                            if let Expr::Lit(ExprLit { lit, .. }) = assign.right.as_ref() {
                                if let Lit::Str(lib) = lit {
                                    lib_list.push(lib.value());                                    
                                } else {
                                    return Err(Error::new(lit.span(), "expected `name`"))
                                }
                            } else {
                                panic!()
                            }
                        }
                        _ => panic!("expected `name = <string>`")
                    }
                }
                if lib_list.is_empty() {
                    panic!("no arguments detected")
                } else {
                    Ok(LinkType::Normal(lib_list))
                }
            }
            _ => panic!(),
        }
    }
}
