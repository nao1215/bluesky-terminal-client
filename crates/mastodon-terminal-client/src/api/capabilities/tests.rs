use super::*;
use crate::api::stub_tests::{reply, serve};
use rstest::rstest;

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
        assert_eq!(calls.lock().unwrap().len(), 2);
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
        calls.lock().unwrap().len(),
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
        4,
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
#[case(r#"{"version":"4.4.3"}"#, Some(Kind::Unsupported))]
#[case(r#"{"version":"4.5.0+glitch"}"#, None)]
#[case(r#"{"version":"5.0.0"}"#, None)]
#[case(r#"{"api_versions":{"mastodon":7}}"#, None)]
#[case(r#"{"api_versions":{"mastodon":6}}"#, Some(Kind::Unsupported))]
#[case(r#"{}"#, Some(Kind::Api))]
fn quote_support_uses_advertised_capabilities(#[case] body: &str, #[case] expected: Option<Kind>) {
    let body = body.to_owned();
    let (origin, _) = serve(Arc::new(move |_, _| reply(200, &body)));
    assert_eq!(
        Client::new(&origin)
            .posting_limits(true)
            .err()
            .map(|e| e.kind()),
        expected
    );
}

#[test]
fn instance_authentication_failure_does_not_fall_back() {
    let (origin, calls) = serve(Arc::new(|_, _| reply(403, r#"{"error":"forbidden"}"#)));
    assert_eq!(
        Client::new(&origin)
            .posting_limits(true)
            .unwrap_err()
            .kind(),
        Kind::Api
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
}
