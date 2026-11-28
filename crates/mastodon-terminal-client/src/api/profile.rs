//! Only changed profile attributes are sent, in one PATCH including pictures.
use super::*;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ProfileChange {
    pub attributes: Value,
    pub pictures: Vec<(String, PathBuf)>,
}

impl Client {
    pub(crate) fn update_profile(&self, change: &ProfileChange) -> Result<Account> {
        let path = "v1/accounts/update_credentials";
        let url = format!("{}/api/{path}", self.origin);
        let (content_type, body) = profile_body(change)?;
        let answer = self.exchange(path, false, &|| {
            let req = self
                .agent
                .patch(&url)
                .config()
                .timeout_global(Some(UPLOAD_TIMEOUT))
                .build()
                .header("Content-Type", &content_type);
            self.signed(req).send(&body[..])
        })?;
        match answer {
            Answer::Took { value, .. } => {
                serde_json::from_value(value).map_err(|e| Error::api(format!("{path}: {e}")))
            }
            Answer::Refused(r) => Err(self.refusal(path, r)),
        }
    }
}

fn profile_body(change: &ProfileChange) -> Result<(String, Vec<u8>)> {
    if change.pictures.is_empty() {
        return Ok((
            "application/json".into(),
            change.attributes.to_string().into_bytes(),
        ));
    }
    // Read and strip every file before issuing the sole write.
    let mut pictures = Vec::new();
    for (field, path) in &change.pictures {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if !terminal_kit::files::is_picture_name(&name) {
            return Err(Error::usage("choose a JPEG, PNG, GIF or WebP picture"));
        }
        let bytes =
            std::fs::read(path).map_err(|e| Error::usage(format!("cannot read picture: {e}")))?;
        pictures.push((
            field,
            name.clone(),
            crate::metadata::strip(&name, &bytes)?.into_owned(),
        ));
    }
    let mut fields = Vec::new();
    if let Some(attrs) = change.attributes.as_object() {
        for (key, value) in attrs {
            if key == "fields_attributes" {
                if let Some(values) = value.as_object() {
                    for (i, field) in values.iter() {
                        for k in ["name", "value"] {
                            fields.push((
                                format!("fields_attributes[{i}][{k}]"),
                                field[k].as_str().unwrap_or_default().into(),
                            ));
                        }
                    }
                }
            } else {
                fields.push((
                    key.clone(),
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string()),
                ));
            }
        }
    }
    let fields: Vec<_> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let files: Vec<_> = pictures
        .iter()
        .map(|(field, name, data)| {
            (
                field.as_str(),
                name.as_str(),
                mime_of(name, data),
                data.as_slice(),
            )
        })
        .collect();
    let boundary = random_hex("mstdn-")?;
    Ok((
        format!("multipart/form-data; boundary={boundary}"),
        multipart(&boundary, &fields, &files),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_attributes_only_and_pictures_are_stripped() {
        let mut change = ProfileChange {
            attributes: json!({"locked":false,"fields_attributes":{"0":{"name":"site","value":"https://example.test"}}}),
            pictures: Vec::new(),
        };
        let (kind, body) = profile_body(&change).unwrap();
        assert_eq!(kind, "application/json");
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            change.attributes
        );
        change.pictures.push((
            "avatar".into(),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("e2e/atago/testdata/gps.jpg"),
        ));
        let (kind, body) = profile_body(&change).unwrap();
        assert!(kind.starts_with("multipart/form-data; boundary="));
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("name=\"avatar\"; filename=\"gps.jpg\""));
        assert!(text.contains("name=\"fields_attributes[0][name]\"\r\n\r\nsite"));
        assert!(!body.windows(6).any(|w| w == b"Exif\0\0"));
    }
}
