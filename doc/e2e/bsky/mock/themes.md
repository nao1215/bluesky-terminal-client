# atago Behavior Specs
## Summary
1 suite · 9 scenarios
## Contents
- [bsky themes](#bsky-themes): 9 scenarios
  - [applying a theme saves it in settings.json](#scenario-applying-a-theme-saves-it-in-settingsjson)
  - [the settings screen chooses a theme in the picker and comes back to it](#scenario-the-settings-screen-chooses-a-theme-in-the-picker-and-comes-back-to-it)
  - [previewing and cancelling writes nothing](#scenario-previewing-and-cancelling-writes-nothing)
  - [a saved theme is back on the next start](#scenario-a-saved-theme-is-back-on-the-next-start)
  - [a broken settings file is reported, ignored, and left alone (cfg)](#scenario-a-broken-settings-file-is-reported-ignored-and-left-alone-cfg)
  - [a broken settings file is reported, ignored, and left alone (settings-in-a-long-directory-name)](#scenario-a-broken-settings-file-is-reported-ignored-and-left-alone-settings-in-a-long-directory-name)
  - [an unknown theme name falls back to bluesky and says so](#scenario-an-unknown-theme-name-falls-back-to-bluesky-and-says-so)
  - [NO_COLOR draws without color and keeps the picker closed](#scenario-no_color-draws-without-color-and-keeps-the-picker-closed)
  - [logout leaves the settings alone](#scenario-logout-leaves-the-settings-alone)

## bsky themes
Source: `crates/bluesky-terminal-client/e2e/atago/themes.atago.yaml`
### Scenario: applying a theme saves it in settings.json
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
- file `cfg/settings.json` at `$.theme` equals `bluesky-light`
- file `cfg/accounts/did_plc_me.json` at `$.accessJwt` equals `acc`

### Scenario: the settings screen chooses a theme in the picker and comes back to it
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
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
- file `cfg/settings.json` at `$.theme` equals `bluesky-light`

### Scenario: previewing and cancelling writes nothing
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
- file `cfg/settings.json` does not exist

### Scenario: a saved theme is back on the next start
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/settings.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/settings.json`:_

```text
{"theme": "nord", "keep": {"written by": "a newer bsky"}}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- file `cfg/settings.json` at `$.theme` equals `oceanic-next`; at `$.keep['written by']` equals `a newer bsky`

### Scenario: a broken settings file is reported, ignored, and left alone (cfg)
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/settings.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/settings.json`:_

```text
{broken
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- the step changed exactly created nothing, modified nothing, deleted nothing

### Scenario: a broken settings file is reported, ignored, and left alone (settings-in-a-long-directory-name)
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `settings-in-a-long-directory-name/accounts/did_plc_me.json` is created.
- Fixture file `settings-in-a-long-directory-name/settings.json` is created.

#### Inputs
_Fixture `settings-in-a-long-directory-name/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `settings-in-a-long-directory-name/settings.json`:_

```text
{broken
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- the step changed exactly created nothing, modified nothing, deleted nothing

### Scenario: an unknown theme name falls back to bluesky and says so
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/settings.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/settings.json`:_

```text
{"theme": "neon"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`

### Scenario: NO_COLOR draws without color and keeps the picker closed
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/settings.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/settings.json`:_

```text
{"theme": "dracula"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- the step changed exactly created nothing, modified nothing, deleted nothing

### Scenario: logout leaves the settings alone
#### Given
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/settings.json` is created.
- Environment variables are set: BSKY_CACHE_DIR, BSKY_CONFIG_DIR.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "https://pds.example", "did": "did:plc:me", "handle": "me.test", "accessJwt": "a", "refreshJwt": "r"}
```

_Fixture `cfg/settings.json`:_

```text
{"theme": "nord"}
```

#### When

```shell
bsky logout
```

#### Then
- exit code is `0`
- the step changed exactly created nothing, modified nothing, deleted `cfg/accounts/did_plc_me.json`
