# atago Behavior Specs
## Summary
1 suite · 5 scenarios
## Contents
- [mstdn profile editing](#mstdn-profile-editing): 5 scenarios
  - [only changed display name is patched after ctrl+s](#scenario-only-changed-display-name-is-patched-after-ctrls)
  - [cancelling an edited profile sends no PATCH](#scenario-cancelling-an-edited-profile-sends-no-patch)
  - [bio flags and profile fields are a single exact PATCH](#scenario-bio-flags-and-profile-fields-are-a-single-exact-patch)
  - [avatar and header strip metadata in one PATCH](#scenario-avatar-and-header-strip-metadata-in-one-patch)
  - [cancelling chosen profile pictures sends no PATCH](#scenario-cancelling-chosen-profile-pictures-sends-no-patch)

## mstdn profile editing
Source: `crates/mastodon-terminal-client/e2e/atago/tui-edit-profile.atago.yaml`
### Scenario: only changed display name is patched after ctrl+s
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"fake-profile","scopes":["read","write"]}
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
- mock `api` received `PATCH /api/v1/accounts/update_credentials` exactly 1 time(s)

### Scenario: cancelling an edited profile sends no PATCH
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"fake-profile","scopes":["read","write"]}
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
- mock `api` received `PATCH` exactly 0 time(s)

### Scenario: bio flags and profile fields are a single exact PATCH
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"fake-profile","scopes":["read","write"]}
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
- mock `api` received `PATCH /api/v1/accounts/update_credentials` exactly 1 time(s)

### Scenario: avatar and header strip metadata in one PATCH
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Fixture file `pictures/gps.jpg` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"fake-profile","scopes":["read","write"]}
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
- mock `api` received `PATCH /api/v1/accounts/update_credentials` exactly 1 time(s)

### Scenario: cancelling chosen profile pictures sends no PATCH
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Fixture file `pictures/gps.jpg` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service":"${api.url}","userId":"7","username":"me","token":"fake-profile","scopes":["read","write"]}
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
- mock `api` received `PATCH /api/v1/accounts/update_credentials` exactly 0 time(s)
