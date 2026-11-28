# atago Behavior Specs
## Summary
1 suite · 15 scenarios
## Contents
- [bsky terminal and exit codes](#bsky-terminal-and-exit-codes): 15 scenarios
  - [help documents the image protocols, the text fallback, and the exit codes](#scenario-help-documents-the-image-protocols-the-text-fallback-and-the-exit-codes)
  - [version names the binary and a semantic version](#scenario-version-names-the-binary-and-a-semantic-version)
  - [an unknown flag is a usage error that points at help](#scenario-an-unknown-flag-is-a-usage-error-that-points-at-help)
  - [a service URL without a scheme is a usage error](#scenario-a-service-url-without-a-scheme-is-a-usage-error)
  - [a service URL with more than a host is a usage error, quoted as it was typed](#scenario-a-service-url-with-more-than-a-host-is-a-usage-error-quoted-as-it-was-typed)
  - [BSKY_SERVICE is checked like --service](#scenario-bsky_service-is-checked-like---service)
  - [without a terminal bsky refuses to start and touches nothing](#scenario-without-a-terminal-bsky-refuses-to-start-and-touches-nothing)
  - [a terminal that cannot draw images gets the client as text](#scenario-a-terminal-that-cannot-draw-images-gets-the-client-as-text)
  - [a kitty terminal is detected from its answer alone](#scenario-a-kitty-terminal-is-detected-from-its-answer-alone)
  - [BSKY_GRAPHICS forces a protocol on a terminal that does not answer](#scenario-bsky_graphics-forces-a-protocol-on-a-terminal-that-does-not-answer)
  - [BSKY_GRAPHICS naming something that is not an image protocol is a usage error](#scenario-bsky_graphics-naming-something-that-is-not-an-image-protocol-is-a-usage-error)
  - [a corrupt session file is a local-file error that names the way out](#scenario-a-corrupt-session-file-is-a-local-file-error-that-names-the-way-out)
  - [logout removes the saved session and nothing else, and is idempotent](#scenario-logout-removes-the-saved-session-and-nothing-else-and-is-idempotent)
  - [a terminal under the minimum size says so and gives both sizes](#scenario-a-terminal-under-the-minimum-size-says-so-and-gives-both-sizes)
  - [the help is readable on a narrow terminal](#scenario-the-help-is-readable-on-a-narrow-terminal)

## bsky terminal and exit codes
Source: `crates/bluesky-terminal-client/e2e/atago/terminal.atago.yaml`
### Scenario: help documents the image protocols, the text fallback, and the exit codes
#### When

```shell
bsky --help
```

#### Then
- exit code is `0`
- stdout contains `kitty graphics, sixel, or iTerm2`, `On a terminal that supports none of them bsky runs as text`, `0 success, 1 usage error, 2 not an interactive terminal`, `3 local file error, 4 network or server error`, `--json`, `--service <URL>`, `-a, --account <HANDLE>`, `logout`
- stderr is empty

### Scenario: version names the binary and a semantic version
#### When

```shell
bsky --version
```

#### Then
- exit code is `0`
- stdout matches `/^bsky \d+\.\d+\.\d+\n$/`

### Scenario: an unknown flag is a usage error that points at help
#### When

```shell
bsky --no-such-flag
```

#### Then
- exit code is `1`
- stdout is empty
- stderr contains `error: unexpected argument '--no-such-flag'`, `` hint: run `bsky --help` for usage ``

### Scenario: a service URL without a scheme is a usage error
#### When

```shell
bsky --service bsky.social
```

#### Then
- exit code is `1`
- stdout is empty
- stderr contains `error: invalid service URL "bsky.social"`, `hint: pass an http:// or https:// URL`

### Scenario: a service URL with more than a host is a usage error, quoted as it was typed
#### When

```shell
bsky --service https://bsky.social/xrpc
bsky --service "https://bsky.social?x=1"
bsky --service https:///
```

#### Then
- after `bsky --service https://bsky.social/xrpc`:
  - exit code is `1`
  - stdout is empty
  - stderr contains `error: invalid service URL "https://bsky.social/xrpc"`, `hint: pass an http:// or https:// URL with no path`
- after `bsky --service "https://bsky.social?x=1"`:
  - exit code is `1`
  - stderr contains `error: invalid service URL "https://bsky.social?x=1"`
- after `bsky --service https:///`:
  - exit code is `1`
  - stderr contains `error: invalid service URL "https:///"`

### Scenario: BSKY_SERVICE is checked like --service
#### Given
- Environment variables are set: BSKY_SERVICE.

#### When

```shell
bsky
```

#### Then
- exit code is `1`
- stderr contains `error: invalid service URL "ftp://pds.example"`

### Scenario: without a terminal bsky refuses to start and touches nothing
#### Given
- Environment variables are set: BSKY_CACHE_DIR, BSKY_CONFIG_DIR.

#### When

```shell
bsky
```

#### Then
- exit code is `2`
- stdout is empty
- stderr contains `error: bsky needs an interactive terminal`, `hint: run bsky directly in a terminal, not through a pipe or redirect`
- the step changed exactly created nothing, modified nothing, deleted nothing

### Scenario: a terminal that cannot draw images gets the client as text
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- rendered screen shows exactly 0 image(s)
- mock `api` received `/img/a.png` exactly 0 time(s)
- mock `api` received `/img/1.png` exactly 0 time(s)

### Scenario: a kitty terminal is detected from its answer alone
#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- the step changed exactly created nothing, modified nothing, deleted nothing

### Scenario: BSKY_GRAPHICS forces a protocol on a terminal that does not answer
#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`

### Scenario: BSKY_GRAPHICS naming something that is not an image protocol is a usage error
#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `1`
- rendered screen contains `error: BSKY_GRAPHICS="halfblocks" is not a graphics protocol`, `hint: set BSKY_GRAPHICS to kitty, sixel, or iterm2, or unset it`

### Scenario: a corrupt session file is a local-file error that names the way out
#### Given
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CACHE_DIR, BSKY_CONFIG_DIR.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{not json
```

#### When

```shell
# interactive (pty): bsky
bsky logout --all
```

#### Then
- after `interactive (pty): bsky`:
  - exit code is `3`
  - rendered screen contains `is not a valid session file`, `` hint: run `bsky logout --all` to discard it and log in again ``
  - the step changed exactly created nothing, modified nothing, deleted nothing
- after `bsky logout --all`:
  - exit code is `0`
  - file `cfg/accounts/did_plc_me.json` does not exist

### Scenario: logout removes the saved session and nothing else, and is idempotent
#### Given
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/unrelated.txt` is created.
- Environment variables are set: BSKY_CACHE_DIR, BSKY_CONFIG_DIR.
- Environment variables are set: BSKY_CACHE_DIR, BSKY_CONFIG_DIR.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "https://pds.example", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/unrelated.txt`:_

```text
keep me
```

#### When

```shell
bsky logout
bsky logout
```

#### Then
- after `bsky logout`:
  - exit code is `0`
  - stdout equals an exact value
  - stderr is empty
  - the step changed exactly created nothing, modified nothing, deleted `cfg/accounts/did_plc_me.json`
- after `bsky logout`:
  - exit code is `0`
  - stdout equals an exact value
  - the step changed exactly created nothing, modified nothing, deleted nothing

#### Expected output
_expected stdout:_

```text
logged out @me.test
```

_expected stdout:_

```text
not logged in
```

### Scenario: a terminal under the minimum size says so and gives both sizes
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`

### Scenario: the help is readable on a narrow terminal
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
