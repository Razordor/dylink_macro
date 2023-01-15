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
                }
            }
        }
    }
}

impl TryFrom<syn::Expr> for LinkType {
    type Error = syn::Error;
    fn try_from(value: syn::Expr) -> std::result::Result<Self, Self::Error> {
        match value {
            // Branch for syntax: #[dylink(vulkan)]
            Expr::Path(ExprPath { path, .. }) => {
                if path.is_ident("vulkan") {
                    Ok(LinkType::Vulkan)
                } else {
                    Err(Error::new(
                        path.span(),
                        "expected `vulkan`, `any`, or `name`",
                    ))
                }
            }
            // Branch for syntax: #[dylink(name = "")]
            Expr::Assign(assign) => {
                if !matches!(assign.left.as_ref(), Expr::Path(ExprPath { path, .. }) if path.is_ident("name"))
                {
                    return Err(Error::new(assign.left.span(), "expected identifier `name`"));
                }
                match assign.right.as_ref() {
                    Expr::Lit(ExprLit {
                        lit: Lit::Str(lib), ..
                    }) => Ok(LinkType::Normal(vec![lib.value()])),
                    right => Err(Error::new(right.span(), "expected string literal")),
                }
            }
            // Branch for syntax: #[dylink(any())]
            Expr::Call(call) => {
                if !matches!(call.func.as_ref(), Expr::Path(ExprPath { path, .. }) if path.is_ident("any"))
                {
                    return Err(Error::new(call.func.span(), "expected function `any`"));
                }
                let mut lib_list = Vec::new();
                // This is non-recursive by design.
                // The `any` function should only be used once and vulkan style loading is no longer an option by this point.
                for arg in call.args.iter() {
                    match arg {
                        Expr::Assign(assign) => {
                            if !matches!(assign.left.as_ref(), Expr::Path(ExprPath { path, .. }) if path.is_ident("name"))
                            {
                                return Err(Error::new(
                                    assign.left.span(),
                                    "expected identifier `name`",
                                ));
                            }
                            match assign.right.as_ref() {
                                Expr::Lit(ExprLit {
                                    lit: Lit::Str(lib), ..
                                }) => lib_list.push(lib.value()),
                                right => {
                                    return Err(Error::new(right.span(), "expected string literal"))
                                }
                            }
                        }
                        other => {
                            return Err(Error::new(other.span(), "expected `name = <string>`"))
                        }
                    }
                }
                if lib_list.is_empty() {
                    return Err(Error::new(call.span(), "no arguments detected"));
                } else {
                    Ok(LinkType::Normal(lib_list))
                }
            }
            expr => Err(Error::new(
                expr.span(),
                "expected `vulkan`, `any`, or `name`",
            )),
        }
    }
}
