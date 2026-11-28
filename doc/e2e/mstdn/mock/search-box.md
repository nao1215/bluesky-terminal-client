# atago Behavior Specs
## Summary
1 suite · 1 scenario
## Contents
- [mstdn search box](#mstdn-search-box): 1 scenario
  - [slash opens a box and cancel and result escape preserve the selected timeline post](#scenario-slash-opens-a-box-and-cancel-and-result-escape-preserve-the-selected-timeline-post)

## mstdn search box
Source: `crates/mastodon-terminal-client/e2e/atago/search-box.atago.yaml`

Secrets declared: `fake-search-box`.
### Scenario: slash opens a box and cancel and result escape preserve the selected timeline post
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"fake-search-box","scopes":["read","write"]}
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
- mock `api` received `/api/v1/timelines/home` exactly 1 time(s)
- mock `api` received `/api/v2/search` exactly 2 time(s)
- mock `api` received `/api/v1/accounts/2` exactly 4 time(s)
- mock `api` received `/api/v1/statuses/result/favourite` exactly 1 time(s)
