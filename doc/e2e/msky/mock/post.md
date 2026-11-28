# atago Behavior Specs
## Summary
1 suite · 9 scenarios
## Contents
- [msky post](#msky-post): 9 scenarios
  - [post uploads the pictures and makes the note with them](#scenario-post-uploads-the-pictures-and-makes-the-note-with-them)
  - [post reads the text from stdin with - and posts on MSKY_HOST](#scenario-post-reads-the-text-from-stdin-with---and-posts-on-msky_host)
  - [post --dry-run shows the note and the preview of its first link and sends nothing](#scenario-post---dry-run-shows-the-note-and-the-preview-of-its-first-link-and-sends-nothing)
  - [post --dry-run fails when the server cannot make the preview](#scenario-post---dry-run-fails-when-the-server-cannot-make-the-preview)
  - [post --no-card makes the links silent](#scenario-post---no-card-makes-the-links-silent)
  - [what is wrong with a note is exit 1 before anything is sent](#scenario-what-is-wrong-with-a-note-is-exit-1-before-anything-is-sent)
  - [post with a read-only login or none says to log in, and sends nothing](#scenario-post-with-a-read-only-login-or-none-says-to-log-in-and-sends-nothing)
  - [an MSKY_TOKEN without the permission says what it needs](#scenario-an-msky_token-without-the-permission-says-what-it-needs)
  - [a photo's EXIF (its GPS position among it) and a video's metadata are taken off before they are uploaded](#scenario-a-photos-exif-its-gps-position-among-it-and-a-videos-metadata-are-taken-off-before-they-are-uploaded)

## msky post
Source: `crates/misskey-terminal-client/e2e/atago/post.atago.yaml`

Secrets declared: `e2e-env-token-0001`, `e2e-saved-token-0002`.
### Scenario: post uploads the pictures and makes the note with them
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `photo.png` is created.
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post "新しい記事 🐈‍⬛ https://debimate.jp/post/a/" --image photo.png --alt "黒猫 🐈‍⬛ の写真" --cw ネタバレ --visibility home --reply 9old --host ${api.url}
msky post "二つ目" --json --host ${api.url}
```

#### Then
- after `msky post "新しい記事 🐈‍⬛ https://debimate.jp/post/a/" --image photo.png --alt "黒猫 🐈‍⬛ の写真" --cw ネタバレ --visibility home --reply 9old --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - stderr is empty
  - mock `api` received `/api/drive/files/create` exactly 1 time(s)
  - mock `api` received `/api/notes/create` exactly 1 time(s)
- after `msky post "二つ目" --json --host ${api.url}`:
  - exit code is `0`
  - stdout at `$.url` equals `${api.url}/notes/9new`; at `$.action` equals `post`; at `$.id` equals `9new`
  - mock `api` received `/api/notes/create` exactly 2 time(s)

#### Expected output
_expected stdout:_

```text
${api.url}/notes/9new
```

### Scenario: post reads the text from stdin with - and posts on MSKY_HOST
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_HOST, MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_stdin for `msky`:_

```text
ブログを更新しました 📝
https://debimate.jp/post/b/
```

#### When

```shell
msky post -
```

#### Then
- exit code is `0`
- stdout equals an exact value
- mock `api` received `/api/notes/create`

#### Expected output
_expected stdout:_

```text
${api.url}/notes/9in
```

### Scenario: post --dry-run shows the note and the preview of its first link and sends nothing
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `photo.png` is created.
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post "新しい記事 https://debimate.jp/post/a/#top ?[静か](https://quiet.test/)" --image photo.png --dry-run --json --host ${api.url}
msky post "新しい記事 https://debimate.jp/post/a/" --dry-run --host ${api.url}
```

#### Then
- after `msky post "新しい記事 https://debimate.jp/post/a/#top ?[静か](https://quiet.test/)" --image photo.png --dry-run --json --host ${api.url}`:
  - exit code is `0`
  - stdout at `$.dryRun` equals `true`; at `$.host` equals `127.0.0.1:${api.port}`; at `$.username` equals `nchika`; at `$.post.text` equals `新しい記事 https://debimate.jp/post/a/#top ?[静か](https://quiet.test/)`; at `$.post.visibility` equals `public`; at `$.post.files[0].name` equals `photo.png`; at `$.previewed[0]` equals `https://debimate.jp/post/a/#top`; at `$.preview.title` equals `記事の題 👨‍👩‍👧`; at `$.preview.description` equals `記事の説明`; at `$.preview.thumbnail` equals `${api.url}/proxy/preview.webp?url=ogp.png`
  - mock `api` received `GET /url` exactly 1 time(s)
  - mock `api` received `/url`
  - mock `api` received `/api/drive/files/create` exactly 0 time(s)
  - mock `api` received `/api/notes/create` exactly 0 time(s)
- after `msky post "新しい記事 https://debimate.jp/post/a/" --dry-run --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/api/notes/create` exactly 0 time(s)

#### Expected output
_expected stdout:_

```text
dry run: nothing was sent to 127.0.0.1:${api.port}
as @nchika@127.0.0.1:${api.port}, public
text:
新しい記事 https://debimate.jp/post/a/
preview of https://debimate.jp/post/a/:
  title: 記事の題 👨‍👩‍👧
  description: 記事の説明
  image: ${api.url}/proxy/preview.webp?url=ogp.png
  site: debimate
```

### Scenario: post --dry-run fails when the server cannot make the preview
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post "https://down.test/a" --dry-run --json --host ${api.url}
```

#### Then
- exit code is `4`
- stdout contains `"preview":null`, `"previewError":`, `URL_PREVIEW_FAILED`
- stderr contains `error: 127.0.0.1:${api.port} could not make a preview of https://down.test/a: URL_PREVIEW_FAILED: Failed to get preview`, `hint: check that the page is up`

### Scenario: post --no-card makes the links silent
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post "記事 https://debimate.jp/post/a/ と [前回](https://debimate.jp/post/z/)" --no-card --host ${api.url}
```

#### Then
- exit code is `0`
- mock `api` received `/api/notes/create`

### Scenario: what is wrong with a note is exit 1 before anything is sent
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `photo.png` is created.
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Fixture file `long.txt` is created.
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_stdin for `msky`:_

```text
(read from file long.txt)
```

#### When

```shell
msky post x --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --host ${api.url}
msky post - --host ${api.url}
msky post x --cw "" --host ${api.url}
msky post "" --host ${api.url}
msky post x --visibility specified --host ${api.url}
msky post x --image gone.png --host ${api.url}
```

#### Then
- after `msky post x --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --host ${api.url}`:
  - exit code is `1`
  - stderr equals an exact value
- after `msky post - --host ${api.url}`:
  - exit code is `1`
  - stderr equals an exact value
- after `msky post x --cw "" --host ${api.url}`:
  - exit code is `1`
  - stderr contains `error: --cw is empty`
- after `msky post "" --host ${api.url}`:
  - exit code is `1`
  - stderr contains `error: the note is empty`
- after `msky post x --visibility specified --host ${api.url}`:
  - exit code is `1`
  - stderr contains `invalid value 'specified' for '--visibility <VISIBILITY>'`
- after `msky post x --image gone.png --host ${api.url}`:
  - exit code is `3`
  - stderr contains `error: cannot read gone.png`
  - mock `api` received `/api/i` exactly 0 time(s)

#### Expected output
_expected stderr:_

```text
error: a note can have at most 16 pictures
```

_expected stderr:_

```text
error: the note is 3001 characters; the limit is 3000
```

### Scenario: post with a read-only login or none says to log in, and sends nothing
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Fixture file `cfg/accounts/127.0.0.1_${api.port}.json` is created.
- Environment variables are set: MSKY_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/accounts/127.0.0.1_${api.port}.json`:_

```text
{"service": "${api.url}", "userId": "u-me", "username": "nchika", "token": "e2e-saved-token-0002"}
```

#### When

```shell
msky post hello --host ${api.url}
msky post hello --host ${api.url}
```

#### Then
- after `msky post hello --host ${api.url}`:
  - exit code is `1`
  - stderr equals an exact value
- after `msky post hello --host ${api.url}`:
  - exit code is `1`
  - stderr equals an exact value
  - mock `api` received `/api/notes/create` exactly 0 time(s)

#### Expected output
_expected stderr:_

```text
error: not logged in on 127.0.0.1:${api.port}
hint: run `msky login --host 127.0.0.1:${api.port}`
```

_expected stderr:_

```text
error: the login saved for 127.0.0.1:${api.port} does not allow msky to post notes: it lacks write:notes
hint: it was made when msky could only read; run `msky login --host 127.0.0.1:${api.port}` again and allow the access
```

### Scenario: an MSKY_TOKEN without the permission says what it needs
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post hello --host ${api.url}
```

#### Then
- exit code is `4`
- stderr contains `error: notes/create failed: PERMISSION_DENIED`, `hint: the token in MSKY_TOKEN was not given that permission; make one in Settings > API on 127.0.0.1:${api.port} with read:account, write:notes, write:drive and write:following`, does not contain `e2e-env-token-0001`

### Scenario: a photo's EXIF (its GPS position among it) and a video's metadata are taken off before they are uploaded
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `gps.jpg` is created.
- Fixture file `gps.mp4` is created.
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post "写真 📷" --image gps.jpg --host ${api.url}
msky post "動画 🎥" --video gps.mp4 --host ${api.url}
```

#### Then
- after `msky post "写真 📷" --image gps.jpg --host ${api.url}`:
  - exit code is `0`
  - mock `api` received `/api/drive/files/create` exactly 1 time(s)
- after `msky post "動画 🎥" --video gps.mp4 --host ${api.url}`:
  - exit code is `0`
  - mock `api` received `/api/drive/files/create`
