use super::*;
use crate::api::stub_tests::{reply, serve};
use crate::api::types::Limits;
use rstest::rstest;
use serde_json::json;

#[rstest]
#[case("v1/bookmarks", "3.0.6", "3.1.0")]
#[case("v1/statuses/42/bookmark", "3.0.6", "3.1.0")]
#[case("v1/statuses/42/unbookmark", "3.0.6", "3.1.0")]
#[case("v1/markers", "2.9.4", "3.0.0")]
#[case("v1/notifications/unread_count", "4.2.9", "4.3.0")]
#[case("v1/conversations", "2.5.9", "2.6.0")]
#[case("v1/conversations/42/read", "2.5.9", "2.6.0")]
#[case("v1/lists", "2.0.9", "2.1.0")]
#[case("v1/lists/42", "2.0.9", "2.1.0")]
#[case("v1/timelines/list/42", "2.0.9", "2.1.0")]
fn a_missing_route_needs_evidence_of_an_older_release(
    #[case] path: &str,
    #[case] old: &str,
    #[case] introduced: &str,
) {
    for (version, expected) in [
        (old, Kind::Unsupported),
        (introduced, Kind::Api),
        ("unknown", Kind::Api),
    ] {
        let instance = json!({"version": version}).to_string();
        let (origin, calls) = serve(Arc::new(move |c, _| {
            if c.path() == "/api/v2/instance" {
                reply(200, &instance)
            } else {
                reply(404, r#"{"error":"Record not found"}"#)
            }
        }));
        // This annotation also checks that the command uses terminal-kit's type.
        let error: terminal_kit::error::Error =
            Client::new(&origin).get_one(path, &[]).unwrap_err();
        assert_eq!(error.kind(), expected);
        assert_eq!(
            error.kind().exit_code(),
            if expected == Kind::Unsupported { 5 } else { 4 }
        );
        let calls = calls.lock().unwrap();
        assert_eq!(
            calls
                .iter()
                .filter(|c| c.path() == format!("/api/{path}"))
                .count(),
            1
        );
        assert_eq!(
            calls
                .iter()
                .filter(|c| c.path() == "/api/v2/instance")
                .count(),
            1
        );
        assert!(calls.iter().all(|c| c.method == "GET"));
    }
}

#[rstest]
#[case(401)]
#[case(403)]
#[case(422)]
#[case(500)]
#[case(501)]
fn only_not_implemented_is_an_explicit_capability_refusal(#[case] status: u16) {
    let (origin, calls) = serve(Arc::new(move |_, _| {
        reply(status, r#"{"error":"refused"}"#)
    }));
    let error = Client::new(&origin)
        .post("v1/statuses/42/favourite", &json!({}), None)
        .unwrap_err();
    assert_eq!(
        error.kind(),
        if status == 501 {
            Kind::Unsupported
        } else {
            Kind::Api
        }
    );
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.method == "POST")
            .count(),
        1,
        "writes must never be retried"
    );
}

