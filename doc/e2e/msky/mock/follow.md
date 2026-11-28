# atago Behavior Specs
## Summary
1 suite · 4 scenarios
## Contents
- [msky follow](#msky-follow): 4 scenarios
  - [follow of an account of another server says it is pending until accepted](#scenario-follow-of-an-account-of-another-server-says-it-is-pending-until-accepted)
  - [follow of an account followed already writes nothing](#scenario-follow-of-an-account-followed-already-writes-nothing)
  - [unfollow unfollows, and an account not followed is exit 1](#scenario-unfollow-unfollows-and-an-account-not-followed-is-exit-1)
  - [follow without a login says to log in, and sends nothing](#scenario-follow-without-a-login-says-to-log-in-and-sends-nothing)

## msky follow
Source: `crates/misskey-terminal-client/e2e/atago/follow.atago.yaml`

Secrets declared: `e2e-env-token-0003`.
### Scenario: follow of an account of another server says it is pending until accepted
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky follow @alice@misskey.design --host ${api.url}
```

#### Then
- exit code is `0`
- stdout equals an exact value
- mock `api` received `/api/users/show` exactly 2 time(s)
- mock `api` received `/api/following/create` exactly 1 time(s)

#### Expected output
_expected stdout:_

```text
asked to follow @alice@misskey.design; pending until their server accepts (msky follows --pending lists it)
```

### Scenario: follow of an account followed already writes nothing
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky follow bob --json --host ${api.url}
```

#### Then
- exit code is `0`
- stdout equals an exact value
- mock `api` received `/api/following/create` exactly 0 time(s)

#### Expected output
_expected stdout:_

```text
{"action":"follow","already":true,"id":null,"requested":false,"target":"u-bob","url":null}
```

### Scenario: unfollow unfollows, and an account not followed is exit 1
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Stub HTTP server `other` serves 2 canned route(s) at `${other.url}` and records every request (#24).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky unfollow bob --json --host ${api.url}
msky unfollow carol --host ${other.url}
```

#### Then
- after `msky unfollow bob --json --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/api/following/delete` exactly 1 time(s)
- after `msky unfollow carol --host ${other.url}`:
  - exit code is `1`
  - stderr equals an exact value

#### Expected output
_expected stdout:_

```text
{"action":"unfollow","already":false,"id":null,"requested":false,"target":"u-bob","url":null}
```

_expected stderr:_

```text
error: you do not follow @carol@127.0.0.1:${other.port}
```

### Scenario: follow without a login says to log in, and sends nothing
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky follow bob --host ${api.url}
```

#### Then
- exit code is `1`
- stderr contains `error: not logged in on 127.0.0.1:${api.port}`
- mock `api` received `/api/following/create` exactly 0 time(s)
