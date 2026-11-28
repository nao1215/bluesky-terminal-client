# atago Behavior Specs
## Summary
1 suite · 5 scenarios
## Contents
- [bsky pictures](#bsky-pictures): 5 scenarios
  - [two pictures from two folders are posted with their alt text and shape](#scenario-two-pictures-from-two-folders-are-posted-with-their-alt-text-and-shape)
  - [four pictures at most, and one that cannot be read stops the post before any upload](#scenario-four-pictures-at-most-and-one-that-cannot-be-read-stops-the-post-before-any-upload)
  - [the avatar is chosen in the browser, hidden files on request, esc changes nothing](#scenario-the-avatar-is-chosen-in-the-browser-hidden-files-on-request-esc-changes-nothing)
  - [a composer too short for thumbnails still lists the pictures it will send](#scenario-a-composer-too-short-for-thumbnails-still-lists-the-pictures-it-will-send)
  - [a quote with a picture is sent as one embed carrying both](#scenario-a-quote-with-a-picture-is-sent-as-one-embed-carrying-both)

## bsky pictures
Source: `crates/bluesky-terminal-client/e2e/atago/pictures.atago.yaml`
### Scenario: two pictures from two folders are posted with their alt text and shape
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `pics/photo.png` is created.
- Fixture file `pics/sub/alice.png` is created.
- Fixture file `pics/notes.txt` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `pics/notes.txt`:_

```text
not a picture, and not listed
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- rendered screen shows an image width 100px, height 100px, like testdata/avatar-alice.png
- the step changed exactly created nothing, modified nothing, deleted nothing
- mock `api` received `POST /xrpc/com.atproto.repo.uploadBlob` exactly 2 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: four pictures at most, and one that cannot be read stops the post before any upload
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `pics/p1.png` is created.
- Fixture file `pics/p2.png` is created.
- Fixture file `pics/p3.png` is created.
- Fixture file `pics/p4.png` is created.
- Fixture file `pics/broken.png` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `pics/broken.png`:_

```text
only text
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `api` received `/xrpc/com.atproto.repo.uploadBlob` exactly 0 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 0 time(s)

### Scenario: the avatar is chosen in the browser, hidden files on request, esc changes nothing
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `me.png` is created.
- Fixture file `.hidden.png` is created.

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
- mock `api` received `/xrpc/com.atproto.repo.uploadBlob` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.putRecord` exactly 1 time(s)

### Scenario: a composer too short for thumbnails still lists the pictures it will send
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `pics/photo.png` is created.
- Fixture file `pics/alice.png` is created.

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
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: a quote with a picture is sent as one embed carrying both
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `pics/photo.png` is created.

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
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)
