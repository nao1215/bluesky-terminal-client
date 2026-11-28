//! Blossom BUD-01/02 authorization and NIP-92 attachment metadata.
use super::{
    Client,
    actions::{draft, now, tag},
    event::Event,
};
use crate::error::{Error, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use k256::sha2::{Digest, Sha256};
use serde_json::{Value, json};
use std::{io::Read, path::Path};

pub(crate) fn server_url(value: &str) -> Result<String> {
    let url =
        url::Url::parse(value).map_err(|_invalid| Error::file("invalid Blossom server URL"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::file(
            "Blossom server requires http(s), host and no credentials/query/fragment",
        ));
    }
    Ok(url.to_string().trim_end_matches('/').into())
}
pub(crate) fn files(event: &Event) -> Vec<Value> {
    if let Some(inner) = super::actions::reposted(event) {
        return files(&inner);
    }
    let mut files = Vec::new();
    for tag in &event.tags {
        if tag.first().is_none_or(|s| s != "imeta") {
            continue;
        }
        let field = |key: &str| {
            tag.iter()
                .skip(1)
                .filter_map(|s| s.split_once(' '))
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v)
        };
        let Some(url) = field("url").filter(|u| valid_url(u)) else {
            continue;
        };
        let mime = field("m");
        let kind = if mime.is_some_and(|m| m.starts_with("video/")) {
            "video"
        } else if mime.is_some_and(|m| m.starts_with("image/")) {
            "image"
        } else {
            "file"
        };
        if !files.iter().any(|f: &Value| f["url"] == url) {
            files.push(json!({"id":field("x"),"url":url,"previewUrl":field("image"),"type":kind,"mime":mime,"description":field("alt"),"sha256":field("x"),"size":field("size").and_then(|s|s.parse::<u64>().ok()),"dimensions":field("dim")}));
        }
    }
    for raw in event.content.split_whitespace() {
        let raw = raw.trim_matches(['(', ')', '[', ']', '<', '>', ',', '"']);
        let Some((kind, mime)) = inferred_type(raw) else {
            continue;
        };
        if !files.iter().any(|f| f["url"] == raw) {
            files.push(json!({"id":null,"url":raw,"previewUrl":null,"type":kind,"mime":mime,"description":null}));
        }
    }
    files.sort_by_key(|f| {
        event
            .content
            .find(f["url"].as_str().unwrap_or(""))
            .unwrap_or(usize::MAX)
    });
    files
}
pub(crate) fn valid_url(raw: &str) -> bool {
    url::Url::parse(raw).is_ok_and(|u| {
        matches!(u.scheme(), "http" | "https")
            && u.host_str().is_some()
            && u.username().is_empty()
            && u.password().is_none()
    })
}
impl Client<'_> {
    pub(crate) fn upload(&self, path: &Path, alt: &str) -> Result<Vec<String>> {
        if self.config.blossom_servers.is_empty() {
            return Err(Error::unavailable(
                "upload requires blossomServers in config.json",
            ));
        }
        let file = std::fs::File::open(path).map_err(Error::file)?;
        let mut data = Vec::new();
        file.take(100 * 1024 * 1024 + 1)
            .read_to_end(&mut data)
            .map_err(Error::file)?;
        if data.len() > 100 * 1024 * 1024 {
            return Err(Error::usage("attachment exceeds 100 MiB"));
        }
        let data = terminal_kit::media::metadata::strip(&path.display().to_string(), &data)
            .map_err(Error::usage)?;
        use terminal_kit::media::metadata::{Format, format};
        let mime = match format(&data) {
            Some(Format::Jpeg) => "image/jpeg",
            Some(Format::Png) => "image/png",
            Some(Format::Gif) => "image/gif",
            Some(Format::WebP) => "image/webp",
            Some(Format::Mp4) => "video/mp4",
            Some(Format::WebM) => "video/webm",
            None => return Err(Error::usage("unsupported attachment format")),
        };
        let hash = hex::encode(Sha256::digest(&data));
        let auth = self.sign(draft(
            24242,
            vec![
                tag("t", "upload"),
                tag("x", &hash),
                tag("expiration", &(now()? + 300).to_string()),
            ],
            "Upload attachment".into(),
        ))?;
        let authorization = format!(
            "Nostr {}",
            STANDARD.encode(serde_json::to_vec(&auth).map_err(Error::usage)?)
        );
        let mut errors = Vec::new();
        // Servers are tried in configured order; a successful upload ends the attempt.
        for server in &self.config.blossom_servers {
            let result = (|| {
                let bytes = terminal_kit::media::transfer::put(
                    &format!("{server}/upload"),
                    mime,
                    &authorization,
                    &data,
                    self.config.timeout,
                )
                .map_err(Error::network)?;
                let response: Value = serde_json::from_slice(&bytes).map_err(Error::network)?;
                let url = response["url"]
                    .as_str()
                    .filter(|u| valid_url(u))
                    .ok_or_else(|| Error::network("Blossom response has no valid URL"))?;
                if response["sha256"] != hash
                    || response["size"].as_u64() != Some(data.len() as u64)
                {
                    return Err(Error::network(
                        "Blossom response hash/size does not match upload",
                    ));
                }
                Ok(vec![
                    "imeta".into(),
                    format!("url {url}"),
                    format!("m {mime}"),
                    format!("x {hash}"),
                    format!("size {}", data.len()),
                    format!("alt {alt}"),
                ])
            })();
            match result {
                Ok(tag) => return Ok(tag),
                Err(e) => errors.push(e.error),
            }
        }
        Err(Error::network(errors.join("; ")))
    }
}

