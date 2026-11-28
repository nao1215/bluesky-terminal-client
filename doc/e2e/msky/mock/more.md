# atago Behavior Specs
## Summary
1 suite · 5 scenarios
## Contents
- [msky more commands](#msky-more-commands): 5 scenarios
  - [thread, profile, reactions and renotes read what the note and account have](#scenario-thread-profile-reactions-and-renotes-read-what-the-note-and-account-have)
  - [like, repost, delete, mute and block write once each, as the token's account](#scenario-like-repost-delete-mute-and-block-write-once-each-as-the-tokens-account)
  - [unlike, unrepost, unmute and unblock take back once each, and renotes and blocks list what they read](#scenario-unlike-unrepost-unmute-and-unblock-take-back-once-each-and-renotes-and-blocks-list-what-they-read)
  - [chat lists the latest message of each conversation, newest first, asking for accounts and rooms](#scenario-chat-lists-the-latest-message-of-each-conversation-newest-first-asking-for-accounts-and-rooms)
  - [report sends nothing until confirmed, and with --yes reports the note's author with its address](#scenario-report-sends-nothing-until-confirmed-and-with---yes-reports-the-notes-author-with-its-address)

## msky more commands
Source: `crates/misskey-terminal-client/e2e/atago/more.atago.yaml`

Secrets declared: `e2e-env-token-0501`.
### Scenario: thread, profile, reactions and renotes read what the note and account have
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky thread n --json --host ${api.url}
msky likes n --host ${api.url}
msky profile @bob@x.test --host ${api.url}
```

#### Then
- after `msky thread n --json --host ${api.url}`:
  - exit code is `0`
  - stdout matches `/^\{[^\n]*"id":"p"[^\n]*\}\n\{[^\n]*"id":"n"[^\n]*\}\n\{[^\n]*"id":"c"[^\n]*\}\n$/`
- after `msky likes n --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
- after `msky profile @bob@x.test --host ${api.url}`:
  - exit code is `0`
  - stdout contains `ボブ @bob@x.test`, `3 notes`, `こんにちは`

#### Expected output
_expected stdout:_

```text
:blobcat:  @ann@127.0.0.1:${api.port}  ${api.url}/@ann
```

### Scenario: like, repost, delete, mute and block write once each, as the token's account
#### Given
- Stub HTTP server `api` serves 8 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky like n --reaction 👍🏽 --host ${api.url}
msky repost n --json --host ${api.url}
msky delete n --host ${api.url}
msky mute @bob@x.test --host ${api.url}
msky block bob@x.test --json --host ${api.url}
msky mutes --host ${api.url}
```

#### Then
- after `msky like n --reaction 👍🏽 --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/api/notes/reactions/create` exactly 1 time(s)
- after `msky repost n --json --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
- after `msky delete n --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
- after `msky mute @bob@x.test --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
- after `msky block bob@x.test --json --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/api/blocking/create` exactly 1 time(s)
- after `msky mutes --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value

#### Expected output
_expected stdout:_

```text
reacted 👍🏽 to ${api.url}/notes/n
```

_expected stdout:_

```text
{"action":"repost","already":false,"id":"rn1","target":"n","url":"${api.url}/notes/rn1"}
```

_expected stdout:_

```text
deleted ${api.url}/notes/n
```

_expected stdout:_

```text
muted @bob@x.test
```

_expected stdout:_

```text
{"action":"block","already":false,"id":null,"target":"u-bob","url":null}
```

_expected stdout:_

```text
ボブ @bob@x.test  ${api.url}/@bob@x.test
```

### Scenario: unlike, unrepost, unmute and unblock take back once each, and renotes and blocks list what they read
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky unlike n --host ${api.url}
msky unrepost n --json --host ${api.url}
msky unmute @bob@x.test --host ${api.url}
msky unblock bob@x.test --json --host ${api.url}
msky renotes n --json --host ${api.url}
msky blocks --host ${api.url}
```

#### Then
- after `msky unlike n --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/api/notes/reactions/delete` exactly 1 time(s)
- after `msky unrepost n --json --host ${api.url}`:
  - exit code is `0`
  - stdout at `$.action` equals `unrepost`; at `$.target` equals `n`
  - mock `api` received `/api/notes/unrenote` exactly 1 time(s)
- after `msky unmute @bob@x.test --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/api/mute/delete` exactly 1 time(s)
- after `msky unblock bob@x.test --json --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/api/blocking/delete` exactly 1 time(s)
- after `msky renotes n --json --host ${api.url}`:
  - exit code is `0`
  - stdout at `$.id` equals `rn1`
  - mock `api` received `/api/notes/renotes`
- after `msky blocks --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value

#### Expected output
_expected stdout:_

```text
took back the reaction to ${api.url}/notes/n
```

_expected stdout:_

```text
unmuted @bob@x.test
```

_expected stdout:_

```text
{"action":"unblock","already":false,"id":null,"target":"u-bob","url":null}
```

_expected stdout:_

```text
ボブ @bob@x.test  ${api.url}/@bob@x.test
```

### Scenario: chat lists the latest message of each conversation, newest first, asking for accounts and rooms
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_TOKEN, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky chat -n 2 --host ${api.url}
```

#### Then
- exit code is `0`
- stdout equals an exact value
- mock `api` received `/api/chat/history` where body exactly 1 time(s)
- mock `api` received `/api/chat/history` where body exactly 1 time(s)

#### Expected output
_expected stdout:_

```text
@bob@x.test · 2026-11-26 10:00
返事 👨‍👩‍👧
@bob@x.test · 2026-11-26 10:00
返事 👨‍👩‍👧
```

### Scenario: report sends nothing until confirmed, and with --yes reports the note's author with its address
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky report n9 --reason spam --host ${api.url}
msky report n9 --reason spam --comment "宣伝ばかり 📢" --yes --host ${api.url}
```

#### Then
- after `msky report n9 --reason spam --host ${api.url}`:
  - exit code is `1`
  - stderr contains `a report is sent only once confirmed`, `--yes`
  - mock `api` received `/api/users/report-abuse` exactly 0 time(s)
- after `msky report n9 --reason spam --comment "宣伝ばかり 📢" --yes --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/api/users/report-abuse` exactly 1 time(s)

#### Expected output
_expected stdout:_

```text
reported @bob@127.0.0.1:${api.port} for ${api.url}/notes/n9
```

