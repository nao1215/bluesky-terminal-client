# atago Behavior Specs
## Summary
1 suite · 2 scenarios
## Contents
- [mstdn search and profiles](#mstdn-search-and-profiles): 2 scenarios
  - [search posts and accounts and open profiles with enter and 5](#scenario-search-posts-and-accounts-and-open-profiles-with-enter-and-5)
  - [visitors search hashtags and refuse account and full text searches](#scenario-visitors-search-hashtags-and-refuse-account-and-full-text-searches)

## mstdn search and profiles
Source: `crates/mastodon-terminal-client/e2e/atago/tui-browse.atago.yaml`

Secrets declared: `e2e-fake-stage2-browse`.
### Scenario: search posts and accounts and open profiles with enter and 5
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"e2e-fake-stage2-browse","scopes":["read","write"]}
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
- mock `api` received `/api/v2/search` exactly 2 time(s)
- mock `api` received `/api/v1/accounts/verify_credentials` exactly 1 time(s)
- mock `api` received `/api/v1/accounts/2/statuses` exactly 4 time(s)

### Scenario: visitors search hashtags and refuse account and full text searches
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): mstdn --host ${api.url}
```

#### Then
- exit code is `0`
- mock `api` received `/api/v2/search` exactly 0 time(s)
- mock `api` received `/api/v1/timelines/tag/rust` exactly 1 time(s)
