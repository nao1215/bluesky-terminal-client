# atago Behavior Specs
## Summary
1 suite · 4 scenarios
## Contents
- [msky accounts and follows](#msky-accounts-and-follows): 4 scenarios
  - [accounts lists the logins as bsky's does, and --use changes the one in use](#scenario-accounts-lists-the-logins-as-bskys-does-and---use-changes-the-one-in-use)
  - [servers lists Misskey Hub's servers that run Misskey, the most accounts first](#scenario-servers-lists-misskey-hubs-servers-that-run-misskey-the-most-accounts-first)
  - [servers falls back to the ones msky knows when the list cannot be read](#scenario-servers-falls-back-to-the-ones-msky-knows-when-the-list-cannot-be-read)
  - [follows and followers list the accounts page by page, and --pending the requests](#scenario-follows-and-followers-list-the-accounts-page-by-page-and---pending-the-requests)

## msky accounts and follows
Source: `crates/misskey-terminal-client/e2e/atago/accounts.atago.yaml`

Secrets declared: `e2e-fake-token-0201`, `e2e-fake-token-0202`.
### Scenario: accounts lists the logins as bsky's does, and --use changes the one in use
#### Given
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Fixture file `cfg/accounts/a.test.json` is created.
- Fixture file `cfg/accounts/b.test.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/a.test.json`:_

```text
{"service": "https://a.test", "userId": "u-a", "username": "ann", "token": "e2e-fake-token-0201"}
```

_Fixture `cfg/accounts/b.test.json`:_

```text
{"service": "https://b.test", "userId": "u-b", "username": "ben", "token": "e2e-fake-token-0202"}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "a.test"}
```

#### When

```shell
msky accounts
msky accounts
msky accounts --use b.test --json
msky accounts --use c.test
```

#### Then
- after `msky accounts`:
  - exit code is `0`
  - stdout equals an exact value
- after `msky accounts`:
  - exit code is `0`
  - stdout equals an exact value
- after `msky accounts --use b.test --json`:
  - exit code is `0`
  - stdout equals an exact value
  - file `cfg/accounts.json` contains `"current": "b.test"`
- after `msky accounts --use c.test`:
  - exit code is `1`
  - stderr equals an exact value

#### Expected output
_expected stdout:_

```text
not logged in
```

_expected stdout:_

```text
* @ann@a.test  u-a
  @ben@b.test  u-b
```

_expected stdout:_

```text
{"current":false,"host":"a.test","id":"u-a","service":"https://a.test","username":"ann"}
{"current":true,"host":"b.test","id":"u-b","service":"https://b.test","username":"ben"}
```

_expected stderr:_

```text
error: no account is logged in on c.test
hint: logged in on: a.test, b.test
```

### Scenario: servers lists Misskey Hub's servers that run Misskey, the most accounts first
#### Given
- Stub HTTP server `hub` serves 1 canned route(s) at `${hub.url}` and records every request (#24).
- Environment variables are set: MSKY_SERVERS_URL.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_SERVERS_URL.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky servers
msky servers -n 1 --json
```

#### Then
- after `msky servers`:
  - exit code is `0`
  - stdout equals an exact value
- after `msky servers -n 1 --json`:
  - exit code is `0`
  - stdout equals an exact value

#### Expected output
_expected stdout:_

```text
big.test  大きい 🐈  9000 accounts  (closed to new accounts)
small.test  小さい  10 accounts
```

_expected stdout:_

```text
{"host":"big.test","langs":["ja","en"],"name":"大きい 🐈","openRegistrations":false,"users":9000}
```

### Scenario: servers falls back to the ones msky knows when the list cannot be read
#### Given
- Stub HTTP server `hub` serves 1 canned route(s) at `${hub.url}` and records every request (#24).
- Environment variables are set: MSKY_SERVERS_URL.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky servers -n 2
```

#### Then
- exit code is `0`
- stdout equals an exact value
- stderr contains `HTTP 503; showing the servers msky knows`

#### Expected output
_expected stdout:_

```text
misskey.io  Misskey.io
misskey-square.net  みすきーすくえあ！
```

### Scenario: follows and followers list the accounts page by page, and --pending the requests
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0201"}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "127.0.0.1:${api.port}"}
```

#### When

```shell
msky follows @alice
msky followers --json -n 1
msky follows --pending
msky follows bob --pending
```

#### Then
- after `msky follows @alice`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/api/users/following` exactly 2 time(s)
- after `msky followers --json -n 1`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/api/users/followers`
- after `msky follows --pending`:
  - exit code is `0`
  - stdout equals an exact value
- after `msky follows bob --pending`:
  - exit code is `1`
  - stderr equals an exact value

#### Expected output
_expected stdout:_

```text
ボブ 👨‍👩‍👧 @bob@mstdn.test  (following)  ${api.url}/@bob@mstdn.test
```

_expected stdout:_

```text
{"description":null,"followersCount":null,"following":false,"followingCount":null,"host":"127.0.0.1:${api.port}","id":"u-cat","locked":false,"name":null,"postsCount":null,"url":"${api.url}/@cat","username":"cat"}
```

_expected stdout:_

```text
@dan@remote.test  ${api.url}/@dan@remote.test
```

_expected stderr:_

```text
error: --pending lists your own requests, of the account logged in
hint: leave ACTOR out
```

