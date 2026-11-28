use quote::quote;
use std::{env, fs, path::PathBuf};

fn main() {
    let source = "../src/api/nip46.rs";
    println!("cargo:rerun-if-changed={source}");
    let mut file = syn::parse_file(&fs::read_to_string(source).unwrap()).unwrap();
    // Keep the URI and response parsers with their original bodies; leave
    // signing, terminal diagnostics and network orchestration out of the harness.
    file.items.retain(|item| match item {
        syn::Item::Struct(s) => ["Bunker", "Response"].iter().any(|n| s.ident == n),
        syn::Item::Enum(e) => e.ident == "Reply",
        syn::Item::Fn(f) => f.sig.ident == "response",
        syn::Item::Impl(i) => {
            matches!(&*i.self_ty, syn::Type::Path(p) if p.path.is_ident("Bunker"))
        }
        _ => false,
    });
    file.items.insert(
        0,
        syn::parse_quote!(
            use super::{
                event::{Event, bytes},
                nip44,
            };
        ),
    );
    file.items.insert(
        0,
        syn::parse_quote!(
            use crate::{
                config::relay_urls,
                error::{Error, Result},
            };
        ),
    );
    file.items.insert(
        0,
        syn::parse_quote!(
            use k256::schnorr::VerifyingKey;
        ),
    );
    file.items.insert(
        0,
        syn::parse_quote!(
            use serde::Deserialize;
        ),
    );
    file.items.insert(
        0,
        syn::parse_quote!(
            use zeroize::Zeroizing;
        ),
    );
    for item in &mut file.items {
        match item {
            syn::Item::Fn(f) if f.sig.ident == "response" => f.vis = syn::parse_quote!(pub(crate)),
            syn::Item::Enum(e) if e.ident == "Reply" => e.vis = syn::parse_quote!(pub(crate)),
            syn::Item::Impl(i) => i
                .items
                .retain(|item| matches!(item, syn::ImplItem::Fn(f) if f.sig.ident == "parse")),
            _ => (),
        }
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out.join("nip46.rs"), quote!(#file).to_string()).unwrap();
    fs::write(
        out.join("module.rs"),
        format!("#[path = {:?}] pub(crate) mod nip46;", out.join("nip46.rs")),
    )
    .unwrap();
}