#[test]
fn missing_objects_and_unavailable_metadata_remain_api_errors() {
    let (origin, calls) = serve(Arc::new(|_, _| {
        reply(404, r#"{"error":"Record not found"}"#)
    }));
    let client = Client::new(&origin);
    assert_eq!(client.status("42").unwrap_err().kind(), Kind::Api);
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(
        client.get_one("v1/bookmarks", &[]).unwrap_err().kind(),
        Kind::Api
    );
    assert_eq!(
        calls.lock().unwrap().len(),
        5,
        "v1 instance fallback must terminate"
    );
}

#[test]
fn old_servers_use_v1_instance_without_retrying_the_write() {
    let (origin, calls) = serve(Arc::new(|c, _| match c.path() {
        "/api/v1/instance" => reply(200, r#"{"version":"2.9.4"}"#),
        _ => reply(404, r#"{"error":"Record not found"}"#),
    }));
    let error = Client::new(&origin)
        .post(
            "v1/markers",
            &json!({"notifications":{"last_read_id":"42"}}),
            None,
        )
        .unwrap_err();
    assert_eq!(error.kind(), Kind::Unsupported);
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.method == "POST")
            .count(),
        1
    );
}

#[rstest]
#[case(
    r#"{"version":"4.4.3"}"#,
    r#"{"id":"7","source":{}}"#,
    Some(Kind::Unsupported)
)]
#[case(
    r#"{"version":"4.5.0+glitch"}"#,
    r#"{"id":"7","source":{"quote_policy":"public"}}"#,
    None
)]
#[case(
    r#"{"version":"5.0.0"}"#,
    r#"{"id":"7","source":{"quote_policy":"public"}}"#,
    None
)]
#[case(
    r#"{"api_versions":{"mastodon":7}}"#,
    r#"{"id":"7","source":{"quote_policy":"public"}}"#,
    None
)]
#[case(
    r#"{"api_versions":{"mastodon":6}}"#,
    r#"{"id":"7","source":{}}"#,
    Some(Kind::Unsupported)
)]
#[case(r#"{}"#, "{}", Some(Kind::Api))]
#[case(
    r#"{"api_versions":{"mastodon":7}}"#,
    r#"{"id":"7","source":{}}"#,
    Some(Kind::Unsupported)
)]
#[case(
    r#"{"version":"unknown"}"#,
    r#"{"id":"7","source":{"quote_policy":"public"}}"#,
    None
)]
fn quote_support_probes_fields_on_old_new_and_unknown_instances(
    #[case] body: &str,
    #[case] account: &str,
    #[case] expected: Option<Kind>,
) {
    let body = body.to_owned();
    let account = account.to_owned();
    let (origin, calls) = serve(Arc::new(move |c, _| match c.path() {
        "/api/v2/instance" => reply(200, &body),
        "/api/v1/accounts/verify_credentials" => reply(200, &account),
        _ => reply(404, "{}"),
    }));
    assert_eq!(
        Client::new(&origin)
            .require_quotes(None)
            .err()
            .map(|e| e.kind()),
        expected
    );
    assert!(calls.lock().unwrap().iter().all(|c| c.method == "GET"));
}

