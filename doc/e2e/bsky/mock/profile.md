# atago Behavior Specs
## Summary
1 suite · 13 scenarios
## Contents
- [bsky profile](#bsky-profile): 13 scenarios
  - [the Profile tab shows the account itself](#scenario-the-profile-tab-shows-the-account-itself)
  - [saving keeps the fields bsky does not edit and guards the write with swapRecord](#scenario-saving-keeps-the-fields-bsky-does-not-edit-and-guards-the-write-with-swaprecord)
  - [a cleared description is removed from the record, not saved empty](#scenario-a-cleared-description-is-removed-from-the-record-not-saved-empty)
  - [a new avatar is uploaded first and the record points at the uploaded blob](#scenario-a-new-avatar-is-uploaded-first-and-the-record-points-at-the-uploaded-blob)
  - [a file that is not a picture is refused as an avatar before anything is uploaded](#scenario-a-file-that-is-not-a-picture-is-refused-as-an-avatar-before-anything-is-uploaded)
  - [in Japanese, a file that is not a picture is refused in Japanese](#scenario-in-japanese-a-file-that-is-not-a-picture-is-refused-in-japanese)
  - [an account that never saved a profile gets a new record, swapped against none](#scenario-an-account-that-never-saved-a-profile-gets-a-new-record-swapped-against-none)
  - [a save that lost a race with another client says so and keeps the edit](#scenario-a-save-that-lost-a-race-with-another-client-says-so-and-keeps-the-edit)
  - [someone else's profile can be followed but not edited, and Esc goes back to the timeline](#scenario-someone-elses-profile-can-be-followed-but-not-edited-and-esc-goes-back-to-the-timeline)
  - [nearing the end of a profile's posts asks for the next page once](#scenario-nearing-the-end-of-a-profiles-posts-asks-for-the-next-page-once)
  - [pictures turned off on the settings screen stay off, and auto brings them back](#scenario-pictures-turned-off-on-the-settings-screen-stay-off-and-auto-brings-them-back)
  - [a setting a variable fixes is shown as set for this run and is not changed](#scenario-a-setting-a-variable-fixes-is-shown-as-set-for-this-run-and-is-not-changed)
  - [the language chosen on the settings screen is used at once and on the next start](#scenario-the-language-chosen-on-the-settings-screen-is-used-at-once-and-on-the-next-start)

## bsky profile
Source: `crates/bluesky-terminal-client/e2e/atago/profile.atago.yaml`
### Scenario: the Profile tab shows the account itself
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

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
- mock `api` received `/xrpc/app.bsky.actor.getProfile` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 0 time(s)

### Scenario: saving keeps the fields bsky does not edit and guards the write with swapRecord
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

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
- mock `api` received `/xrpc/com.atproto.repo.getRecord` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.getRecord`
- mock `api` received `POST /xrpc/com.atproto.repo.putRecord` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.uploadBlob` exactly 0 time(s)
- mock `api` received `/xrpc/app.bsky.actor.getProfile` exactly 2 time(s)

### Scenario: a cleared description is removed from the record, not saved empty
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

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
- mock `api` received `/xrpc/com.atproto.repo.putRecord` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.putRecord`

### Scenario: a new avatar is uploaded first and the record points at the uploaded blob
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `me.png` is created.

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
- mock `api` received `POST /xrpc/com.atproto.repo.uploadBlob` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.putRecord` exactly 1 time(s)
- file `me.png` exists

#### Generated artifacts
- `me.png`

### Scenario: a file that is not a picture is refused as an avatar before anything is uploaded
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `notes.txt` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `notes.txt`:_

```text
not an image
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `api` received `/xrpc/com.atproto.repo.uploadBlob` exactly 0 time(s)
- mock `api` received `/xrpc/com.atproto.repo.putRecord` exactly 0 time(s)

### Scenario: in Japanese, a file that is not a picture is refused in Japanese
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/settings.json` is created.
- Fixture file `notes.txt` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/settings.json`:_

```text
{"language": "ja"}
```

_Fixture `notes.txt`:_

```text
not an image
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `api` received `/xrpc/com.atproto.repo.uploadBlob` exactly 0 time(s)

### Scenario: an account that never saved a profile gets a new record, swapped against none
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

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
- mock `api` received `/xrpc/com.atproto.repo.putRecord` exactly 1 time(s)

### Scenario: a save that lost a race with another client says so and keeps the edit
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

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

### Scenario: someone else's profile can be followed but not edited, and Esc goes back to the timeline
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

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
- mock `api` received `/xrpc/com.atproto.repo.deleteRecord` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.getRecord` exactly 0 time(s)
- mock `api` received `/xrpc/app.bsky.actor.getProfile` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.feed.getTimeline` exactly 1 time(s)

### Scenario: nearing the end of a profile's posts asks for the next page once
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

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
- mock `api` received `/xrpc/app.bsky.feed.getAuthorFeed` exactly 2 time(s)

### Scenario: pictures turned off on the settings screen stay off, and auto brings them back
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
# interactive (pty): bsky
```

#### Then
- after `interactive (pty): bsky`:
  - exit code is `0`
  - file `cfg/settings.json` at `$.pictures` equals `off`
- after `interactive (pty): bsky`:
  - exit code is `0`
  - file `cfg/settings.json` at `$.pictures` equals `auto`

### Scenario: a setting a variable fixes is shown as set for this run and is not changed
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

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
- file `cfg/settings.json` does not exist

### Scenario: the language chosen on the settings screen is used at once and on the next start
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
# interactive (pty): bsky
```

#### Then
- after `interactive (pty): bsky`:
  - exit code is `0`
  - file `cfg/settings.json` at `$.language` equals `ja`
- after `interactive (pty): bsky`:
  - exit code is `0`
