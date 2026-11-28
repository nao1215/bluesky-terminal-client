# atago Behavior Specs
## Summary
1 suite · 8 scenarios
## Contents
- [msky author](#msky-author): 8 scenarios
  - [author prints the account's notes as text, emoji and CJK whole](#scenario-author-prints-the-accounts-notes-as-text-emoji-and-cjk-whole)
  - [author reads pages back until -n notes came, and stops on a page with nothing new](#scenario-author-reads-pages-back-until--n-notes-came-and-stops-on-a-page-with-nothing-new)
  - [author --reposts keeps the renotes, each under a line that says who renoted it](#scenario-author---reposts-keeps-the-renotes-each-under-a-line-that-says-who-renoted-it)
  - [author --filter video and threads keep what Misskey cannot filter by itself](#scenario-author---filter-video-and-threads-keep-what-misskey-cannot-filter-by-itself)
  - [author --json prints one note per line in the shape every command uses](#scenario-author---json-prints-one-note-per-line-in-the-shape-every-command-uses)
  - [author asks the server --host, MSKY_HOST or the account's address names](#scenario-author-asks-the-server---host-msky_host-or-the-accounts-address-names)
  - [an account the server does not know is an API error with what to check](#scenario-an-account-the-server-does-not-know-is-an-api-error-with-what-to-check)
  - [author --download saves the notes' files instead of printing them, and never writes over one](#scenario-author---download-saves-the-notes-files-instead-of-printing-them-and-never-writes-over-one)

## msky author
Source: `crates/misskey-terminal-client/e2e/atago/author.atago.yaml`
### Scenario: author prints the account's notes as text, emoji and CJK whole
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky author @alice --host ${api.url}
```

#### Then
- exit code is `0`
- stdout equals an exact value
- stderr is empty
- mock `api` received `/api/users/show` exactly 1 time(s)
- mock `api` received `/api/users/notes`

#### Expected output
_expected stdout:_

```text
家族👨‍👩‍👧 アリス @alice · 2026-09-20 10:00
山の写真 🇯🇵👍🏽
二行目 مرحبا
[1 picture]
:blobcat: 3  👍 2  :igyo@other.example: 1
♡ 6  ⟳ 1  ↩ 4
${api.url}/notes/9c

家族👨‍👩‍👧 アリス @alice · 2026-09-20 09:00
CW: ネタバレ
結末は…
♡ 0  ⟳ 0  ↩ 0
${api.url}/notes/9b
```

### Scenario: author reads pages back until -n notes came, and stops on a page with nothing new
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky author alice -n 3 --host ${api.url}
msky author alice -n 1 --host ${api.url}
```

#### Then
- after `msky author alice -n 3 --host ${api.url}`:
  - exit code is `0`
  - stdout contains `three`, `two`, does not contain `one`
  - mock `api` received `/api/users/notes` exactly 2 time(s)
- after `msky author alice -n 1 --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/api/users/notes` exactly 3 time(s)

#### Expected output
_expected stdout:_

```text
@alice · 2026-09-20 10:00
three
♡ 0  ⟳ 0  ↩ 0
${api.url}/notes/9c
```

### Scenario: author --reposts keeps the renotes, each under a line that says who renoted it
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky author alice --reposts --filter no-replies --host ${api.url}
msky author alice --filter media --host ${api.url}
msky author alice --filter replies --host ${api.url}
```

#### Then
- after `msky author alice --reposts --filter no-replies --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/api/users/notes`
- after `msky author alice --filter media --host ${api.url}`:
  - exit code is `0`
  - mock `api` received `/api/users/notes`
- after `msky author alice --filter replies --host ${api.url}`:
  - exit code is `1`
  - stderr contains `invalid value 'replies' for '--filter <FILTER>'`
  - mock `api` received `/api/users/notes` exactly 4 time(s)

#### Expected output
_expected stdout:_

```text
⟳ renoted by @alice
Bob 🐈‍⬛ @bob@other.example · 2026-09-19 08:00
リノートされた
♡ 0  ⟳ 5  ↩ 0
${api.url}/notes/8a
```

### Scenario: author --filter video and threads keep what Misskey cannot filter by itself
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky author alice --filter video --json --host ${api.url}
msky author alice --filter threads --host ${api.url}
```

#### Then
- after `msky author alice --filter video --json --host ${api.url}`:
  - exit code is `0`
  - stdout at `$.id` equals `9e`
  - mock `api` received `/api/users/notes`
- after `msky author alice --filter threads --host ${api.url}`:
  - exit code is `0`
  - stdout contains `動画 🎬`, `写真`, `自分への返信`, `ふつう`, does not contain `他人への返信`
  - mock `api` received `/api/users/notes`

### Scenario: author --json prints one note per line in the shape every command uses
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky author alice --json -n 2 --host ${api.url}
msky author alice --json -n 1 --host ${api.url}
```

#### Then
- after `msky author alice --json -n 2 --host ${api.url}`:
  - exit code is `0`
  - stdout matches `/^\{[^\n]*\}\n\{[^\n]*\}\n$/`
- after `msky author alice --json -n 1 --host ${api.url}`:
  - exit code is `0`
  - stdout at `$.id` equals `9c`; at `$.createdAt` equals `2026-09-20T10:00:00.000Z`; at `$.url` equals `${api.url}/notes/9c`; at `$.text` equals `"hi\x1b]52;c;cHduZWQ=\a 🇯🇵"`; at `$.cw` equals `null`; at `$.likeCount` equals `5`; at `$.reactions['👍']` equals `2`; at `$.reactions[':blobcat@.:']` equals `3`; at `$.repostCount` equals `1`; at `$.repliesCount` equals `4`; at `$.files[0].url` equals `${api.url}/files/f1.png`; at `$.files[0].type` equals `image`; at `$.files[0].mime` equals `image/png`; at `$.files[0].size` equals `242`; at `$.user.username` equals `alice`; at `$.user.host` equals `127.0.0.1:${api.port}`; at `$.user.name` equals `アリス`
  - stdout contains `hi\u001b]52;c;cHduZWQ=\u0007 🇯🇵`

### Scenario: author asks the server --host, MSKY_HOST or the account's address names
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_HOST, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky author @bob@Other.Example
msky author ${api.url}/@bob --host 127.0.0.1:9
msky author bob@127.0.0.1:${api.port} --host ${api.url}
```

#### Then
- after `msky author @bob@Other.Example`:
  - exit code is `0`
  - stdout is empty
  - mock `api` received `/api/users/show`
- after `msky author ${api.url}/@bob --host 127.0.0.1:9`:
  - exit code is `0`
- after `msky author bob@127.0.0.1:${api.port} --host ${api.url}`:
  - exit code is `0`
  - mock `api` received `/api/users/show` exactly 3 time(s)

### Scenario: an account the server does not know is an API error with what to check
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky author nobody --host ${api.url}
msky author nobody --json --host ${api.url}
msky author "@アリス" --host ${api.url}
```

#### Then
- after `msky author nobody --host ${api.url}`:
  - exit code is `4`
  - stdout is empty
  - stderr equals an exact value
- after `msky author nobody --json --host ${api.url}`:
  - exit code is `4`
  - stdout at `$.error` equals `users/show failed: NO_SUCH_USER: No such user.`; at `$.status` equals `4`
- after `msky author "@アリス" --host ${api.url}`:
  - exit code is `1`
  - stderr contains `is not an account`
  - mock `api` received `/api/users/show` exactly 2 time(s)

#### Expected output
_expected stderr:_

```text
error: users/show failed: NO_SUCH_USER: No such user.
hint: check the name, and that it is on 127.0.0.1:${api.port} (or pass --host)
```

### Scenario: author --download saves the notes' files instead of printing them, and never writes over one
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `expected/photo.png` is created.
- Environment variables are set: TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_DOWNLOAD_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky author alice -n 2 -o out --host ${api.url}
msky author alice -n 2 --download --json --host ${api.url}
```

#### Then
- after `msky author alice -n 2 -o out --host ${api.url}`:
  - exit code is `0`
  - stdout matches `/^out[/\\]9c-1\.png\nout[/\\]9c-2\.mp4\n$/`
  - dir `out` contains `9c-1.png`, contains `9c-2.mp4`, has 2 entries
  - file `out/9c-1.png` is byte-identical to `expected/photo.png`
- after `msky author alice -n 2 --download --json --host ${api.url}`:
  - exit code is `0`
  - stdout matches `/^\{"path":"out[/\\]+9c-1 \(1\)\.png","postId":"9c","type":"image","url":"[^"]*/files/f1\.png"\}\n\{"path":"out[/\\]+9c-2 \(1\)\.mp4","postId":"9c","type":"video","url":"[^"]*/files/f2"\}\n$/`
  - dir `out` contains `9c-1 (1).png`, contains `9c-2 (1).mp4`, has 4 entries
  - file `out/9c-1.png` is exactly 242 bytes