#[test]
fn instance_authentication_failure_does_not_fall_back() {
    let (origin, calls) = serve(Arc::new(|_, _| reply(403, r#"{"error":"forbidden"}"#)));
    assert_eq!(
        Client::new(&origin).instance().unwrap_err().kind(),
        Kind::Api
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn only_endpoint_absence_is_unsupported() {
    let (origin, _) = serve(Arc::new(|c, _| {
        if c.path() == "/api/v2/instance" {
            reply(
                200,
                r#"{"version":"4.2.0 (compatible; GoToSocial 0.19.0)"}"#,
            )
        } else {
            reply(404, "{}")
        }
    }));
    let client = Client::new(&origin);
    for path in [
        "v1/timelines/public",
        "v2/media",
        "v2/search",
        "v1/conversations",
        "v1/lists",
        "v1/followed_tags",
        "v1/accounts/update_credentials",
    ] {
        assert!(client.capability_refusal(path, 404).is_some());
        assert!(client.capability_refusal(path, 501).is_some());
        for status in [400, 401, 403, 422, 429, 500, 502, 503] {
            assert!(client.capability_refusal(path, status).is_none());
        }
    }
    for path in [
        // Creation and reports can reference missing/hidden resources too.
        "v1/statuses",
        "v1/reports",
        "v1/statuses/123",
        "v1/statuses/123/context",
        "v1/statuses/123/favourite",
        "v1/accounts/lookup",
        "v1/accounts/123",
        "v1/timelines/tag/unknown",
        "v1/media/123",
    ] {
        assert!(client.capability_refusal(path, 404).is_none());
        assert!(client.capability_refusal(path, 501).is_some());
    }
}

#[test]
fn discovery_stays_on_the_origin_and_recognizes_both_schemas() {
    for version in ["2.0", "2.1"] {
        for url in [
            "https://other.test/nodeinfo",
            "https://m.test.evil/nodeinfo",
            "https://m.test@evil/nodeinfo",
            "http://m.test/nodeinfo",
        ] {
            let d = json!({"links": [{"rel": format!("http://nodeinfo.diaspora.software/ns/schema/{version}"), "href": url}]});
            assert_eq!(nodeinfo_url("https://m.test", &d), None);
        }
        let d = json!({"links": [{"rel": format!("http://nodeinfo.diaspora.software/ns/schema/{version}"), "href": "https://m.test/nodeinfo/2.0"}]});
        assert_eq!(
            nodeinfo_url("https://m.test", &d),
            Some("https://m.test/nodeinfo/2.0")
        );
    }
    assert_eq!(nodeinfo_url("https://m.test", &json!({})), None);
}

#[test]
fn documented_shapes_keep_real_software_separate_from_compatibility_versions() {
    let fixtures: Value =
        serde_json::from_str(include_str!("../../../testdata/compat/servers.json")).unwrap();
    for fixture in fixtures.as_array().unwrap() {
        assert_eq!(
            server_label("127.0.0.1", &fixture["instance"], &fixture["nodeinfo"]),
            fixture["label"]
        );
        assert_eq!(
            Limits::from_instance(&fixture["instance"]).max_characters,
            fixture["characters"].as_u64().unwrap() as usize
        );
    }
    assert_eq!(
        server_label("127.0.0.1", &json!({}), &Value::Null),
        "127.0.0.1"
    );
    assert_eq!(
        Limits::from_instance(
            &json!({"configuration": {"statuses": {"max_media_attachments": 0}}})
        )
        .max_media_attachments,
        0
    );
}

#[test]
fn refused_public_metadata_cannot_establish_an_old_release() {
    let (origin, calls) = serve(Arc::new(|c, _| match c.path() {
        "/api/v2/instance" => reply(403, r#"{"error":"forbidden"}"#),
        "/api/v1/instance" => reply(200, r#"{"version":"2.0.0"}"#),
        _ => reply(404, "{}"),
    }));
    let error = Client::new(&origin)
        .with_token(Some("private-test-token".into()))
        .get_one("v1/bookmarks", &[])
        .unwrap_err();
    assert_eq!(error.kind(), Kind::Api);
    let calls = calls.lock().unwrap();
    assert!(!calls.iter().any(|c| c.path() == "/api/v1/instance"));
    assert!(calls[1..].iter().all(|c| c.auth.is_none()));
}

#[test]
fn nodeinfo_prevents_treating_another_implementations_release_as_mastodon() {
    let address = Arc::new(std::sync::Mutex::new(String::new()));
    let server_address = Arc::clone(&address);
    let (origin, calls) = serve(Arc::new(move |c, _| match c.path() {
        "/api/v2/instance" => reply(200, r#"{"version":"2.0.0"}"#),
        "/.well-known/nodeinfo" => reply(
            200,
            &json!({"links":[{
                "rel":"http://nodeinfo.diaspora.software/ns/schema/2.0",
                "href":format!("{}/nodeinfo/2.0", server_address.lock().unwrap())
            }]})
            .to_string(),
        ),
        "/nodeinfo/2.0" => reply(200, r#"{"software":{"name":"pleroma","version":"2.0.0"}}"#),
        _ => reply(404, "{}"),
    }));
    *address.lock().unwrap() = origin.clone();
    let client = Client::new(&origin).with_token(Some("private-test-token".into()));
    assert_eq!(
        client
            .get_one("v1/statuses/42/bookmark", &[])
            .unwrap_err()
            .kind(),
        Kind::Api
    );
    let error = client.get_one("v1/lists", &[]).unwrap_err();
    assert_eq!(error.kind(), Kind::Unsupported);
    assert!(error.message().contains("pleroma 2.0.0"));
    assert!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| !c.path().contains("bookmark") && c.path() != "/api/v1/lists")
            .all(|c| c.auth.is_none())
    );
}
