# atago Behavior Specs
## Summary
1 suite · 3 scenarios
## Contents
- [mstdn quotes](#mstdn-quotes): 3 scenarios
  - [quote with followers approval](#scenario-quote-with-followers-approval)
  - [cancel quote](#scenario-cancel-quote)
  - [a readable status without quote capability refuses quotes](#scenario-a-readable-status-without-quote-capability-refuses-quotes)

## mstdn quotes
Source: `crates/mastodon-terminal-client/e2e/atago/tui-quotes.atago.yaml`
### Scenario: quote with followers approval
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"fake-quote","scopes":["read","write"]}
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
- mock `api` received `POST /api/v1/statuses` exactly 1 time(s)

### Scenario: cancel quote
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"fake-quote","scopes":["read","write"]}
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
- mock `api` received `POST /api/v1/statuses` exactly 0 time(s)

### Scenario: a readable status without quote capability refuses quotes
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"fake-quote","scopes":["read","write"]}
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
- mock `api` received `POST /api/v1/statuses` exactly 0 time(s)
