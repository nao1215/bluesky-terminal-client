# atago Behavior Specs
## Summary
1 suite · 2 scenarios
## Contents
- [mstdn settings and accounts](#mstdn-settings-and-accounts): 2 scenarios
  - [settings keep theme and language before the locale and preserve unknown keys](#scenario-settings-keep-theme-and-language-before-the-locale-and-preserve-unknown-keys)
  - [A switches saved accounts and later actions use the new server and token](#scenario-a-switches-saved-accounts-and-later-actions-use-the-new-server-and-token)

## mstdn settings and accounts
Source: `crates/mastodon-terminal-client/e2e/atago/tui-settings-accounts.atago.yaml`

Secrets declared: `e2e-fake-stage2-first`, `e2e-fake-stage2-second`.
### Scenario: settings keep theme and language before the locale and preserve unknown keys
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/settings.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/settings.json`:_

```text
{"language":"en","later":{"kept":true}}
```

#### When

```shell
# interactive (pty): mstdn --host ${api.url}
# interactive (pty): mstdn --host ${api.url}
```

#### Then
- after `interactive (pty): mstdn --host ${api.url}`:
  - exit code is `0`
  - file `cfg/settings.json` at `$.language` equals `ja`; at `$.theme` equals `bluesky-dark`; at `$.later.kept` equals `true`
- after `interactive (pty): mstdn --host ${api.url}`:
  - exit code is `0`

### Scenario: A switches saved accounts and later actions use the new server and token
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Stub HTTP server `second` serves 3 canned route(s) at `${second.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts/localhost_${second.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"1","username":"first","token":"e2e-fake-stage2-first","scopes":["read","write"]}
```

_Fixture `cfg/accounts/localhost_${second.port}.json`:_

```text
{"service":"http://localhost:${second.port}","userId":"2","username":"second","token":"e2e-fake-stage2-second","scopes":["read","write"]}
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
- file `cfg/accounts.json` at `$.current` equals `localhost:${second.port}`
- mock `second` received `/api/v1/timelines/home`
- mock `second` received `/api/v1/statuses/new1/favourite` exactly 1 time(s)
- mock `api` received `/api/v1/statuses/new1/favourite` exactly 0 time(s)
