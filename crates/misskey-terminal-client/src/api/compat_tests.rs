//! Responses captured from isolated Misskey 2025.12.0 and Sharkey 2025.4.3.
//! Only the local port, timestamps and random search marker are normalized.

use super::*;
use rstest::rstest;

fn fixture(service: &str) -> Value {
    serde_json::from_str(match service {
        "misskey" => include_str!("../testdata/compat/misskey.json"),
        "sharkey" => include_str!("../testdata/compat/sharkey.json"),
        _ => unreachable!("test case names one captured server"),
    })
    .unwrap()
}

#[rstest]
#[case("misskey")]
#[case("sharkey")]
fn live_note_media_and_custom_reactions_keep_their_meaning(#[case] service: &str) {
    let data = fixture(service);
    let note: Note = serde_json::from_value(data["custom-note"].clone()).unwrap();
    assert_eq!(note.text.as_deref(), Some("mskycompatfixture"));
    assert_eq!(note.reaction_total(), 1);
    assert_eq!(note.reactions[":compat@.:"], 1);
    assert!(note.reaction_emojis.is_empty()); // Local emoji are in /api/emojis.
    let reactions: Vec<Reaction> =
        serde_json::from_value(data["custom-reactions"].clone()).unwrap();
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].reaction, ":compat@.:");
    assert_eq!(reactions[0].user.username, "compat_user");
    assert!(reactions[0].user.host.is_none());
    assert_eq!(data["emojis"]["emojis"][0]["name"], "compat");
    let media: Note = serde_json::from_value(data["media-note"].clone()).unwrap();
    assert_eq!(media.files.len(), 1);
    assert_eq!(media.files[0].mime, "image/png");
    assert_eq!(
        media.files[0].comment.as_deref(),
        Some("A local compatibility picture")
    );
    let output = crate::cli::format::note_json(&note, "http://127.0.0.1:3000");
    assert_eq!(output["likeCount"], 1);
    // The stale renote count in Misskey remains the server's count. A client
    // cannot derive a total by counting one page of renotes visible to it.
    assert_eq!(output["repostCount"], data["custom-note"]["renoteCount"]);
}

#[rstest]
#[case("misskey")]
#[case("sharkey")]
fn live_errors_separate_policy_authentication_and_token_permissions(#[case] service: &str) {
    let data = fixture(service);
    let client = Client::new("http://127.0.0.1:3000");
    for (name, endpoint, code, hint) in [
        (
            "search-unavailable",
            "notes/search",
            "UNAVAILABLE",
            "role policy",
        ),
        ("authentication", "i", "AUTHENTICATION_FAILED", "token"),
        (
            "permission",
            "chat/history",
            "PERMISSION_DENIED",
            "permission",
        ),
        (
            "role",
            "admin/emoji/add",
            "ROLE_PERMISSION_DENIED",
            "account's role",
        ),
        (
            "missing-note",
            "notes/show",
            "NO_SUCH_NOTE",
            "check the name",
        ),
    ] {
        let response = &data[name];
        let error = client.refusal(
            endpoint,
            response["status"].as_u64().unwrap() as u16,
            ErrorDetail::of(&response["body"].to_string()),
        );
        assert_eq!(error.api_code(), Some(code));
        assert_eq!(error.kind().exit_code(), 4);
        assert!(!chat_unavailable(&error), "{name} does not mean no chat");
        let message = error.hint().unwrap();
        assert!(message.contains("127.0.0.1:3000") && message.contains(hint));
        assert!(!message.contains("visitors"));
    }
    assert_eq!(data["chat"]["body"], json!([]));
}

#[test]
fn only_structured_capability_errors_hide_chat() {
    let client = Client::new("http://127.0.0.1:3000");
    for code in ["NO_SUCH_ENDPOINT", "CHAT_UNAVAILABLE"] {
        let error = client.refusal(
            "chat/history",
            400,
            ErrorDetail {
                code: code.into(),
                message: "Unavailable".into(),
            },
        );
        assert!(chat_unavailable(&error));
        assert_eq!(
            error.kind().exit_code(),
            if code == "NO_SUCH_ENDPOINT" { 5 } else { 4 }
        );
        assert!(error.hint().unwrap().contains("127.0.0.1:3000"));
        assert!(error.hint().unwrap().contains("chat"));
    }
    for message in [
        "chat/history failed: HTTP 404",
        "chat/history: cannot reach the server",
        "chat/history failed: PERMISSION_DENIED: NO_SUCH_ENDPOINT",
    ] {
        assert!(!chat_unavailable(&Error::api(message)));
    }
    let role = client.refusal(
        "chat/history",
        403,
        ErrorDetail {
            code: "ROLE_PERMISSION_DENIED".into(),
            message: "Denied".into(),
        },
    );
    assert!(!chat_unavailable(&role));
    assert!(role.hint().unwrap().contains("account's role"));
}

#[rstest]
#[case(include_str!("../testdata/compat/misskey-upload.json"))]
#[case(include_str!("../testdata/compat/sharkey-upload.json"))]
fn repeated_picture_upload_keeps_the_new_alt_text(#[case] response: &str) {
    use super::stub_tests::{reply, serve};
    let data: Value = serde_json::from_str(response).unwrap();
    assert_eq!(data["reused"]["comment"], "compat media");
    let (url, _) = serve(Arc::new(move |_, body, _| {
        let response = if body.contains("name=\"force\"\r\n\r\ntrue\r\n") {
            &data["fresh"]
        } else {
            &data["reused"]
        };
        reply(200, &response.to_string())
    }));
    let file = Client::new(&url)
        .upload(
            "photo.png",
            include_bytes!("../../e2e/atago/testdata/photo.png"),
            Some("compat replacement alt"),
        )
        .unwrap();
    assert_eq!(file.comment.as_deref(), Some("compat replacement alt"));
}

#[rstest]
#[case(include_str!("../testdata/compat/misskey-blocked.json"))]
#[case(include_str!("../testdata/compat/sharkey-blocked.json"))]
fn hidden_and_refused_notes_are_permission_failures(#[case] response: &str) {
    use super::stub_tests::{reply, serve};
    let data: Value = serde_json::from_str(response).unwrap();
    let (url, _) = serve(Arc::new(move |_, _, _| {
        reply(
            data["status"].as_u64().unwrap() as u16,
            &data["body"].to_string(),
        )
    }));
    let error = Client::new(&url).note("blocked").unwrap_err();
    assert_eq!(error.kind().exit_code(), 4);
    assert!(format!("{error}").contains(&url[7..]));
}
