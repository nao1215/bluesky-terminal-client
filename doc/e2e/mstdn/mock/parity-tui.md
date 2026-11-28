# atago Behavior Specs
## Summary
1 suite · 5 scenarios
## Contents
- [mstdn TUI parity](#mstdn-tui-parity): 5 scenarios
  - [add an account with browser OAuth then decline and confirm logout](#scenario-add-an-account-with-browser-oauth-then-decline-and-confirm-logout)
  - [cancelling browser login saves no account and does not exchange a token](#scenario-cancelling-browser-login-saves-no-account-and-does-not-exchange-a-token)
  - [notification marker syncs the viewed notification](#scenario-notification-marker-syncs-the-viewed-notification)
  - [notification marker refusal restores server unread count](#scenario-notification-marker-refusal-restores-server-unread-count)
  - [download and browser settings can be saved cancelled and reset](#scenario-download-and-browser-settings-can-be-saved-cancelled-and-reset)

## mstdn TUI parity
Source: `crates/mastodon-terminal-client/e2e/atago/parity-tui.atago.yaml`
### Scenario: add an account with browser OAuth then decline and confirm logout
_skipped on Windows_
#### Given
- Stub HTTP server `api` serves 9 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `browser.sh` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): mstdn --host ${api.url}
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `POST /api/v1/apps` exactly 1 time(s)
- mock `api` received `POST /oauth/token` exactly 1 time(s)
- mock `api` received `/oauth/token`
- mock `api` received `POST /api/v1/apps`
- file `cfg/accounts/127.0.0.1_${api.port}.json` does not exist

### Scenario: cancelling browser login saves no account and does not exchange a token
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): mstdn --host ${api.url}
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `POST /oauth/token` exactly 0 time(s)

### Scenario: notification marker syncs the viewed notification
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"alice","token":"parity-fake-token","scopes":["read","write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current":"127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn --host ${api.url}
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `POST /api/v1/markers` exactly 1 time(s)

### Scenario: notification marker refusal restores server unread count
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"alice","token":"parity-fake-token","scopes":["read","write"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current":"127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): mstdn --host ${api.url}
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `POST /api/v1/markers` exactly 1 time(s)

### Scenario: download and browser settings can be saved cancelled and reset
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): mstdn --host ${api.url}
```

#### Then
- exit code is `0`
- stderr is empty
- file `cfg/settings.json` at `$.download_dir` equals `downloads-parity`; at `$.browser` equals `browser-parity`
