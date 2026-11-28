# atago Behavior Specs
## Summary
1 suite · 5 scenarios
## Contents
- [tui-attachments](#tui-attachments): 5 scenarios
  - [composer strips picture metadata and uploads its alt text](#scenario-composer-strips-picture-metadata-and-uploads-its-alt-text)
  - [composer uploads a video](#scenario-composer-uploads-a-video)
  - [cancelled attachments are never uploaded](#scenario-cancelled-attachments-are-never-uploaded)
  - [server image size limit stops upload and keeps the draft](#scenario-server-image-size-limit-stops-upload-and-keeps-the-draft)
  - [server alt length limit stops upload and keeps the draft](#scenario-server-alt-length-limit-stops-upload-and-keeps-the-draft)

## tui-attachments
Source: `crates/mastodon-terminal-client/e2e/atago/tui-attachments.atago.yaml`

Secrets declared: `e2e-stage3-fake`.
### Scenario: composer strips picture metadata and uploads its alt text
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Fixture file `media/gps.jpg` is created.
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
- mock `api` received `POST /api/v2/media` exactly 1 time(s)
- mock `api` received `GET /api/v1/media/m1` exactly 1 time(s)
- mock `api` received `POST /api/v1/statuses` exactly 1 time(s)

### Scenario: composer uploads a video
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Fixture file `media/gps.mp4` is created.
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
- mock `api` received `POST /api/v2/media` exactly 1 time(s)
- mock `api` received `GET /api/v1/media/m1` exactly 1 time(s)
- mock `api` received `POST /api/v1/statuses` exactly 1 time(s)

### Scenario: cancelled attachments are never uploaded
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Fixture file `media/photo.png` is created.
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
- mock `api` received `POST /api/v2/media` exactly 0 time(s)
- mock `api` received `POST /api/v1/statuses` exactly 0 time(s)

### Scenario: server image size limit stops upload and keeps the draft
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Fixture file `media/photo.png` is created.
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
- mock `api` received `POST /api/v2/media` exactly 0 time(s)
- mock `api` received `POST /api/v1/statuses` exactly 0 time(s)

### Scenario: server alt length limit stops upload and keeps the draft
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Fixture file `media/photo.png` is created.
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
- mock `api` received `POST /api/v2/media` exactly 0 time(s)
- mock `api` received `POST /api/v1/statuses` exactly 0 time(s)
