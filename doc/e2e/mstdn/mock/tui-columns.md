# atago Behavior Specs
## Summary
1 suite · 1 scenario
## Contents
- [mstdn columns](#mstdn-columns): 1 scenario
  - [list and followed tag sources beside Home and confirmed column close](#scenario-list-and-followed-tag-sources-beside-home-and-confirmed-column-close)

## mstdn columns
Source: `crates/mastodon-terminal-client/e2e/atago/tui-columns.atago.yaml`
### Scenario: list and followed tag sources beside Home and confirmed column close
#### Given
- Stub HTTP server `api` serves 8 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"fake-columns","scopes":["read"]}
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
- mock `api` received `GET /api/v1/timelines/list/42` exactly 1 time(s)
- mock `api` received `GET /api/v1/timelines/tag/rust` exactly 1 time(s)
- mock `api` received `GET /api/v2/search` exactly 1 time(s)
- mock `api` received `POST` exactly 0 time(s)
