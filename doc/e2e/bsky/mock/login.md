# atago Behavior Specs
## Summary
1 suite · 9 scenarios
## Contents
- [bsky login](#bsky-login): 9 scenarios
  - [logging in saves the session and loads the timeline with its token](#scenario-logging-in-saves-the-session-and-loads-the-timeline-with-its-token)
  - [a saved session skips the login screen on the next start](#scenario-a-saved-session-skips-the-login-screen-on-the-next-start)
  - [rejected credentials keep the form open and save nothing](#scenario-rejected-credentials-keep-the-form-open-and-save-nothing)
  - [an empty field is caught before anything is sent](#scenario-an-empty-field-is-caught-before-anything-is-sent)
  - [an unreachable server is reported on the form](#scenario-an-unreachable-server-is-reported-on-the-form)
  - [a second account is logged in from the account list, and A switches between them](#scenario-a-second-account-is-logged-in-from-the-account-list-and-a-switches-between-them)
  - [-a starts on the account it names without changing the one in use, and an unknown one is a usage error](#scenario--a-starts-on-the-account-it-names-without-changing-the-one-in-use-and-an-unknown-one-is-a-usage-error)
  - [logout logs one account out and keeps the other, and --all logs every one out](#scenario-logout-logs-one-account-out-and-keeps-the-other-and---all-logs-every-one-out)
  - [a write waiting behind a slow one is sent as the account it was pressed on, after a switch](#scenario-a-write-waiting-behind-a-slow-one-is-sent-as-the-account-it-was-pressed-on-after-a-switch)

## bsky login
Source: `crates/bluesky-terminal-client/e2e/atago/login.atago.yaml`

Secrets declared: `app-pass-1234`.
### Scenario: logging in saves the session and loads the timeline with its token
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).

#### When

```shell
# interactive (pty): bsky --service ${api.url}
```

#### Then
- exit code is `0`
- mock `api` received `POST /xrpc/com.atproto.server.createSession` exactly 1 time(s)
- mock `api` received `GET /xrpc/app.bsky.feed.getTimeline` exactly 1 time(s)
- file `cfg/accounts/did_plc_me.json` at `$.service` equals `${api.url}`; at `$.did` equals `did:plc:me`; at `$.handle` equals `me.test`; at `$.accessJwt` equals `acc-1`; at `$.refreshJwt` equals `ref-1`
- file `cfg/accounts/did_plc_me.json` does not contain `app-pass-1234`

### Scenario: a saved session skips the login screen on the next start
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc-9", "refreshJwt": "ref-9"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `api` received `/xrpc/app.bsky.feed.getTimeline` exactly 1 time(s)

### Scenario: rejected credentials keep the form open and save nothing
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).

#### When

```shell
# interactive (pty): bsky --service ${api.url}
```

#### Then
- exit code is `0`
- mock `api` received `/xrpc/com.atproto.server.createSession` exactly 2 time(s)
- mock `api` received `/xrpc/app.bsky.feed.getTimeline` exactly 0 time(s)
- file `cfg/accounts/did_plc_me.json` does not exist

### Scenario: an empty field is caught before anything is sent
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).

#### When

```shell
# interactive (pty): bsky --service ${api.url}
```

#### Then
- exit code is `0`
- mock `api` received a request exactly 0 time(s)

### Scenario: an unreachable server is reported on the form
#### When

```shell
# interactive (pty): bsky --service http://127.0.0.1:9
```

#### Then
- exit code is `0`
- the step changed exactly created nothing, modified nothing, deleted nothing

### Scenario: a second account is logged in from the account list, and A switches between them
#### Given
- Stub HTTP server `home` serves 1 canned route(s) at `${home.url}` and records every request (#24).
- Stub HTTP server `work` serves 2 canned route(s) at `${work.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${home.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `home` received `/xrpc/app.bsky.feed.getTimeline`
- mock `work` received `/xrpc/app.bsky.feed.getTimeline` exactly 1 time(s)
- mock `home` received `/xrpc/app.bsky.feed.getTimeline` exactly 2 time(s)
- file `cfg/accounts/did_plc_me.json` at `$.accessJwt` equals `acc`
- file `cfg/accounts/did_plc_work.json` at `$.accessJwt` equals `acc-work`; at `$.service` equals `${work.url}`
- file `cfg/accounts.json` at `$.current` equals `did:plc:me`

### Scenario: -a starts on the account it names without changing the one in use, and an unknown one is a usage error
#### Given
- Stub HTTP server `work` serves 1 canned route(s) at `${work.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/accounts/did_plc_work.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Environment variables are set: BSKY_CACHE_DIR, BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "http://127.0.0.1:9", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/accounts/did_plc_work.json`:_

```text
{"service": "${work.url}", "did": "did:plc:work", "handle": "work.example", "accessJwt": "acc-work", "refreshJwt": "ref-work"}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "did:plc:me"}
```

#### When

```shell
# interactive (pty): bsky -a @Work.Example
bsky -a nobody.test
```

#### Then
- after `interactive (pty): bsky -a @Work.Example`:
  - exit code is `0`
  - the step changed exactly modified nothing, deleted nothing
- after `bsky -a nobody.test`:
  - exit code is `1`
  - stderr contains `error: nobody.test is not logged in`, `hint: logged in: @me.test, @work.example`

### Scenario: logout logs one account out and keeps the other, and --all logs every one out
#### Given
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/accounts/did_plc_work.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Environment variables are set: BSKY_CACHE_DIR, BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CACHE_DIR, BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CACHE_DIR, BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "http://127.0.0.1:9", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/accounts/did_plc_work.json`:_

```text
{"service": "${work.url}", "did": "did:plc:work", "handle": "work.example", "accessJwt": "acc-work", "refreshJwt": "ref-work"}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "did:plc:me"}
```

#### When

```shell
bsky logout -a work.example
bsky logout --all
bsky logout
```

#### Then
- after `bsky logout -a work.example`:
  - exit code is `0`
  - stdout equals an exact value
  - the step changed exactly created nothing, modified nothing, deleted `cfg/accounts/did_plc_work.json`
- after `bsky logout --all`:
  - exit code is `0`
  - stdout equals an exact value
  - file `cfg/accounts/did_plc_me.json` does not exist
- after `bsky logout`:
  - exit code is `0`
  - stdout equals an exact value

#### Expected output
_expected stdout:_

```text
logged out @work.example
```

_expected stdout:_

```text
logged out @me.test
```

_expected stdout:_

```text
not logged in
```

### Scenario: a write waiting behind a slow one is sent as the account it was pressed on, after a switch
#### Given
- Stub HTTP server `home` serves 2 canned route(s) at `${home.url}` and records every request (#24).
- Stub HTTP server `work` serves 2 canned route(s) at `${work.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/accounts/did_plc_work.json` is created.
- Fixture file `cfg/accounts.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${home.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/accounts/did_plc_work.json`:_

```text
{"service": "${work.url}", "did": "did:plc:work", "handle": "work.example", "accessJwt": "acc-work", "refreshJwt": "ref-work"}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "did:plc:me"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `home` received `/xrpc/com.atproto.repo.createRecord` exactly 2 time(s)
- mock `home` received `/xrpc/com.atproto.repo.createRecord` exactly 2 time(s)
- mock `work` received `/xrpc/com.atproto.repo.createRecord` exactly 0 time(s)
