# atago Behavior Specs
## Summary
1 suite · 2 scenarios
## Contents
- [msky notifications](#msky-notifications): 2 scenarios
  - [notifications say who did what to which note, and leave them unread](#scenario-notifications-say-who-did-what-to-which-note-and-leave-them-unread)
  - [notifications without a login are a usage error before anything is asked](#scenario-notifications-without-a-login-are-a-usage-error-before-anything-is-asked)

## msky notifications
Source: `crates/misskey-terminal-client/e2e/atago/notifications.atago.yaml`

Secrets declared: `e2e-fake-token-0001`.
### Scenario: notifications say who did what to which note, and leave them unread
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Environment variables are set: MSKY_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-alice", "username": "alice", "token": "e2e-fake-token-0001"}
```

#### When

```shell
msky notifications -n 4 --host ${api.url}
msky notifications -n 10 --json --host ${api.url}
msky notifications -n 1 --json --host ${api.url}
```

#### Then
- after `msky notifications -n 4 --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - stderr is empty
  - mock `api` received `/api/i/notifications` exactly 1 time(s)
- after `msky notifications -n 10 --json --host ${api.url}`:
  - exit code is `0`
  - stdout matches `/^(\{[^\n]*\}\n){4}$/`
  - mock `api` received `/api/i/notifications` exactly 3 time(s)
- after `msky notifications -n 1 --json --host ${api.url}`:
  - exit code is `0`
  - stdout at `$.id` equals `n4`; at `$.type` equals `like`; at `$.reaction` equals `:blobcat@.:`; at `$.user.username` equals `bob`; at `$.user.host` equals `misskey.design`; at `$.postId` equals `9a`; at `$.post.url` equals `${api.url}/notes/9a`; at `$.post.user.host` equals `127.0.0.1:${api.port}`; at `$.post.files[0].type` equals `image`; at `$.post.files[0].mime` equals `image/png`

#### Expected output
_expected stdout:_

```text
ボブ 🐈‍⬛ @bob@misskey.design reacted :blobcat: to your note · 2026-11-26 10:00
朝ごはん 🍙
[1 picture]
${api.url}/notes/9a

キャロル🇰🇷 @carol replied to you · 2026-11-26 09:00
いいね 👍🏽
二行目
${api.url}/notes/9b

@dave@other.example followed you · 2026-11-26 08:00
${api.url}/@dave@other.example

achievement earned: notes1 · 2026-11-26 07:00
```

### Scenario: notifications without a login are a usage error before anything is asked
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky notifications --host ${api.url}
```

#### Then
- exit code is `1`
- stdout is empty
- stderr equals an exact value
- mock `api` received a request exactly 0 time(s)

#### Expected output
_expected stderr:_

```text
error: not logged in on 127.0.0.1:${api.port}
hint: run `msky login --host 127.0.0.1:${api.port}`
```

