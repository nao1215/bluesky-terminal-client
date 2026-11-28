# atago Behavior Specs
## Summary
1 suite · 11 scenarios
## Contents
- [tui-actions](#tui-actions): 11 scenarios
  - [follow toggles from a post and from its empty profile](#scenario-follow-toggles-from-a-post-and-from-its-empty-profile)
  - [follow refusal restores the state](#scenario-follow-refusal-restores-the-state)
  - [mute toggles from a post and from its empty profile](#scenario-mute-toggles-from-a-post-and-from-its-empty-profile)
  - [mute refusal restores the state](#scenario-mute-refusal-restores-the-state)
  - [block toggles from a post and from its empty profile](#scenario-block-toggles-from-a-post-and-from-its-empty-profile)
  - [block refusal restores the state](#scenario-block-refusal-restores-the-state)
  - [declining mute block delete and report sends no write](#scenario-declining-mute-block-delete-and-report-sends-no-write)
  - [delete removes the post only after success](#scenario-delete-removes-the-post-only-after-success)
  - [a refused delete keeps the post](#scenario-a-refused-delete-keeps-the-post)
  - [report a post with an optional comment](#scenario-report-a-post-with-an-optional-comment)
  - [report the account without a comment](#scenario-report-the-account-without-a-comment)

## tui-actions
Source: `crates/mastodon-terminal-client/e2e/atago/tui-actions.atago.yaml`

Secrets declared: `e2e-stage3-fake`.
### Scenario: follow toggles from a post and from its empty profile
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "7", "username": "me", "token": "e2e-stage3-fake", "scopes": ["read", "write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current":"127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`
- mock `api` received `POST /api/v1/accounts/2/follow` exactly 1 time(s)
- mock `api` received `POST /api/v1/accounts/2/unfollow` exactly 1 time(s)
- mock `api` received `GET /api/v1/accounts/relationships` exactly 1 time(s)

### Scenario: follow refusal restores the state
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "7", "username": "me", "token": "e2e-stage3-fake", "scopes": ["read", "write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current":"127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`
- mock `api` received `POST /api/v1/accounts/2/unfollow` exactly 1 time(s)
- mock `api` received `POST /api/v1/accounts/2/follow` exactly 0 time(s)

### Scenario: mute toggles from a post and from its empty profile
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "7", "username": "me", "token": "e2e-stage3-fake", "scopes": ["read", "write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current":"127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`
- mock `api` received `POST /api/v1/accounts/2/mute` exactly 1 time(s)
- mock `api` received `POST /api/v1/accounts/2/unmute` exactly 1 time(s)
- mock `api` received `GET /api/v1/accounts/relationships` exactly 1 time(s)

### Scenario: mute refusal restores the state
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "7", "username": "me", "token": "e2e-stage3-fake", "scopes": ["read", "write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current":"127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`
- mock `api` received `POST /api/v1/accounts/2/unmute` exactly 1 time(s)
- mock `api` received `POST /api/v1/accounts/2/mute` exactly 0 time(s)

### Scenario: block toggles from a post and from its empty profile
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "7", "username": "me", "token": "e2e-stage3-fake", "scopes": ["read", "write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current":"127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`
- mock `api` received `POST /api/v1/accounts/2/block` exactly 1 time(s)
- mock `api` received `POST /api/v1/accounts/2/unblock` exactly 1 time(s)
- mock `api` received `GET /api/v1/accounts/relationships` exactly 1 time(s)

### Scenario: block refusal restores the state
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "7", "username": "me", "token": "e2e-stage3-fake", "scopes": ["read", "write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current":"127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`
- mock `api` received `POST /api/v1/accounts/2/unblock` exactly 1 time(s)
- mock `api` received `POST /api/v1/accounts/2/block` exactly 0 time(s)

### Scenario: declining mute block delete and report sends no write
#### Given
- Stub HTTP server `api` serves 9 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "7", "username": "me", "token": "e2e-stage3-fake", "scopes": ["read", "write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current":"127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`
- mock `api` received `POST /api/v1/accounts/2/mute` exactly 0 time(s)
- mock `api` received `POST /api/v1/accounts/2/block` exactly 0 time(s)
- mock `api` received `DELETE /api/v1/statuses/s1` exactly 0 time(s)
- mock `api` received `POST /api/v1/reports` exactly 0 time(s)

### Scenario: delete removes the post only after success
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "7", "username": "me", "token": "e2e-stage3-fake", "scopes": ["read", "write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current":"127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`
- mock `api` received `DELETE /api/v1/statuses/s1` exactly 1 time(s)

### Scenario: a refused delete keeps the post
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "7", "username": "me", "token": "e2e-stage3-fake", "scopes": ["read", "write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current":"127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`
- mock `api` received `DELETE /api/v1/statuses/s1` exactly 1 time(s)

### Scenario: report a post with an optional comment
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "7", "username": "me", "token": "e2e-stage3-fake", "scopes": ["read", "write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current":"127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`
- mock `api` received `POST /api/v1/reports` exactly 1 time(s)

### Scenario: report the account without a comment
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "7", "username": "me", "token": "e2e-stage3-fake", "scopes": ["read", "write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current":"127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`
- mock `api` received `POST /api/v1/reports` exactly 1 time(s)
