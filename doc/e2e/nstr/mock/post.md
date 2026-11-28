# atago Behavior Specs
## Summary
1 suite · 4 scenarios
## Contents
- [nstr post](#nstr-post): 4 scenarios
  - [post signs the note and the relay accepts it](#scenario-post-signs-the-note-and-the-relay-accepts-it)
  - [a note the relay rejects is exit 4](#scenario-a-note-the-relay-rejects-is-exit-4)
  - [posting without keyFile is exit 1 and sends nothing](#scenario-posting-without-keyfile-is-exit-1-and-sends-nothing)
  - [a key file others can read is exit 3 and sends nothing](#scenario-a-key-file-others-can-read-is-exit-3-and-sends-nothing)

## nstr post
Source: `crates/nostr-terminal-client/e2e/atago/post.atago.yaml`
### Scenario: post signs the note and the relay accepts it
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --dead`.
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr post "新しい記事 🐈‍⬛ https://debimate.jp/post/a/"
nstr post "二つ目 👨‍👩‍👧" --json
```

#### Then
- after `nstr post "新しい記事 🐈‍⬛ https://debimate.jp/post/a/"`:
  - exit code is `0`
  - stdout matches `/^nostr:note1[02-9ac-hj-np-z]{58}\n$/`
  - stderr contains `warning: ws://127.0.0.1:`, does not contain `${relay}`
  - file `published.json` at `$.content` equals `新しい記事 🐈‍⬛ https://debimate.jp/post/a/`; at `$.kind` equals `1`; at `$.tags` equals `[]`; at `$.pubkey` equals `76c5012018add43f4d5b3842e479d47850333e4b86525946862477546eed7742`
- after `nstr post "二つ目 👨‍👩‍👧" --json`:
  - exit code is `0`
  - stdout at `$.action` equals `post`; at `$.acceptedRelays` equals `1`; at `$.already` equals `false`; at `$.target` equals `null`; at `$.url` equals `null`; at `$.id` matches `/^[0-9a-f]{64}$/`; at `$.uri` matches `/^nostr:note1[02-9ac-hj-np-z]{58}$/`
  - file `published.json` at `$.content` equals `二つ目 👨‍👩‍👧`

### Scenario: a note the relay rejects is exit 4
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --reject "blocked: e2e relay takes no notes"`.
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr post "届かない"
nstr post "届かない" --json
```

#### Then
- after `nstr post "届かない"`:
  - exit code is `4`
  - stdout is empty
  - stderr equals an exact value
- after `nstr post "届かない" --json`:
  - exit code is `4`
  - stdout at `$.status` equals `4`; at `$.error` equals `no relay completed the operation: ${relay}: publish rejected: blocked: e2e relay takes no notes`
  - file `published.json` does not exist

#### Expected output
_expected stderr:_

```text
error: no relay completed the operation: ${relay}: publish rejected: blocked: e2e relay takes no notes
```

### Scenario: posting without keyFile is exit 1 and sends nothing
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --no-key`.
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr post x
```

#### Then
- exit code is `1`
- stderr equals an exact value
- file `requests.jsonl` does not exist

#### Expected output
_expected stderr:_

```text
error: posting requires keyFile or bunker in the config
hint: run `nstr --help` for usage
```

### Scenario: a key file others can read is exit 3 and sends nothing
_skipped on Windows_
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py"`.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
chmod 644 cfg/key
nstr post x
```

#### Then
- after `nstr post x`:
  - exit code is `3`
  - stderr equals an exact value
  - file `requests.jsonl` does not exist

#### Expected output
_expected stderr:_

```text
error: key file must be accessible only to its owner; run chmod 600 on it
```

