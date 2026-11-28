# atago Behavior Specs
## Summary
1 suite · 6 scenarios
## Contents
- [msky login](#msky-login): 6 scenarios
  - [login shows the page to open, takes the token, and whoami and logout use it](#scenario-login-shows-the-page-to-open-takes-the-token-and-whoami-and-logout-use-it)
  - [a login not allowed in time says so and saves nothing](#scenario-a-login-not-allowed-in-time-says-so-and-saves-nothing)
  - [a server that turns the check down has denied the access, and nothing is saved](#scenario-a-server-that-turns-the-check-down-has-denied-the-access-and-nothing-is-saved)
  - [search, author and note send the saved token to its own server only](#scenario-search-author-and-note-send-the-saved-token-to-its-own-server-only)
  - [a token the server no longer takes is an API error that says to log in again](#scenario-a-token-the-server-no-longer-takes-is-an-api-error-that-says-to-log-in-again)
  - [a broken login file says how to get out of it, and logout --all takes it away](#scenario-a-broken-login-file-says-how-to-get-out-of-it-and-logout---all-takes-it-away)

## msky login
Source: `crates/misskey-terminal-client/e2e/atago/login.atago.yaml`

Secrets declared: `e2e-fake-token-0001`.
### Scenario: login shows the page to open, takes the token, and whoami and logout use it
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_CONFIG_DIR, MSKY_MIAUTH_SESSION, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR, MSKY_MIAUTH_SESSION, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR, MSKY_MIAUTH_SESSION, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR, MSKY_MIAUTH_SESSION, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR, MSKY_MIAUTH_SESSION, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR, MSKY_MIAUTH_SESSION, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky login --host ${api.url}
msky whoami
msky whoami --json
msky logout
msky whoami --host ${api.url}
msky logout --json
```

#### Then
- after `msky login --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - stderr contains `To log in on 127.0.0.1:${api.port}, open this address`, `"${api.url}/miauth/e2e-session-0001?name=msky&permission=read:account,read:notifications,read:following,read:reactions,write:notes,write:drive,write:following,write:reactions,write:notifications,read:mutes,write:mutes,read:blocks,write:blocks,read:chat,write:chat,write:account,write:report-abuse\n"`, `up to 300 s`, does not contain `e2e-fake-token-0001`
  - mock `api` received `/api/miauth/e2e-session-0001/check` exactly 1 time(s)
  - file `cfg/accounts/127.0.0.1_${api.port}.json` at `$.service` equals `${api.url}`; at `$.userId` equals `u-alice`; at `$.username` equals `alice`; at `$.token` equals `e2e-fake-token-0001`
  - file `cfg/accounts.json` at `$.current` equals `127.0.0.1:${api.port}`
- after `msky whoami`:
  - exit code is `0`
  - stdout equals an exact value
  - stderr is empty
  - mock `api` received `/api/i` exactly 1 time(s)
- after `msky whoami --json`:
  - exit code is `0`
  - stdout at `$.id` equals `u-alice`; at `$.username` equals `alice`; at `$.host` equals `127.0.0.1:${api.port}`; at `$.name` equals `アリス 🐈‍⬛`; at `$.url` equals `${api.url}/@alice`; at `$.followersCount` equals `12`; at `$.followingCount` equals `34`; at `$.postsCount` equals `56`
- after `msky logout`:
  - exit code is `0`
  - stdout equals an exact value
  - file `cfg/accounts/127.0.0.1_${api.port}.json` does not exist
- after `msky whoami --host ${api.url}`:
  - exit code is `1`
  - stderr equals an exact value
  - mock `api` received `/api/i` exactly 2 time(s)
- after `msky logout --json`:
  - exit code is `0`
  - stdout equals an exact value

#### Expected output
_expected stdout:_

```text
logged in as @alice@127.0.0.1:${api.port}
```

_expected stdout:_

```text
アリス 🐈‍⬛ @alice@127.0.0.1:${api.port}
${api.url}/@alice
12 followers  34 following  56 notes

家族👨‍👩‍👧 と 🇯🇵 に住む
```

_expected stdout:_

```text
logged out @alice@127.0.0.1:${api.port}
```

_expected stderr:_

```text
error: not logged in on 127.0.0.1:${api.port}
hint: run `msky login --host 127.0.0.1:${api.port}`
```

_expected stdout:_

```text
{"loggedOut":[]}
```

### Scenario: a login not allowed in time says so and saves nothing
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_CONFIG_DIR, MSKY_MIAUTH_SESSION.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky login --host ${api.url} --timeout 1
```

#### Then
- exit code is `1`
- stdout is empty
- stderr contains `"error: the access was not given on 127.0.0.1:${api.port} within 1 s\n"`, `hint: nothing was saved`
- mock `api` received `/api/miauth/e2e-session-0002/check` exactly 2 time(s)
- file `cfg/accounts` does not exist

### Scenario: a server that turns the check down has denied the access, and nothing is saved
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_CONFIG_DIR, MSKY_MIAUTH_SESSION.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR, MSKY_MIAUTH_SESSION.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky login --host ${api.url} --json
msky login --host ${api.url}
```

#### Then
- after `msky login --host ${api.url} --json`:
  - exit code is `1`
  - stdout at `$.error` equals `127.0.0.1:${api.port} did not give msky the access: ACCESS_DENIED: The user denied it.`; at `$.status` equals `1`
  - file `cfg/accounts` does not exist
- after `msky login --host ${api.url}`:
  - exit code is `1`
  - stderr contains `MSKY_MIAUTH_SESSION is not a session ID`
  - mock `api` received `/api/miauth/e2e-session-0003/check` exactly 1 time(s)

### Scenario: search, author and note send the saved token to its own server only
#### Given
- Stub HTTP server `home` serves 4 canned route(s) at `${home.url}` and records every request (#24).
- Stub HTTP server `other` serves 1 canned route(s) at `${other.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${home.port}.json` is created.
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${home.port}.json`:_

```text
{"service": "${home.url}", "userId": "u-alice", "username": "alice", "token": "e2e-fake-token-0001"}
```

#### When

```shell
msky search 猫 -n 1 --host ${home.url}
msky author bob -n 1 --host ${home.url}
msky note ${home.url}/notes/n1
msky note ${other.url}/notes/n2 --host ${home.url}
```

#### Then
- after `msky search 猫 -n 1 --host ${home.url}`:
  - exit code is `0`
  - mock `home` received `/api/notes/search`
- after `msky author bob -n 1 --host ${home.url}`:
  - exit code is `0`
  - mock `home` received `/api/users/notes`
- after `msky note ${home.url}/notes/n1`:
  - exit code is `0`
  - stdout contains `見える 👀`
  - mock `home` received `/api/notes/show`
- after `msky note ${other.url}/notes/n2 --host ${home.url}`:
  - exit code is `0`
  - mock `other` received `/api/notes/show`

### Scenario: a token the server no longer takes is an API error that says to log in again
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-alice", "username": "alice", "token": "e2e-fake-token-0001"}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "127.0.0.1:${api.port}"}
```

#### When

```shell
msky whoami
```

#### Then
- exit code is `4`
- stderr equals an exact value

#### Expected output
_expected stderr:_

```text
error: i failed: AUTHENTICATION_FAILED: Authentication failed. Please ensure your token is correct.
hint: 127.0.0.1:${api.port} no longer takes the token msky saved; run `msky login --host 127.0.0.1:${api.port}` again
```

### Scenario: a broken login file says how to get out of it, and logout --all takes it away
#### Given
- Fixture file `cfg/accounts/misskey.io.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/misskey.io.json`:_

```text
{"token": "e2e-fake-token-0001" oops
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "misskey.io"}
```

#### When

```shell
msky whoami
msky logout --all
```

#### Then
- after `msky whoami`:
  - exit code is `3`
  - stderr contains `is not a valid login file (line 1, column`, `` hint: run `msky logout --all` to discard it and log in again ``, does not contain `e2e-fake-token-0001`
- after `msky logout --all`:
  - exit code is `0`
  - stdout equals an exact value
  - file `cfg/accounts/misskey.io.json` does not exist

#### Expected output
_expected stdout:_

```text
logged out of misskey.io
```

