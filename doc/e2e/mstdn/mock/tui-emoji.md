# atago Behavior Specs
## Summary
1 suite · 1 scenario
## Contents
- [mstdn inline custom emoji](#mstdn-inline-custom-emoji): 1 scenario
  - [entity emoji pictures are fetched while unknown shortcodes remain text](#scenario-entity-emoji-pictures-are-fetched-while-unknown-shortcodes-remain-text)

## mstdn inline custom emoji
Source: `crates/mastodon-terminal-client/e2e/atago/tui-emoji.atago.yaml`
### Scenario: entity emoji pictures are fetched while unknown shortcodes remain text
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"fake-emoji","scopes":["read"]}
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
- mock `api` received `GET /emoji.png` exactly 1 time(s)
- mock `api` received `POST` exactly 0 time(s)
