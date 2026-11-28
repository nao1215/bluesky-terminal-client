# atago Behavior Specs
## Summary
1 suite · 3 scenarios
## Contents
- [mstdn direct conversations](#mstdn-direct-conversations): 3 scenarios
  - [enter opens and marks read then i and enter reply directly](#scenario-enter-opens-and-marks-read-then-i-and-enter-reply-directly)
  - [viewing the list and cancelling a new profile message send no writes](#scenario-viewing-the-list-and-cancelling-a-new-profile-message-send-no-writes)
  - [m on profile creates a direct post only on enter](#scenario-m-on-profile-creates-a-direct-post-only-on-enter)

## mstdn direct conversations
Source: `crates/mastodon-terminal-client/e2e/atago/tui-chat.atago.yaml`
### Scenario: enter opens and marks read then i and enter reply directly
#### Given
- Stub HTTP server `api` serves 10 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"fake-chat","scopes":["read","write"]}
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
- mock `api` received `POST /api/v1/conversations/c1/read` exactly 1 time(s)
- mock `api` received `POST /api/v1/statuses` exactly 1 time(s)

### Scenario: viewing the list and cancelling a new profile message send no writes
#### Given
- Stub HTTP server `api` serves 10 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"fake-chat","scopes":["read","write"]}
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
- mock `api` received `POST` exactly 0 time(s)

### Scenario: m on profile creates a direct post only on enter
#### Given
- Stub HTTP server `api` serves 10 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"fake-chat","scopes":["read","write"]}
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
- mock `api` received `POST /api/v1/conversations/c1/read` exactly 0 time(s)
- mock `api` received `POST /api/v1/statuses` exactly 1 time(s)
