# atago Behavior Specs
## Summary
1 suite · 11 scenarios
## Contents
- [msky client](#msky-client): 11 scenarios
  - [the home timeline draws its notes and pictures, and the keys act on them](#scenario-the-home-timeline-draws-its-notes-and-pictures-and-the-keys-act-on-them)
  - [without an account the login form lists the servers, and esc reads as a visitor](#scenario-without-an-account-the-login-form-lists-the-servers-and-esc-reads-as-a-visitor)
  - [the login form logs in with MiAuth on a server typed, saves it, and reads its home](#scenario-the-login-form-logs-in-with-miauth-on-a-server-typed-saves-it-and-reads-its-home)
  - [the account list switches to another server's account and keeps it in use](#scenario-the-account-list-switches-to-another-servers-account-and-keeps-it-in-use)
  - [the viewer plays a note's MP4 video](#scenario-the-viewer-plays-a-notes-mp4-video)
  - [the timeline draws a video by its thumbnail beside a picture](#scenario-the-timeline-draws-a-video-by-its-thumbnail-beside-a-picture)
  - [custom emoji are drawn as pictures from the server's list](#scenario-custom-emoji-are-drawn-as-pictures-from-the-servers-list)
  - [custom emoji in names, quotes and notifications are pictures, and the counts beside them go with the tab](#scenario-custom-emoji-in-names-quotes-and-notifications-are-pictures-and-the-counts-beside-them-go-with-the-tab)
  - [s opens the settings, T picks a theme and the language turns the client Japanese, kept in settings.json; an old tabs setting is ignored and kept](#scenario-s-opens-the-settings-t-picks-a-theme-and-the-language-turns-the-client-japanese-kept-in-settingsjson-an-old-tabs-setting-is-ignored-and-kept)
  - [the client is in the language the locale asks for](#scenario-the-client-is-in-the-language-the-locale-asks-for)
  - [the thread where the selection rests is read ahead, so v asks nothing; the Notifications tab counts the unread](#scenario-the-thread-where-the-selection-rests-is-read-ahead-so-v-asks-nothing-the-notifications-tab-counts-the-unread)

## msky client
Source: `crates/misskey-terminal-client/e2e/atago/tui.atago.yaml`

Secrets declared: `e2e-fake-token-0101`, `e2e-fake-token-0102`, `e2e-fake-token-0109`.
### Scenario: the home timeline draws its notes and pictures, and the keys act on them
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Fixture file `expected/photo.png` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0101"}
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
- rendered screen shows exactly 2 image(s) and an image width 40px, height 40px, like testdata/expected/alice-avatar-40.png and an image width 320px, height 240px, like testdata/expected/photo-320x240.png
- mock `api` received `/api/notes/timeline`
- mock `api` received `/api/notes/timeline` exactly 2 time(s)
- mock `api` received `/api/notes/show` where body exactly 1 time(s)
- mock `api` received `/api/notes/show` where body exactly 1 time(s)
- mock `api` received `/api/notes/children` exactly 0 time(s)
- mock `api` received `/api/notes/local-timeline` exactly 1 time(s)
- mock `api` received `/api/i/notifications`
- file `dl/a1-1.png` is byte-identical to `expected/photo.png`
- mock `api` received `/img/photo.png` exactly 2 time(s)

### Scenario: without an account the login form lists the servers, and esc reads as a visitor
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): msky --host ${api.url}
```

#### Then
- exit code is `0`
- mock `api` received `/api/notes/timeline` exactly 0 time(s)
- mock `api` received `/api/notes/local-timeline` exactly 1 time(s)
- file `cfg/accounts` does not exist

### Scenario: the login form logs in with MiAuth on a server typed, saves it, and reads its home
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): msky --host ${api.url}
```

#### Then
- exit code is `0`
- file `cfg/accounts/127.0.0.1_${api.port}.json` contains `"username": "newbie"`, `"userId": "u-new"`
- file `cfg/accounts.json` contains `127.0.0.1:${api.port}`
- mock `api` received `/api/notes/timeline`

### Scenario: the account list switches to another server's account and keeps it in use
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Stub HTTP server `other` serves 2 canned route(s) at `${other.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts/127.0.0.1_${other.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-a", "username": "ann", "token": "e2e-fake-token-0101"}
```

_Fixture `cfg/accounts/127.0.0.1_${other.port}.json`:_

```text
{"service": "${other.url}", "userId": "u-b", "username": "ben", "token": "e2e-fake-token-0102"}
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
- file `cfg/accounts.json` contains `127.0.0.1:${other.port}`
- mock `other` received `/api/notes/timeline` exactly 1 time(s)

### Scenario: the viewer plays a note's MP4 video
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0101"}
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
- mock `api` received `/files/clip.mp4`

### Scenario: the timeline draws a video by its thumbnail beside a picture
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0101"}
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
- rendered screen shows exactly 2 image(s) and an image width 320px, height 240px, like testdata/expected/photo-320x240.png
- mock `api` received `/img/poster.png` exactly 1 time(s)
- mock `api` received `/img/photo.png` exactly 1 time(s)
- mock `api` received `/files/clip.mp4` exactly 0 time(s)

### Scenario: custom emoji are drawn as pictures from the server's list
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0101"}
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
- rendered screen shows exactly 1 image(s) and an image width 20px, height 20px
- mock `api` received `/emoji/blobcat.png` exactly 1 time(s)

### Scenario: custom emoji in names, quotes and notifications are pictures, and the counts beside them go with the tab
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0101", "permissions": ["read:account", "read:notifications"]}
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

### Scenario: s opens the settings, T picks a theme and the language turns the client Japanese, kept in settings.json; an old tabs setting is ignored and kept
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- Fixture file `cfg/settings.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0401", "permissions": []}
```

_Fixture `cfg/accounts.json`:_

```text
{"current": "127.0.0.1:${api.port}"}
```

_Fixture `cfg/settings.json`:_

```text
{"tabs": ["home", "chat"], "later": {"kept": true}}
```

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`
- file `cfg/settings.json` at `$.language` equals `ja`; at `$.theme` equals `ayu-dark`; at `$.tabs` equals `[home chat]`; at `$.later.kept` equals `true`

### Scenario: the client is in the language the locale asks for
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with a cleared environment (passing through: PATH, HOME, USERPROFILE, APPDATA, LOCALAPPDATA).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`

### Scenario: the thread where the selection rests is read ahead, so v asks nothing; the Notifications tab counts the unread
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Fixture file `cfg/accounts.json` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "me", "token": "e2e-fake-token-0801", "permissions": ["read:account"]}
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
- mock `api` received `/api/notes/show` exactly 1 time(s)
