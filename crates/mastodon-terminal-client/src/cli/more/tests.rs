use super::*;
use crate::api::stub_tests::{Call, reply, serve};
use std::sync::Arc;

fn ctx(origin: &str) -> (tempfile::TempDir, Ctx) {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Ctx {
        origin: origin.into(),
        json: true,
        host_given: true,
        accounts: Ok(AccountStore::open(dir.path())),
        env_token: Some("test-only".into()),
    };
    (dir, ctx)
}
fn json_out(out: &[u8]) -> Value {
    serde_json::from_slice(out).unwrap()
}

#[test]
fn every_status_write_uses_its_endpoint_once_and_never_reports_a_refused_write_as_success() {
    for (action, endpoint, method) in [
        ("like", "favourite", "POST"),
        ("unlike", "unfavourite", "POST"),
        ("repost", "reblog", "POST"),
        ("unrepost", "unreblog", "POST"),
        ("bookmark", "bookmark", "POST"),
        ("unbookmark", "unbookmark", "POST"),
        ("delete", "", "DELETE"),
    ] {
        for accepted in [true, false] {
            let (origin, calls) = serve(Arc::new(move |_, _| {
                reply(
                    if accepted { 200 } else { 403 },
                    if accepted {
                        "{}"
                    } else {
                        r#"{"error":"refused"}"#
                    },
                )
            }));
            let (_dir, ctx) = ctx(&origin);
            let mut out = Vec::new();
            let result = status_action(&ctx, &mut out, "42", action);
            assert_eq!(result.is_ok(), accepted);
            if accepted {
                let v = json_out(&out);
                assert_eq!(v["action"], action);
                assert_eq!(v["target"], "42");
                assert!(v["id"].is_null());
            } else {
                assert!(out.is_empty());
            }
            let calls = calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].method, method);
            assert_eq!(
                calls[0].target,
                if endpoint.is_empty() {
                    "/api/v1/statuses/42".into()
                } else {
                    format!("/api/v1/statuses/42/{endpoint}")
                }
            );
            assert_eq!(calls[0].auth.as_deref(), Some("Bearer test-only"));
            if method == "POST" {
                assert_eq!(json_out(calls[0].body.as_bytes()), json!({}));
            }
        }
    }
}

#[test]
fn every_relationship_action_uses_the_local_account_id_and_propagates_refusal() {
    for action in ["follow", "unfollow", "mute", "unmute", "block", "unblock"] {
        for accepted in [true, false] {
            let (origin, calls) = serve(Arc::new(move |c, _| {
                if c.method == "GET" {
                    reply(200, r#"{"id":"7","acct":"bob","username":"bob"}"#)
                } else {
                    reply(
                        if accepted { 200 } else { 403 },
                        if accepted {
                            r#"{"id":"7","requested":true}"#
                        } else {
                            r#"{"error":"refused"}"#
                        },
                    )
                }
            }));
            let (_dir, ctx) = ctx(&origin);
            let mut out = Vec::new();
            assert_eq!(relation(&ctx, &mut out, "@bob", action).is_ok(), accepted);
            let calls = calls.lock().unwrap();
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0].query("acct").as_deref(), Some("bob"));
            assert_eq!(calls[1].target, format!("/api/v1/accounts/7/{action}"));
            assert_eq!(json_out(calls[1].body.as_bytes()), json!({}));
            if accepted {
                assert_eq!(json_out(&out)["action"], action);
            } else {
                assert!(out.is_empty());
            }
        }
    }
}

#[test]
fn declined_reports_and_foreign_status_writes_send_nothing() {
    let (origin, calls) = serve(Arc::new(|_, _| reply(500, "{}")));
    let (_dir, ctx) = ctx(&origin);
    let mut out = Vec::new();
    let args = ReportArgs {
        target: "42".into(),
        reason: ReportReason::Spam,
        comment: None,
        yes: false,
    };
    assert!(
        report(
            &ctx,
            &mut out,
            &args,
            &mut b"n\n".as_slice(),
            Some(&mut Vec::new())
        )
        .is_err()
    );
    assert!(report(&ctx, &mut out, &args, &mut b"y\n".as_slice(), None).is_err());
    assert!(status_action(&ctx, &mut out, "https://other.test/@bob/42", "like").is_err());
    assert!(calls.lock().unwrap().is_empty());
    assert!(out.is_empty());
}

