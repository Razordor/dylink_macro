use proc_macro2::TokenStream as TokenStream2;
use std::str::FromStr;
use syn::spanned::Spanned;
use syn::*;

#[derive(PartialEq)]
pub enum LinkType {
    Vulkan,
    OpenGL,
    // note: dylink_macro must use an owned string instead of `&'static [u8]` since it's reading from the source code.
    Normal(String),
}

impl quote::ToTokens for LinkType {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        unsafe {
            match self {
                LinkType::Vulkan => tokens.extend(TokenStream2::from_str("LinkType::Vulkan").unwrap_unchecked()),
                LinkType::OpenGL => tokens.extend(TokenStream2::from_str("LinkType::OpenGL").unwrap_unchecked()),
                LinkType::Normal(lib) => {
                    tokens.extend(TokenStream2::from_str(&format!("LinkType::Normal(b\"{lib}\\0\")")).unwrap_unchecked())
                }
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
                } else if path.is_ident("opengl") {
                    Ok(LinkType::OpenGL)
                } else {
                    Err(Error::new(
                        path.span(),
                        "expected `vulkan`, `opengl`, or `name`",
                    ))
                }
            }
            // TODO: replace panic branches with `Error` returns
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
                        Ok(LinkType::Normal(lib.value()))
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
}