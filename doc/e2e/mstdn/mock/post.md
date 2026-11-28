# atago Behavior Specs
## Summary
1 suite · 3 scenarios
## Contents
- [mstdn post](#mstdn-post): 3 scenarios
  - [post uploads the picture, waits for it, and makes the status with it](#scenario-post-uploads-the-picture-waits-for-it-and-makes-the-status-with-it)
  - [what the server does not take is exit 1 before anything is sent](#scenario-what-the-server-does-not-take-is-exit-1-before-anything-is-sent)
  - [a status the server refuses is exit 4 and is not sent again](#scenario-a-status-the-server-refuses-is-exit-4-and-is-not-sent-again)

## mstdn post
Source: `crates/mastodon-terminal-client/e2e/atago/post.atago.yaml`

Secrets declared: `e2e-env-token-0001`.
### Scenario: post uploads the picture, waits for it, and makes the status with it
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `gps.jpg` is created.
- Environment variables are set: MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_HOST, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_stdin for `mstdn`:_

```text
ブログを更新しました 📝
https://debimate.jp/post/b/
```

#### When

```shell
mstdn post "新しい記事 🐈‍⬛ https://debimate.jp/post/a/" --image gps.jpg --alt "黒猫 🐈‍⬛ の写真" --cw ネタバレ --visibility unlisted --reply 100 --host ${api.url}
mstdn post - --json
```

#### Then
- after `mstdn post "新しい記事 🐈‍⬛ https://debimate.jp/post/a/" --image gps.jpg --alt "黒猫 🐈‍⬛ の写真" --cw ネタバレ --visibility unlisted --reply 100 --host ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - stderr is empty
  - mock `api` received `/api/v2/media` exactly 1 time(s)
  - mock `api` received `GET /api/v1/media/m1` exactly 1 time(s)
  - mock `api` received `POST /api/v1/statuses` exactly 1 time(s)
- after `mstdn post - --json`:
  - exit code is `0`
  - stdout at `$.action` equals `post`; at `$.id` equals `900`; at `$.url` equals `${api.url}/@nchika/900`; at `$.uri` equals `${api.url}/users/nchika/statuses/900`
  - mock `api` received `/api/v1/statuses` exactly 2 time(s)

#### Expected output
_expected stdout:_

```text
${api.url}/@nchika/900
```

### Scenario: what the server does not take is exit 1 before anything is sent
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Fixture file `photo.png` is created.
- Environment variables are set: MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn post "短い https://debimate.jp/a/very/long/address/that/counts/as/twenty-three" --dry-run --host ${api.url}
mstdn post "短い https://debimate.jp/a" --cw "とても長い注意書きです" --host ${api.url}
mstdn post x --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --host ${api.url}
mstdn post x --image photo.png --video photo.png --host ${api.url}
mstdn post "  " --host ${api.url}
mstdn post x --host ${api.url}
```

#### Then
- after `mstdn post "短い https://debimate.jp/a/very/long/address/that/counts/as/twenty-three" --dry-run --host ${api.url}`:
  - exit code is `0`
  - stdout contains `"text (26 of 30 characters):\n"`
- after `mstdn post "短い https://debimate.jp/a" --cw "とても長い注意書きです" --host ${api.url}`:
  - exit code is `1`
  - stderr equals an exact value
- after `mstdn post x --image photo.png --image photo.png --image photo.png --image photo.png --image photo.png --host ${api.url}`:
  - exit code is `1`
  - stderr contains `error: a post can have at most 4 pictures`
- after `mstdn post x --image photo.png --video photo.png --host ${api.url}`:
  - exit code is `1`
  - stderr contains `not both`
- after `mstdn post "  " --host ${api.url}`:
  - exit code is `1`
  - stderr contains `error: the post is empty`
- after `mstdn post x --host ${api.url}`:
  - exit code is `1`
  - stderr contains `error: not logged in on 127.0.0.1:${api.port}`
  - mock `api` received `/api/v1/statuses` exactly 0 time(s)
  - mock `api` received `/api/v2/media` exactly 0 time(s)

#### Expected output
_expected stderr:_

```text
error: the post is 37 characters with its content warning; 127.0.0.1:${api.port} takes up to 30
hint: an address counts as 23 characters, and @name@host as @name
```

### Scenario: a status the server refuses is exit 4 and is not sent again
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn post x --host ${api.url}
```

#### Then
- exit code is `4`
- stderr equals an exact value
- mock `api` received `/api/v1/statuses` exactly 1 time(s)

#### Expected output
_expected stderr:_

```text
error: v1/statuses failed: Validation failed: Text character limit of 500 exceeded
```

