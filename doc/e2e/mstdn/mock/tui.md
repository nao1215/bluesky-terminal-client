# atago Behavior Specs
## Summary
1 suite · 5 scenarios
## Contents
- [mstdn client](#mstdn-client): 5 scenarios
  - [the home timeline draws its posts, and the keys act on them](#scenario-the-home-timeline-draws-its-posts-and-the-keys-act-on-them)
  - [the composer posts a reply with a warning, once, and the theme is kept](#scenario-the-composer-posts-a-reply-with-a-warning-once-and-the-theme-is-kept)
  - [a visitor reads the local timeline and is told how to log in](#scenario-a-visitor-reads-the-local-timeline-and-is-told-how-to-log-in)
  - [the client speaks Japanese in a Japanese locale](#scenario-the-client-speaks-japanese-in-a-japanese-locale)
  - [without a terminal the client says it needs one](#scenario-without-a-terminal-the-client-says-it-needs-one)

## mstdn client
Source: `crates/mastodon-terminal-client/e2e/atago/tui.atago.yaml`

Secrets declared: `e2e-fake-token-0201`, `e2e-fake-token-0202`.
### Scenario: the home timeline draws its posts, and the keys act on them
#### Given
- Stub HTTP server `api` serves 8 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "7", "username": "me", "token": "e2e-fake-token-0201", "scopes": ["read", "write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`
- mock `api` received `GET /api/v1/timelines/home`
- mock `api` received `GET /api/v1/timelines/home` exactly 2 time(s)
- mock `api` received `GET /api/v1/timelines/public` exactly 2 time(s)
- mock `api` received `POST /api/v1/statuses/s1/favourite` exactly 1 time(s)
- mock `api` received `POST /api/v1/statuses/s1/reblog` exactly 1 time(s)
- mock `api` received `GET /api/v1/statuses/o1/context` exactly 1 time(s)

### Scenario: the composer posts a reply with a warning, once, and the theme is kept
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "7", "username": "me", "token": "e2e-fake-token-0201", "scopes": ["read", "write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`
- mock `api` received `POST /api/v1/statuses` exactly 1 time(s)
- file `cfg/settings.json` at `$.theme` equals `monochrome`

### Scenario: a visitor reads the local timeline and is told how to log in
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): mstdn --host ${api.url}
```

#### Then
- exit code is `0`
- mock `api` received `GET /api/v1/timelines/public` exactly 1 time(s)

### Scenario: the client speaks Japanese in a Japanese locale
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with a cleared environment (passing through: PATH, HOME, USERPROFILE, APPDATA, LOCALAPPDATA).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "7", "username": "me", "token": "e2e-fake-token-0201", "scopes": ["read", "write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`

### Scenario: without a terminal the client says it needs one
#### Given
- Environment variables are set: MSTDN_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn
```

#### Then
- exit code is `2`
- stderr equals an exact value

#### Expected output
_expected stderr:_

```text
error: mstdn needs an interactive terminal
hint: run mstdn directly in a terminal, not through a pipe or redirect
```

