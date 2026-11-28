# atago Behavior Specs
## Summary
1 suite · 4 scenarios
## Contents
- [msky client search and profiles](#msky-client-search-and-profiles): 4 scenarios
  - [search finds notes and accounts, enter opens a profile, f follows it](#scenario-search-finds-notes-and-accounts-enter-opens-a-profile-f-follows-it)
  - [m on someone's profile opens the conversation with them, and the profile says how you are related](#scenario-m-on-someones-profile-opens-the-conversation-with-them-and-the-profile-says-how-you-are-related)
  - [+ on the Home tab adds an antenna as a column beside the home timeline, kept in settings.json; ← → choose the column, > moves it, x then y removes it](#scenario--on-the-home-tab-adds-an-antenna-as-a-column-beside-the-home-timeline-kept-in-settingsjson---choose-the-column--moves-it-x-then-y-removes-it)
  - [the columns settings.json keeps are on the Home tab from the start; a narrow terminal shows fewer and counts the rest; an old tabs setting is ignored](#scenario-the-columns-settingsjson-keeps-are-on-the-home-tab-from-the-start-a-narrow-terminal-shows-fewer-and-counts-the-rest-an-old-tabs-setting-is-ignored)

## msky client search and profiles
Source: `crates/misskey-terminal-client/e2e/atago/tui-browse.atago.yaml`

Secrets declared: `e2e-fake-token-0401`.
### Scenario: search finds notes and accounts, enter opens a profile, f follows it
#### Given
- Stub HTTP server `api` serves 8 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0401",
 "permissions": ["read:account", "read:notifications", "read:following", "read:reactions", "write:notes", "write:drive", "write:following", "write:reactions", "write:notifications"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`
- mock `api` received `/api/notes/search`
- mock `api` received `/api/users/search`
- mock `api` received `/api/users/notes`
- mock `api` received `/api/notes/timeline` exactly 2 time(s)
- mock `api` received `/api/following/requests/cancel` exactly 1 time(s)
- mock `api` received `/api/following/create` exactly 0 time(s)

### Scenario: m on someone's profile opens the conversation with them, and the profile says how you are related
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0401",
 "permissions": ["read:account", "read:chat", "write:chat"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`
- mock `api` received `/api/chat/messages/user-timeline` exactly 1 time(s)
- mock `api` received `/api/chat/messages/create-to-user` exactly 0 time(s)

### Scenario: + on the Home tab adds an antenna as a column beside the home timeline, kept in settings.json; ← → choose the column, > moves it, x then y removes it
#### Given
- Stub HTTP server `api` serves 8 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0601", "permissions": ["read:account"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "127.0.0.1:${api.port}"}
```

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`
- mock `api` received `/api/antennas/notes`
- file `cfg/settings.json` contains `"me@127.0.0.1:${api.port}"`, `"kind": "antenna"`, `"id": "ant1"`, `"name": "ニュース 📰"`
- file `cfg/settings.json` does not contain `"kind": "home"`

### Scenario: the columns settings.json keeps are on the Home tab from the start; a narrow terminal shows fewer and counts the rest; an old tabs setting is ignored
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Fixture file `cfg/settings.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0602", "permissions": ["read:account"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "127.0.0.1:${api.port}"}
```

_Fixture `cfg/settings.json`:_

```text
{"tabs": ["list/127.0.0.1:${api.port}/lst1/友達", "home"], "columns": {"me@127.0.0.1:${api.port}": [{"kind": "list", "id": "lst1", "name": "友達"}, {"kind": "notifications"}, {"kind": "search", "query": "猫"}]}}
```

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`
- mock `api` received `/api/notes/user-list-timeline`
- mock `api` received `/api/notes/search`
- mock `api` received `/api/notifications/mark-all-as-read` exactly 0 time(s)
