// This emits warnings for attributes where applicable
#[cfg(feature = "warn_diag")]
pub(crate) fn foreign_mod_warn(foreign_mod: &syn::ItemForeignMod) {
    use syn::spanned::Spanned;
    let mut doc_spans = Vec::new();
    for mod_attr in foreign_mod.attrs.iter() {
        if mod_attr.path.is_ident("doc") {
            doc_spans.push(mod_attr.span());
        }
    }
    if !doc_spans.is_empty() {
        let mut span = doc_spans.remove(0);
        for item in doc_spans.into_iter() {
            span = span.join(item).unwrap();
        }
        proc_macro::Diagnostic::spanned(
            span.unwrap(),
            proc_macro::Level::Warning,
            "unused doc comment",
        )
        .help("use `//` for a plain comment")
        .emit();
    }
}

/*pub(crate) fn expect_token(tk: proc_macro2::TokenTree) {

}*/
