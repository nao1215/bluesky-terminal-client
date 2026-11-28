# atago Behavior Specs
## Summary
1 suite · 4 scenarios
## Contents
- [msky note and the server](#msky-note-and-the-server): 4 scenarios
  - [note prints one note, asked of the server its address names](#scenario-note-prints-one-note-asked-of-the-server-its-address-names)
  - [note leaves out the control characters others wrote](#scenario-note-leaves-out-the-control-characters-others-wrote)
  - [a note the server does not have, a bad host, and no server at all](#scenario-a-note-the-server-does-not-have-a-bad-host-and-no-server-at-all)
  - [msky without a command needs an interactive terminal](#scenario-msky-without-a-command-needs-an-interactive-terminal)

## msky note and the server
Source: `crates/misskey-terminal-client/e2e/atago/note.atago.yaml`
### Scenario: note prints one note, asked of the server its address names
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_HOST.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky note ${api.url}/notes/9abc?ref=x --host 127.0.0.1:9
msky note 9abc --json
```

#### Then
- after `msky note ${api.url}/notes/9abc?ref=x --host 127.0.0.1:9`:
  - exit code is `0`
  - stdout equals an exact value
- after `msky note 9abc --json`:
  - exit code is `0`
  - stdout at `$.id` equals `9abc`; at `$.quoteId` equals `q1`; at `$.repostOf` equals `null`
  - mock `api` received `/api/notes/show` exactly 2 time(s)

#### Expected output
_expected stdout:_

```text
Alice @alice · 2026-09-20 10:00
引用するよ 🇯🇵
[quotes @bob@other.example]
♡ 0  ⟳ 0  ↩ 2
${api.url}/notes/9abc
```

### Scenario: note leaves out the control characters others wrote
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky note x1 --host ${api.url}
```

#### Then
- exit code is `0`
- stdout equals an exact value

#### Expected output
_expected stdout:_

```text
Eve]0;title 👨‍👩‍👧 @eve · 2026-09-20 10:00
hi]52;c;cHduZWQ=[2J 🇯🇵
♡ 0  ⟳ 0  ↩ 0
${api.url}/notes/x1
```

### Scenario: a note the server does not have, a bad host, and no server at all
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky note nope --host ${api.url}
msky note nope --host https://misskey.io/notes
msky note nope --host http://127.0.0.1:9
msky note --json
```

#### Then
- after `msky note nope --host ${api.url}`:
  - exit code is `4`
  - stderr contains `"error: notes/show failed: NO_SUCH_NOTE: No such note.\n"`
- after `msky note nope --host https://misskey.io/notes`:
  - exit code is `1`
  - stderr equals an exact value
- after `msky note nope --host http://127.0.0.1:9`:
  - exit code is `4`
  - stderr contains `error: notes/show: cannot reach the server`, `hint: check the network connection and --host`
- after `msky note --json`:
  - exit code is `1`
  - stdout at `$.error` equals `the following required arguments were not provided: <NOTE>`; at `$.status` equals `1`

#### Expected output
_expected stderr:_

```text
error: invalid host "https://misskey.io/notes"
hint: pass a host such as misskey.io, or an http:// or https:// URL with no path
```

### Scenario: msky without a command needs an interactive terminal
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky
```

#### Then
- exit code is `2`
- stdout is empty
- stderr equals an exact value

#### Expected output
_expected stderr:_

```text
error: msky needs an interactive terminal
hint: run msky directly in a terminal, not through a pipe or redirect
```

