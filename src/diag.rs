// This emits warnings for attributes where applicable
pub(crate) fn foreign_mod_warn(foreign_mod: &syn::ItemForeignMod) {
	let doc_ident = syn::parse_str::<syn::Ident>("doc").unwrap();
    let mut doc_spans = Vec::new();
    for mod_attr in foreign_mod.attrs.iter() {
        if mod_attr.path.is_ident(&doc_ident) {
            doc_spans.push(mod_attr.pound_token.spans[0]);
        }
    }
    if !doc_spans.is_empty() {
        let mut span = doc_spans.remove(0);
        for item in doc_spans.iter() {
            span = span.join(*item).unwrap();
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