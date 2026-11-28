# atago Behavior Specs
## Summary
1 suite · 5 scenarios
## Contents
- [nstr read](#nstr-read): 5 scenarios
  - [timeline shows alice's and her contacts' notes, newest first](#scenario-timeline-shows-alices-and-her-contacts-notes-newest-first)
  - [without a contact list the timeline is alice's own notes](#scenario-without-a-contact-list-the-timeline-is-alices-own-notes)
  - [notifications are the notes that tag alice, as replies and mentions](#scenario-notifications-are-the-notes-that-tag-alice-as-replies-and-mentions)
  - [no relay answering is exit 4](#scenario-no-relay-answering-is-exit-4)
  - [a pipe closed early ends the output quietly with exit 0](#scenario-a-pipe-closed-early-ends-the-output-quietly-with-exit-0)

## nstr read
Source: `crates/nostr-terminal-client/e2e/atago/read.atago.yaml`
### Scenario: timeline shows alice's and her contacts' notes, newest first
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py"`.
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr timeline -n 2
nstr timeline --json
nstr timeline -n 1 --json
```

#### Then
- after `nstr timeline -n 2`:
  - exit code is `0`
  - stdout equals an exact value
  - stderr is empty
  - file `requests.jsonl` contains `{"authors": ["27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589", "76c5012018add43f4d5b3842e479d47850333e4b86525946862477546eed7742"], "kinds": [1], "limit": 2}`
  - file `requests.jsonl` does not contain `not-a-key`
- after `nstr timeline --json`:
  - exit code is `0`
  - stdout does not contain `carol`, matches `/^\{[^\n]*"text":"返信です"[^\n]*\}\n\{[^\n]*"text":"bob の二つ目 👨‍👩‍👧"[^\n]*\}\n\{[^\n]*"text":"bob の一つ目"[^\n]*\}\n\{[^\n]*"text":"はじめての投稿 🐈‍⬛"[^\n]*\}\n$/`
  - stderr is empty
- after `nstr timeline -n 1 --json`:
  - exit code is `0`
  - stdout at `$.id` equals `1da43a2287cb45e1f5759c68787562a1c94265d0c0b4e7e2ea89a96da533b064`; at `$.text` equals `返信です`; at `$.replyId` equals `99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662`; at `$.createdAt` equals `2026-01-01T00:10:00Z`; at `$.user.id` equals `27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589`; at `$.user.username` equals `npub1yluhcpenal4x57hawx40489ppxpfsgn5emgnahx6ce09kcetkkysac2sl5`; at `$.uri` equals `nostr:note1rkjr5g58edz7rat4n358satz58y5yewscz6w0ch23x5kmffnkpjqm23yjp`; at `$.note` equals `note1rkjr5g58edz7rat4n358satz58y5yewscz6w0ch23x5kmffnkpjqm23yjp`; at `$.kind` equals `1`; at `$.cw` equals `null`; at `$.url` equals `null`; at `$.tags[0][3]` equals `reply`

#### Expected output
_expected stdout:_

```text
npub1yluhcpenal4x57hawx40489ppxpfsgn5emgnahx6ce09kcetkkysac2sl5 · 2026-01-01 00:10
返信です
nostr:note1rkjr5g58edz7rat4n358satz58y5yewscz6w0ch23x5kmffnkpjqm23yjp
npub1yluhcpenal4x57hawx40489ppxpfsgn5emgnahx6ce09kcetkkysac2sl5 · 2026-01-01 00:05
bob の二つ目 👨‍👩‍👧
nostr:note1hn0f0axwprt4m0a8zazmgz47up8mr5kfpej7tx0cs4nftjrz8ztseakpkp
```

### Scenario: without a contact list the timeline is alice's own notes
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --no-contacts`.
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr timeline
```

#### Then
- exit code is `0`
- stdout equals an exact value

#### Expected output
_expected stdout:_

```text
npub1wmzszgqc4h2r7n2m8ppwg7w50pgrx0jtsef9j35xy3m4gmhdwapqx894z8 · 2026-01-01 00:01
はじめての投稿 🐈‍⬛
nostr:note1n9tsk39qcxlxmq2yz7a4cq4ta3zdxtzvyhavdzs35vz2nrk4xe3qhljmgt
```

### Scenario: notifications are the notes that tag alice, as replies and mentions
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py"`.
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr notifications
nstr notifications -n 1 --json
```

#### Then
- after `nstr notifications`:
  - exit code is `0`
  - stdout equals an exact value
  - file `requests.jsonl` contains `{"#p": ["76c5012018add43f4d5b3842e479d47850333e4b86525946862477546eed7742"], "kinds": [1, 3, 6, 7, 16], "limit": 20}`
- after `nstr notifications -n 1 --json`:
  - exit code is `0`
  - stdout at `$.type` equals `reply`; at `$.id` equals `1da43a2287cb45e1f5759c68787562a1c94265d0c0b4e7e2ea89a96da533b064`; at `$.postId` equals `1da43a2287cb45e1f5759c68787562a1c94265d0c0b4e7e2ea89a96da533b064`; at `$.createdAt` equals `2026-01-01T00:10:00Z`; at `$.user.id` equals `27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589`; at `$.post.replyId` equals `99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662`; at `$.post.text` equals `返信です`

#### Expected output
_expected stdout:_

```text
reply: npub1yluhcpenal4x57hawx40489ppxpfsgn5emgnahx6ce09kcetkkysac2sl5 · 2026-01-01 00:10
返信です
nostr:note1rkjr5g58edz7rat4n358satz58y5yewscz6w0ch23x5kmffnkpjqm23yjp
mention: npub1mgdpefvvjt9trkjpvrqpacxtxtwwlj6df9xu6u06czvklntw3lqq2h5lyn · 2026-01-01 00:08
@alice こんにちは
nostr:note1jtx3gxzld40m7f3wer77fjtnw7kjf8tpk9f509p438ufqtz9w9qs0uxv9n
```

### Scenario: no relay answering is exit 4
#### Given
- Fixture file `cfg/config.json` is created.
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `cfg/config.json`:_

```text
{"publicKey": "76c5012018add43f4d5b3842e479d47850333e4b86525946862477546eed7742", "relays": ["ws://127.0.0.1:1/"], "timeoutSeconds": 5}
```

#### When

```shell
nstr timeline --json
```

#### Then
- exit code is `4`
- stdout at `$.status` equals `4`
- stderr contains `error: no relay completed the operation: ws://127.0.0.1:1/: `

### Scenario: a pipe closed early ends the output quietly with exit 0
_skipped on Windows_
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --many`.
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
{ nstr timeline -n 100 --json; echo "nstr=$?" >&2; } | head -c 1
{ nstr timeline -n 100; echo "nstr=$?" >&2; } | head -c 1
```

#### Then
- after `{ nstr timeline -n 100 --json; echo "nstr=$?" >&2; } | head -c 1`:
  - stderr contains `nstr=0`, does not contain `panicked`, `Broken pipe`
- after `{ nstr timeline -n 100; echo "nstr=$?" >&2; } | head -c 1`:
  - stderr contains `nstr=0`, does not contain `panicked`, `Broken pipe`
