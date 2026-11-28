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
            use k256::schnorr::{VerifyingKey, SigningKey};
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
    file.items.insert(
        0,
        syn::parse_quote!(
            use std::sync::Mutex;
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
    println!("cargo:rerun-if-changed=../src/config.rs");
    let mut config = syn::parse_file(&fs::read_to_string("../src/config.rs").unwrap()).unwrap();
    config.attrs.clear();
    config
        .items
        .retain(|item| matches!(item, syn::Item::Fn(f) if f.sig.ident == "relay_urls"));
    config.items.insert(
        0,
        syn::parse_quote!(
            use crate::error::{Error, Result};
        ),
    );
    fs::write(out.join("config.rs"), quote!(#config).to_string()).unwrap();
    // Extract pure operations unchanged, without the client/network implementation.
    for (module, names, imports) in [
        (
            "actions",
            &["draft", "tag", "post", "reaction", "reposted"][..],
            "use super::event::{Event, Unsigned, bytes};",
        ),
        (
            "media",
            &["files", "valid_url", "inferred_type", "is_video"][..],
            "use super::event::Event; use serde_json::{Value, json};",
        ),
    ] {
        let source = format!("../src/api/{module}.rs");
        println!("cargo:rerun-if-changed={source}");
        let mut parsed = syn::parse_file(&fs::read_to_string(source).unwrap()).unwrap();
        parsed.attrs.clear();
        parsed.items.retain(
            |item| matches!(item, syn::Item::Fn(f) if names.iter().any(|name| f.sig.ident == name)),
        );
        fs::write(
            out.join(format!("{module}.rs")),
            format!("{imports} {}", quote!(#parsed)),
        )
        .unwrap();
    }
    fs::write(
        out.join("module.rs"),
        format!("#[path = {:?}] pub(crate) mod nip46;", out.join("nip46.rs")),
    )
    .unwrap();
}
