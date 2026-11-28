# atago Behavior Specs
## Summary
1 suite · 7 scenarios
## Contents
- [mstdn read](#mstdn-read): 7 scenarios
  - [author prints the account's posts as text, the HTML read back, emoji and CJK whole](#scenario-author-prints-the-accounts-posts-as-text-the-html-read-back-emoji-and-cjk-whole)
  - [status, thread and profile read one post, its thread and an account, an address on its own server](#scenario-status-thread-and-profile-read-one-post-its-thread-and-an-account-an-address-on-its-own-server)
  - [the local and public timelines are read as a visitor, home needs a login](#scenario-the-local-and-public-timelines-are-read-as-a-visitor-home-needs-a-login)
  - [search reads a hashtag as a visitor and asks for a login for words](#scenario-search-reads-a-hashtag-as-a-visitor-and-asks-for-a-login-for-words)
  - [notifications are read as the token's account, newest first, page by page](#scenario-notifications-are-read-as-the-tokens-account-newest-first-page-by-page)
  - [what the server refuses is exit 4 with its words, and a rate limit is waited for](#scenario-what-the-server-refuses-is-exit-4-with-its-words-and-a-rate-limit-is-waited-for)
  - [a pipe closed early ends the output quietly with exit 0](#scenario-a-pipe-closed-early-ends-the-output-quietly-with-exit-0)

## mstdn read
Source: `crates/mastodon-terminal-client/e2e/atago/read.atago.yaml`
### Scenario: author prints the account's posts as text, the HTML read back, emoji and CJK whole
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn author @alice -n 2 --host ${api.url}
mstdn author alice -n 4 --filter media --reposts --json --host ${api.url}
mstdn author alice -n 1 --json --host ${api.url}
```

#### Then
- after `mstdn author @alice -n 2 --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - stderr is empty
  - mock `api` received `GET /api/v1/accounts/lookup` exactly 1 time(s)
  - mock `api` received `/api/v1/accounts/7/statuses` exactly 1 time(s)
  - mock `api` received `/api/v1/accounts/7/statuses`
  - mock `api` received `/api/v1/accounts/7/statuses`
- after `mstdn author alice -n 4 --filter media --reposts --json --host ${api.url}`:
  - exit code is `0`
  - stdout matches `/^\{[^\n]*"id":"102"[^\n]*\}\n\{[^\n]*"id":"101"[^\n]*\}\n$/`
  - mock `api` received `/api/v1/accounts/7/statuses` exactly 3 time(s)
  - mock `api` received `/api/v1/accounts/7/statuses`
  - mock `api` received `/api/v1/accounts/7/statuses`
- after `mstdn author alice -n 1 --json --host ${api.url}`:
  - exit code is `0`
  - stdout at `$.id` equals `102`; at `$.url` equals `${api.url}/@alice/102`; at `$.user.host` equals `127.0.0.1:${api.port}`; at `$.text` equals `"山の写真 🇯🇵👍🏽\n二行目 مرحبا\n\n#rust :blobcat: https://example.com/a/very/long/path & the article (https://other.test/post)"`; at `$.files[0].description` equals `山 🏔️`; at `$.likeCount` equals `6`

#### Expected output
_expected stdout:_

```text
家族👨‍👩‍👧 アリス @alice · 2026-09-20 10:00
山の写真 🇯🇵👍🏽
二行目 مرحبا

#rust :blobcat: https://example.com/a/very/long/path & the article (https://other.test/post)
[1 picture]
♡ 6  ⟳ 1  ↩ 4
${api.url}/@alice/102

家族👨‍👩‍👧 アリス @alice · 2026-09-20 09:00
CW: ネタバレ
結末は…
♡ 0  ⟳ 0  ↩ 0
${api.url}/@alice/101
```

### Scenario: status, thread and profile read one post, its thread and an account, an address on its own server
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Stub HTTP server `far` serves 1 canned route(s) at `${far.url}` and records every request (#24).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn status ${api.url}/@bob/200 --host ${far.url}
mstdn note ${far.url}/users/dave/statuses/9 --json
mstdn thread 200 --json --host ${api.url}
mstdn profile @bob --host ${api.url}
```

#### Then
- after `mstdn status ${api.url}/@bob/200 --host ${far.url}`:
  - exit code is `0`
  - stdout equals an exact value
- after `mstdn note ${far.url}/users/dave/statuses/9 --json`:
  - exit code is `0`
  - stdout at `$.text` equals `よそ`
- after `mstdn thread 200 --json --host ${api.url}`:
  - exit code is `0`
  - stdout matches `/^\{"content":"<p>質問</p>".*"id":"199".*\n\{.*"id":"200".*\n\{.*"id":"201".*\n$/`
- after `mstdn profile @bob --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value

#### Expected output
_expected stdout:_

```text
@bob · 2026-11-20 10:00
本文 🐈‍⬛
♡ 0  ⟳ 0  ↩ 0
${api.url}/@bob/200
```

_expected stdout:_

```text
ボブ @bob@127.0.0.1:${api.port}
${api.url}/@bob
1 followers  2 following  3 posts

こんにちは
二行目
```

### Scenario: the local and public timelines are read as a visitor, home needs a login
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn timeline local -n 1 --json --host ${api.url}
mstdn tl federated -n 1 --host ${api.url}
mstdn timeline --host ${api.url}
```

#### Then
- after `mstdn timeline local -n 1 --json --host ${api.url}`:
  - exit code is `0`
  - stdout at `$.text` equals `公開`
  - mock `api` received `/api/v1/timelines/public` exactly 1 time(s)
- after `mstdn tl federated -n 1 --host ${api.url}`:
  - exit code is `0`
- after `mstdn timeline --host ${api.url}`:
  - exit code is `1`
  - stderr equals an exact value
  - mock `api` received `/api/v1/timelines/public` exactly 2 time(s)

#### Expected output
_expected stderr:_

```text
error: not logged in on 127.0.0.1:${api.port}
hint: run `mstdn login --host 127.0.0.1:${api.port}`
```

### Scenario: search reads a hashtag as a visitor and asks for a login for words
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn search "#rust" -n 1 --json --host ${api.url}
mstdn search 猫 --json --host ${api.url}
```

#### Then
- after `mstdn search "#rust" -n 1 --json --host ${api.url}`:
  - exit code is `0`
  - stdout at `$.text` equals `#rust 1.90`
- after `mstdn search 猫 --json --host ${api.url}`:
  - exit code is `1`
  - stdout at `$.error` equals `127.0.0.1:${api.port} searches the text of posts only for an account that is logged in`; at `$.status` equals `1`
  - mock `api` received `/api/v2/search` exactly 0 time(s)

### Scenario: notifications are read as the token's account, newest first, page by page
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_HOST, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_HOST, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn notifications -n 2 --json
mstdn notif -n 1
```

#### Then
- after `mstdn notifications -n 2 --json`:
  - exit code is `0`
  - stdout matches `/^\{[^\n]*"id":"31"[^\n]*\}\n\{[^\n]*"id":"30"[^\n]*\}\n$/`
  - mock `api` received `/api/v1/notifications` exactly 1 time(s)
- after `mstdn notif -n 1`:
  - exit code is `0`
  - stdout contains `ボブ 🇯🇵`, `本文 🐈‍⬛`

### Scenario: what the server refuses is exit 4 with its words, and a rate limit is waited for
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn status 404 --host ${api.url}
mstdn status 429 --host ${api.url}
mstdn status ../../api/v1/x --host ${api.url}
```

#### Then
- after `mstdn status 404 --host ${api.url}`:
  - exit code is `4`
  - stderr equals an exact value
- after `mstdn status 429 --host ${api.url}`:
  - exit code is `4`
  - stderr contains `asks to wait; trying again in 0 s`, `"error: v1/statuses/429 failed: Too many requests\nhint: 127.0.0.1:${api.port} limits how often it is asked; wait a while and try again"`
  - mock `api` received `/api/v1/statuses/429` exactly 4 time(s)
- after `mstdn status ../../api/v1/x --host ${api.url}`:
  - exit code is `1`
  - stderr contains `is not a post`

#### Expected output
_expected stderr:_

```text
error: v1/statuses/404 failed: Record not found
hint: check the name or the ID, and that it is on 127.0.0.1:${api.port} (or pass --host)
```

### Scenario: a pipe closed early ends the output quietly with exit 0
_skipped on Windows_
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
{ mstdn timeline local -n 100 --json --host ${api.url}; echo "mstdn=$?" >&2; } | head -c 1
{ mstdn timeline local -n 100 --host ${api.url}; echo "mstdn=$?" >&2; } | head -c 1
```

#### Then
- after `{ mstdn timeline local -n 100 --json --host ${api.url}; echo "mstdn=$?" >&2; } | head -c 1`:
  - stderr contains `mstdn=0`, does not contain `panicked`, `Broken pipe`
- after `{ mstdn timeline local -n 100 --host ${api.url}; echo "mstdn=$?" >&2; } | head -c 1`:
  - stderr contains `mstdn=0`, does not contain `Broken pipe`
