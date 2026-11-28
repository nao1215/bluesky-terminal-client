# atago Behavior Specs
## Summary
1 suite · 6 scenarios
## Contents
- [msky timeline](#msky-timeline): 6 scenarios
  - [timeline reads home with the saved token, page by page, each note once](#scenario-timeline-reads-home-with-the-saved-token-page-by-page-each-note-once)
  - [social is the hybrid timeline, and local and global are read as a visitor without a login](#scenario-social-is-the-hybrid-timeline-and-local-and-global-are-read-as-a-visitor-without-a-login)
  - [home and social without a login are usage errors before anything is asked](#scenario-home-and-social-without-a-login-are-usage-errors-before-anything-is-asked)
  - [a timeline the server keeps from visitors says so](#scenario-a-timeline-the-server-keeps-from-visitors-says-so)
  - [an error the server writes in another shape is said, not only its status](#scenario-an-error-the-server-writes-in-another-shape-is-said-not-only-its-status)
  - [a pipe closed early ends the output quietly with exit 0](#scenario-a-pipe-closed-early-ends-the-output-quietly-with-exit-0)

## msky timeline
Source: `crates/misskey-terminal-client/e2e/atago/timeline.atago.yaml`

Secrets declared: `e2e-fake-token-0001`.
### Scenario: timeline reads home with the saved token, page by page, each note once
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Environment variables are set: MSKY_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-alice", "username": "alice", "token": "e2e-fake-token-0001"}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "127.0.0.1:${api.port}"}
```

#### When

```shell
msky timeline -n 3
msky timeline home -n 1 --reposts --json
```

#### Then
- after `msky timeline -n 3`:
  - exit code is `0`
  - stdout equals an exact value
  - stderr is empty
  - mock `api` received `/api/notes/timeline` exactly 2 time(s)
- after `msky timeline home -n 1 --reposts --json`:
  - exit code is `0`
  - stdout at `$.id` equals `9c`; at `$.url` equals `${api.url}/notes/9c`; at `$.user.host` equals `misskey.design`; at `$.text` equals `"おはよう 🇯🇵👍🏽\n二行目 مرحبا"`; at `$.likeCount` equals `3`; at `$.repostOf` equals `null`
  - mock `api` received `/api/notes/timeline` exactly 3 time(s)

#### Expected output
_expected stdout:_

```text
ボブ 👨‍👩‍👧 @bob@misskey.design · 2026-11-26 10:00
おはよう 🇯🇵👍🏽
二行目 مرحبا
👍 2  :blobcat: 1
♡ 3  ⟳ 1  ↩ 3
${api.url}/notes/9c

@carol · 2026-11-26 09:00
CW: ネタバレ
結末
♡ 0  ⟳ 0  ↩ 0
${api.url}/notes/9b
```

### Scenario: social is the hybrid timeline, and local and global are read as a visitor without a login
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
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
msky timeline social -n 1 --host ${api.url}
msky timeline local -n 1 --host ${api.url}
msky timeline global -n 1 --host ${api.url} --json
```

#### Then
- after `msky timeline social -n 1 --host ${api.url}`:
  - exit code is `0`
  - stdout contains `ソーシャル`
  - mock `api` received `/api/notes/hybrid-timeline`
- after `msky timeline local -n 1 --host ${api.url}`:
  - exit code is `0`
  - stdout contains `ローカル 🏠`
  - mock `api` received `/api/notes/local-timeline`
- after `msky timeline global -n 1 --host ${api.url} --json`:
  - exit code is `0`
  - stdout at `$.user.host` equals `other.example`

### Scenario: home and social without a login are usage errors before anything is asked
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky timeline --host ${api.url}
msky timeline social --host ${api.url} --json
msky timeline yesterday --host ${api.url}
```

#### Then
- after `msky timeline --host ${api.url}`:
  - exit code is `1`
  - stderr equals an exact value
- after `msky timeline social --host ${api.url} --json`:
  - exit code is `1`
  - stdout at `$.error` equals `not logged in on 127.0.0.1:${api.port}`; at `$.status` equals `1`
- after `msky timeline yesterday --host ${api.url}`:
  - exit code is `1`
  - stderr contains `invalid value 'yesterday'`
  - mock `api` received a request exactly 0 time(s)

#### Expected output
_expected stderr:_

```text
error: not logged in on 127.0.0.1:${api.port}
hint: run `msky login --host 127.0.0.1:${api.port}`
```

### Scenario: a timeline the server keeps from visitors says so
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky timeline local --host ${api.url}
```

#### Then
- exit code is `4`
- stderr equals an exact value

#### Expected output
_expected stderr:_

```text
error: notes/local-timeline failed: LTL_DISABLED: Local timeline has been disabled.
hint: 127.0.0.1:${api.port} does not show this timeline to you; try another one, or log in with `msky login --host 127.0.0.1:${api.port}`
```

### Scenario: an error the server writes in another shape is said, not only its status
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky timeline local --json --host ${api.url}
```

#### Then
- exit code is `4`
- stdout at `$.error` equals `notes/local-timeline failed: Internal Server Error 🔥`
- stderr contains `error: notes/local-timeline failed: Internal Server Error 🔥`

### Scenario: a pipe closed early ends the output quietly with exit 0
_skipped on Windows_
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
{ msky timeline local -n 100 --json --host ${api.url}; echo "msky=$?" >&2; } | head -c 1
{ msky timeline local -n 100 --host ${api.url}; echo "msky=$?" >&2; } | head -c 1
```

#### Then
- after `{ msky timeline local -n 100 --json --host ${api.url}; echo "msky=$?" >&2; } | head -c 1`:
  - stderr contains `msky=0`, does not contain `panicked`, `Broken pipe`
- after `{ msky timeline local -n 100 --host ${api.url}; echo "msky=$?" >&2; } | head -c 1`:
  - stderr contains `msky=0`, does not contain `Broken pipe`