pub(crate) fn is_video(raw: &str) -> bool {
    inferred_type(raw).is_some_and(|(kind, _)| kind == "video")
}

fn inferred_type(raw: &str) -> Option<(&'static str, &'static str)> {
    if !valid_url(raw) {
        return None;
    }
    let url = url::Url::parse(raw).ok()?;
    let path = url.path().to_ascii_lowercase();
    let (_, ext) = path.rsplit_once('.')?;
    Some(match ext {
        "jpg" | "jpeg" => ("image", "image/jpeg"),
        "png" => ("image", "image/png"),
        "gif" => ("image", "image/gif"),
        "webp" => ("image", "image/webp"),
        "avif" => ("image", "image/avif"),
        "bmp" => ("image", "image/bmp"),
        "mp4" | "mov" | "m4v" => ("video", "video/mp4"),
        "webm" => ("video", "video/webm"),
        "mkv" => ("video", "video/x-matroska"),
        "avi" => ("video", "video/x-msvideo"),
        _ => return None,
    })
}
pub(crate) fn download(event: &Event, dir: &Path) -> Result<Vec<Value>> {
    use std::io::Write;
    let mut saved = Vec::new();
    for (i, file) in files(event).iter().enumerate() {
        let url = file["url"]
            .as_str()
            .ok_or_else(|| Error::network("attachment URL missing"))?;
        let data =
            terminal_kit::media::transfer::get(url, 100 * 1024 * 1024).map_err(Error::network)?;
        if let Some(hash) = file["sha256"].as_str()
            && hex::encode(Sha256::digest(&data)) != hash
        {
            return Err(Error::network("attachment SHA-256 mismatch"));
        }
        std::fs::create_dir_all(dir).map_err(Error::file)?;
        let path = dir.join(format!("{}-{i}", event.id));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(Error::file)?
            .write_all(&data)
            .map_err(Error::file)?;
        saved.push(
            json!({"postId":event.id,"type":file["type"],"url":url,"path":path,"size":data.len()}),
        );
    }
    Ok(saved)
}
#[cfg(test)]
mod tests {
    use super::*;
    use k256::{elliptic_curve::Generate, schnorr::SigningKey};
    #[test]
    fn media_contract_deduplicates_urls_and_rejects_local_schemes() {
        let event = Event::sign(
            &SigningKey::generate(),
            1,
            1,
            vec![
                vec![
                    "imeta".into(),
                    "url https://example.invalid/video".into(),
                    "m video/mp4".into(),
                    "alt a cat".into(),
                ],
                vec!["imeta".into(), "url file:///secret".into()],
            ],
            "https://example.invalid/cat.png https://example.invalid/cat.png".into(),
        )
        .unwrap();
        let files = files(&event);
        assert_eq!(files.len(), 2);
        assert_eq!(files[1]["type"], "video");
        assert_eq!(files[1]["description"], "a cat");
        for file in files {
            for key in ["id", "type", "mime", "url", "previewUrl", "description"] {
                assert!(file.get(key).is_some());
            }
        }
    }
}