#[test]
fn report_uses_mastodon_category_and_local_status_author() {
    let (origin, calls) = serve(Arc::new(|c, _| {
        if c.method == "GET" {
            reply(200, r#"{"id":"42","account":{"id":"7"}}"#)
        } else {
            reply(200, "{}")
        }
    }));
    let (_dir, ctx) = ctx(&origin);
    let args = ReportArgs {
        target: "42".into(),
        reason: ReportReason::Spam,
        comment: Some("unsolicited".into()),
        yes: true,
    };
    report(&ctx, &mut Vec::new(), &args, &mut b"".as_slice(), None).unwrap();
    let calls = calls.lock().unwrap();
    assert_eq!(calls[1].method, "POST");
    assert_eq!(calls[1].target, "/api/v1/reports");
    assert_eq!(
        json_out(calls[1].body.as_bytes()),
        json!({"account_id":"7","status_ids":["42"],"comment":"Spam\n-----\nunsolicited","category":"spam","forward":false})
    );
}

fn lists_server(c: &Call, _: usize) -> crate::api::stub_tests::Reply {
    match c.path() {
        "/api/v1/accounts/verify_credentials" | "/api/v1/accounts/lookup" => {
            reply(200, r#"{"id":"7","username":"bob","acct":"bob"}"#)
        }
        "/api/v1/favourites" | "/api/v1/bookmarks" | "/api/v1/timelines/list/9" => {
            reply(200, r#"[{"id":"42","content":"<p>hello</p>"}]"#)
        }
        "/api/v1/lists" => reply(200, r#"[{"id":"9","title":"friends"}]"#),
        "/api/v2/search" => reply(200, r#"{"accounts":[{"id":"7","username":"bob"}]}"#),
        _ => reply(200, r#"[{"id":"7","username":"bob"}]"#),
    }
}
#[test]
fn every_list_uses_common_json_and_the_matching_endpoint() {
    for kind in ["mutes", "blocks", "favourites", "bookmarks"] {
        let (origin, calls) = serve(Arc::new(lists_server));
        let (_d, ctx) = ctx(&origin);
        let mut out = Vec::new();
        collection(&ctx, &mut out, kind, 1).unwrap();
        assert!(json_out(&out)["id"].is_string());
        assert_eq!(calls.lock().unwrap()[0].path(), format!("/api/v1/{kind}"));
    }
    for endpoint in ["favourited_by", "reblogged_by"] {
        let (origin, calls) = serve(Arc::new(lists_server));
        let (_d, ctx) = ctx(&origin);
        let mut out = Vec::new();
        status_accounts(&ctx, &mut out, "42", endpoint, 1).unwrap();
        assert_eq!(json_out(&out)["username"], "bob");
        assert_eq!(
            calls.lock().unwrap()[0].path(),
            format!("/api/v1/statuses/42/{endpoint}")
        );
    }
    for (followers, pending, path) in [
        (true, false, "/api/v1/accounts/7/followers"),
        (false, false, "/api/v1/accounts/7/following"),
        (true, true, "/api/v1/follow_requests"),
    ] {
        let (origin, calls) = serve(Arc::new(lists_server));
        let (_d, ctx) = ctx(&origin);
        let mut out = Vec::new();
        follows(&ctx, &mut out, None, 1, followers, pending).unwrap();
        assert_eq!(json_out(&out)["id"], "7");
        assert_eq!(calls.lock().unwrap().last().unwrap().path(), path);
    }
    let (origin, calls) = serve(Arc::new(lists_server));
    let (_d, ctx) = ctx(&origin);
    let mut out = Vec::new();
    lists(&ctx, &mut out, 1).unwrap();
    assert_eq!(json_out(&out), json!({"id":"9","name":"friends"}));
    out.clear();
    list(&ctx, &mut out, "9", 1).unwrap();
    assert_eq!(json_out(&out)["text"], "hello");
    out.clear();
    search_accounts(&ctx, &mut out, "bob", 1).unwrap();
    assert_eq!(json_out(&out)["username"], "bob");
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .query("type")
            .as_deref(),
        Some("accounts")
    );
}

#[test]
fn chat_send_is_direct_and_history_does_not_mix_groups_or_public_posts() {
    let (origin, calls) = serve(Arc::new(|c, _| match c.path() {
        "/api/v1/accounts/lookup" => reply(200, r#"{"id":"7","acct":"bob","username":"bob"}"#),
        "/api/v2/instance" => reply(200, "{}"),
        "/api/v1/statuses" => reply(
            200,
            r#"{"id":"43","content":"<p>@bob hi</p>","visibility":"direct"}"#,
        ),
        "/api/v1/conversations" => reply(
            200,
            r#"[{"id":"1","accounts":[{"id":"7"}],"last_status":{"id":"42","visibility":"direct","account":{"id":"7","acct":"bob"}}},{"id":"2","accounts":[{"id":"7"},{"id":"8"}],"last_status":{"id":"99","visibility":"direct"}}]"#,
        ),
        "/api/v1/accounts/verify_credentials" => reply(200, r#"{"id":"1","acct":"me"}"#),
        "/api/v1/statuses/42/context" => reply(
            200,
            r#"{"ancestors":[{"id":"40","visibility":"public"},{"id":"41","visibility":"direct","account":{"id":"7","acct":"bob"}}]}"#,
        ),
        _ => reply(200, "[]"),
    }));
    let (_d, ctx) = ctx(&origin);
    let mut out = Vec::new();
    chat(
        &ctx,
        &mut out,
        Some("bob"),
        Some("-"),
        20,
        &mut b"hi\n".as_slice(),
    )
    .unwrap();
    assert_eq!(json_out(&out)["id"], "43");
    let calls_now = calls.lock().unwrap();
    let write = calls_now.iter().find(|c| c.method == "POST").unwrap();
    assert_eq!(
        json_out(write.body.as_bytes()),
        json!({"status":"@bob hi","visibility":"direct"})
    );
    assert!(write.idempotency.is_some());
    drop(calls_now);
    out.clear();
    chat(&ctx, &mut out, Some("bob"), None, 20, &mut b"".as_slice()).unwrap();
    let ids: Vec<_> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap()["id"].clone())
        .collect();
    assert_eq!(ids, vec![json!("41"), json!("42")]);
    assert!(
        !calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.path() == "/api/v1/statuses/99/context")
    );
}

#[test]
fn refused_report_chat_and_marker_writes_print_no_success() {
    let (origin, calls) = serve(Arc::new(|c, _| {
        if c.method == "POST" {
            return reply(403, r#"{"error":"refused"}"#);
        }
        match c.path() {
            "/api/v1/accounts/lookup" => reply(200, r#"{"id":"7","acct":"bob"}"#),
            "/api/v1/statuses/42" => reply(200, r#"{"id":"42","account":{"id":"7"}}"#),
            "/api/v1/notifications" => reply(200, r#"[{"id":"42"}]"#),
            _ => reply(200, "{}"),
        }
    }));
    let (_dir, ctx) = ctx(&origin);
    let mut out = Vec::new();
    assert!(
        report(
            &ctx,
            &mut out,
            &ReportArgs {
                target: "42".into(),
                reason: ReportReason::Other,
                comment: None,
                yes: true
            },
            &mut b"".as_slice(),
            None
        )
        .is_err()
    );
    assert!(
        chat(
            &ctx,
            &mut out,
            Some("bob"),
            Some("hi"),
            1,
            &mut b"".as_slice()
        )
        .is_err()
    );
    assert!(notifications(&ctx, &mut out, 1, true).is_err());
    assert!(out.is_empty());
    let calls = calls.lock().unwrap();
    let paths: Vec<_> = calls
        .iter()
        .filter(|c| c.method == "POST")
        .map(Call::path)
        .collect();
    assert_eq!(
        paths,
        ["/api/v1/reports", "/api/v1/statuses", "/api/v1/markers"]
    );
}
