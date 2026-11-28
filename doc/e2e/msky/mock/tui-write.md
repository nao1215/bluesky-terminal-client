# atago Behavior Specs
## Summary
1 suite · 7 scenarios
## Contents
- [msky client writes](#msky-client-writes): 7 scenarios
  - [n posts a note with a content warning, seen by home only](#scenario-n-posts-a-note-with-a-content-warning-seen-by-home-only)
  - [r replies to the note shown, as public as it is](#scenario-r-replies-to-the-note-shown-as-public-as-it-is)
  - [l reacts, e chooses a reaction, b renotes and takes it back, D deletes after y, and 4 marks read](#scenario-l-reacts-e-chooses-a-reaction-b-renotes-and-takes-it-back-d-deletes-after-y-and-4-marks-read)
  - [the Chat tab lists the conversations, opens one and sends a message](#scenario-the-chat-tab-lists-the-conversations-opens-one-and-sends-a-message)
  - [the Chat tab draws the custom emoji of names and messages as pictures](#scenario-the-chat-tab-draws-the-custom-emoji-of-names-and-messages-as-pictures)
  - [e on your own profile edits the name and uploads a new avatar, sending only what changed](#scenario-e-on-your-own-profile-edits-the-name-and-uploads-a-new-avatar-sending-only-what-changed)
  - [! reports the note's author with a reason, sent only when y confirms](#scenario--reports-the-notes-author-with-a-reason-sent-only-when-y-confirms)

## msky client writes
Source: `crates/misskey-terminal-client/e2e/atago/tui-write.atago.yaml`

Secrets declared: `e2e-fake-token-0301`.
### Scenario: n posts a note with a content warning, seen by home only
#### Given
- Stub HTTP server `api` serves 9 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0301",
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
- mock `api` received `/api/notes/create` exactly 1 time(s)
- mock `api` received `/api/meta` exactly 1 time(s)

### Scenario: r replies to the note shown, as public as it is
#### Given
- Stub HTTP server `api` serves 9 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0301",
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
- mock `api` received `/api/notes/create` exactly 1 time(s)

### Scenario: l reacts, e chooses a reaction, b renotes and takes it back, D deletes after y, and 4 marks read
#### Given
- Stub HTTP server `api` serves 9 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0301",
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
- mock `api` received `/api/notes/reactions/create` exactly 2 time(s)
- mock `api` received `/api/notes/reactions/delete` exactly 1 time(s)
- mock `api` received `/api/notes/create` exactly 1 time(s)
- mock `api` received `/api/notes/unrenote` exactly 1 time(s)
- mock `api` received `/api/notes/delete` exactly 1 time(s)
- mock `api` received `/api/notifications/mark-all-as-read` exactly 1 time(s)

### Scenario: the Chat tab lists the conversations, opens one and sends a message
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0301",
 "permissions": ["read:chat", "write:chat"]}
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
- mock `api` received `/api/chat/messages/create-to-user` exactly 1 time(s)

### Scenario: the Chat tab draws the custom emoji of names and messages as pictures
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0301",
 "permissions": ["read:chat", "write:chat"]}
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
- mock `api` received `/emoji/iruka.png` exactly 1 time(s)
- mock `api` received `/emoji/budou.png` exactly 1 time(s)
- mock `api` received `/api/chat/messages/create-to-user` exactly 0 time(s)

### Scenario: e on your own profile edits the name and uploads a new avatar, sending only what changed
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Fixture file `avatar.png` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0501",
 "permissions": ["read:account", "write:account", "write:drive"]}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "127.0.0.1:${api.port}"}
```

_Fixture `avatar.png`:_

```text
PNG fake
```

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`
- mock `api` received `/api/drive/files/create` exactly 1 time(s)
- mock `api` received `/api/i/update` exactly 1 time(s)
- mock `api` received `/api/i/update`

### Scenario: ! reports the note's author with a reason, sent only when y confirms
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0701", "permissions": ["write:report-abuse"]}
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
- mock `api` received `/api/users/report-abuse` exactly 1 time(s)
