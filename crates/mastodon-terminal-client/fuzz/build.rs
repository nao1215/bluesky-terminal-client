use quote::quote;
use std::{env, fs, path::PathBuf};

fn main() {
    let source = "../src/api/mod.rs";
    println!("cargo:rerun-if-changed={source}");
    let file = syn::parse_file(&fs::read_to_string(source).unwrap()).unwrap();
    let items: Vec<_> = file
        .items
        .into_iter()
        .filter(|item| match item {
            syn::Item::Enum(e) => e.ident == "Next",
            syn::Item::Fn(f) => ["next_of", "percent_decode", "percent_encode"]
                .iter()
                .any(|name| f.sig.ident == name),
            _ => false,
        })
        .collect();
    assert_eq!(
        items.len(),
        4,
        "Link parser items changed; update the harness"
    );
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("link.rs"),
        quote!(#(#items)*).to_string(),
    )
    .unwrap();
}
