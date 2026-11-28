# atago Behavior Specs
## Summary
1 suite · 6 scenarios
## Contents
- [mstdn login](#mstdn-login): 6 scenarios
  - [login --oob registers, takes the pasted code, and whoami and logout use the token](#scenario-login---oob-registers-takes-the-pasted-code-and-whoami-and-logout-use-the-token)
  - [login takes the code the browser brings back to mstdn](#scenario-login-takes-the-code-the-browser-brings-back-to-mstdn)
  - [a code the server does not take saves nothing, and a denial in the browser says so](#scenario-a-code-the-server-does-not-take-saves-nothing-and-a-denial-in-the-browser-says-so)
  - [MSTDN_TOKEN is sent to its server only, and a token turned down says so](#scenario-mstdn_token-is-sent-to-its-server-only-and-a-token-turned-down-says-so)
  - [a broken login file says how to get out of it, and logout --all takes it away](#scenario-a-broken-login-file-says-how-to-get-out-of-it-and-logout---all-takes-it-away)
  - [the login speaks Japanese where the locale is Japanese](#scenario-the-login-speaks-japanese-where-the-locale-is-japanese)

## mstdn login
Source: `crates/mastodon-terminal-client/e2e/atago/login.atago.yaml`

Secrets declared: `e2e-fake-token-0001`, `e2e-client-secret-0001`.
### Scenario: login --oob registers, takes the pasted code, and whoami and logout use the token
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_stdin for `mstdn`:_

```text
e2e-code-0001
```

_stdin for `mstdn`:_

```text
e2e-code-0002
```

#### When

```shell
mstdn login --oob --host ${api.url}
mstdn whoami
mstdn accounts --json
mstdn login --oob --json
mstdn logout
mstdn whoami --host ${api.url}
```

#### Then
- after `mstdn login --oob --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - stderr contains `"To log in on 127.0.0.1:${api.port}, open this address, signed in there, and allow mstdn to read and write:\n${api.url}/oauth/authorize?response_type=code&client_id=e2e-client-id&redirect_uri=urn%3Aietf%3Awg%3Aoauth%3A2.0%3Aoob&scope=read%20write&state="`, `"&code_challenge_method=S256\n"`, `Paste the code the page shows, then press Enter: `, does not contain `e2e-fake-token-0001`, `e2e-client-secret-0001`
  - mock `api` received `/api/v1/apps` exactly 1 time(s)
  - mock `api` received `/oauth/token` exactly 1 time(s)
  - mock `api` received `/api/v1/accounts/verify_credentials`
  - file `cfg/accounts/127.0.0.1_${api.port}.json` at `$.service` equals `${api.url}`; at `$.userId` equals `7`; at `$.username` equals `alice`; at `$.token` equals `e2e-fake-token-0001`; at `$.scopes[1]` equals `write`
  - file `cfg/accounts.json` at `$.current` equals `127.0.0.1:${api.port}`
  - file `cfg/apps/127.0.0.1_${api.port}.json` at `$.clientId` equals `e2e-client-id`
- after `mstdn whoami`:
  - exit code is `0`
  - stdout equals an exact value
  - stderr is empty
- after `mstdn accounts --json`:
  - exit code is `0`
  - stdout at `$.username` equals `alice`; at `$.current` equals `true`
- after `mstdn login --oob --json`:
  - exit code is `0`
  - stdout at `$.username` equals `alice`
  - mock `api` received `/api/v1/apps` exactly 1 time(s)
- after `mstdn logout`:
  - exit code is `0`
  - stdout equals an exact value
  - file `cfg/accounts/127.0.0.1_${api.port}.json` does not exist
- after `mstdn whoami --host ${api.url}`:
  - exit code is `1`
  - stderr equals an exact value

#### Expected output
_expected stdout:_

```text
logged in as @alice@127.0.0.1:${api.port}
```

_expected stdout:_

```text
アリス 🐈‍⬛ @alice@127.0.0.1:${api.port}
${api.url}/@alice
12 followers  34 following  56 posts

家族👨‍👩‍👧 と 🇯🇵 に住む
```

_expected stdout:_

```text
logged out @alice@127.0.0.1:${api.port}
```

_expected stderr:_

```text
error: not logged in on 127.0.0.1:${api.port}
hint: run `mstdn login --host 127.0.0.1:${api.port}`
```

### Scenario: login takes the code the browser brings back to mstdn
_skipped on Windows_
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `browser.sh` is created.
- Environment variables are set: MSTDN_BROWSER, MSTDN_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn login --timeout 20 --host ${api.url}
```

#### Then
- exit code is `0`
- stdout equals an exact value
- stderr contains `redirect_uri=http%3A%2F%2F127.0.0.1%3A`, `"(opened in the web browser)\n"`, `"Waiting for the browser to come back (up to 20 s; Ctrl-C to give up)...\n"`
- mock `api` received `/oauth/token`
- mock `api` received `/api/v1/apps`

#### Expected output
_expected stdout:_

```text
logged in as @alice@127.0.0.1:${api.port}
```

### Scenario: a code the server does not take saves nothing, and a denial in the browser says so
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_stdin for `mstdn`:_

```text
stale
```

#### When

```shell
mstdn login --oob --json --host ${api.url}
mstdn login --oob --host ${api.url}
mstdn login --timeout 1 --host ${api.url}
```

#### Then
- after `mstdn login --oob --json --host ${api.url}`:
  - exit code is `1`
  - stdout at `$.error` matches `/^127\.0\.0\.1:[0-9]+ did not take the code: invalid_grant: The provided authorization grant is invalid/`; at `$.hint` equals `` nothing was saved; run `mstdn login` to ask again ``
  - file `cfg/accounts` does not exist
- after `mstdn login --oob --host ${api.url}`:
  - exit code is `1`
  - stderr contains `error: no code was given`
- after `mstdn login --timeout 1 --host ${api.url}`:
  - exit code is `1`
  - stderr contains `error: the access was not given on 127.0.0.1:${api.port} within 1 s`, `hint: nothing was saved`

### Scenario: MSTDN_TOKEN is sent to its server only, and a token turned down says so
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Stub HTTP server `other` serves 1 canned route(s) at `${other.url}` and records every request (#24).
- Environment variables are set: MSTDN_HOST, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_HOST, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn whoami
mstdn status ${other.url}/@b/2
```

#### Then
- after `mstdn whoami`:
  - exit code is `4`
  - stderr equals an exact value
- after `mstdn status ${other.url}/@b/2`:
  - exit code is `0`
  - mock `other` received `/api/v1/statuses/2` exactly 1 time(s)

#### Expected output
_expected stderr:_

```text
error: v1/accounts/verify_credentials failed: The access token is invalid
hint: 127.0.0.1:${api.port} does not take the token in MSTDN_TOKEN; check it, or make a new one in Preferences > Development
```

### Scenario: a broken login file says how to get out of it, and logout --all takes it away
#### Given
- Fixture file `cfg/accounts/mastodon.social.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Environment variables are set: MSTDN_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/mastodon.social.json`:_

```text
{"token": "e2e-fake-token-0001" oops
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "mastodon.social"}
```

#### When

```shell
mstdn whoami
mstdn logout --all
```

#### Then
- after `mstdn whoami`:
  - exit code is `3`
  - stderr contains `is not a valid login file (line 1, column`, `` hint: run `mstdn logout --all` to discard it and log in again ``, does not contain `e2e-fake-token-0001`
- after `mstdn logout --all`:
  - exit code is `0`
  - stdout equals an exact value

#### Expected output
_expected stdout:_

```text
logged out of mastodon.social
```

### Scenario: the login speaks Japanese where the locale is Japanese
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: LC_ALL, MSTDN_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn login --oob --host ${api.url}
```

#### Then
- exit code is `1`
- stderr contains `"127.0.0.1:${api.port} にログインするには、そのサーバーにサインインした状態でこのアドレスを開き、mstdn に読み書きを許可してください:\n"`, `ページに表示されたコードを貼り付けて Enter を押してください: `, `error: no code was given`
