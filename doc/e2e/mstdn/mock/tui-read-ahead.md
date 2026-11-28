# atago Behavior Specs
## Summary
1 suite · 1 scenario
## Contents
- [mstdn speculative threads and unread notifications](#mstdn-speculative-threads-and-unread-notifications): 1 scenario
  - [resting reads once and v adopts the thread while 4 clears unread](#scenario-resting-reads-once-and-v-adopts-the-thread-while-4-clears-unread)

## mstdn speculative threads and unread notifications
Source: `crates/mastodon-terminal-client/e2e/atago/tui-read-ahead.atago.yaml`
### Scenario: resting reads once and v adopts the thread while 4 clears unread
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"fake-read-ahead","scopes":["read","write"]}
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
- mock `api` received `GET /api/v1/statuses/10/context` exactly 1 time(s)
- mock `api` received `GET /api/v1/statuses/10` exactly 1 time(s)
- mock `api` received `POST` exactly 0 time(s)
