# atago Behavior Specs
## Summary
1 suite · 7 scenarios
## Contents
- [bsky chat](#bsky-chat): 7 scenarios
  - [the Chat tab lists conversations, marks one read only when it is opened, and sends a message once](#scenario-the-chat-tab-lists-conversations-marks-one-read-only-when-it-is-opened-and-sends-a-message-once)
  - [listing conversations marks nothing read](#scenario-listing-conversations-marks-nothing-read)
  - [an app password without access to messages is told how to get one that has it](#scenario-an-app-password-without-access-to-messages-is-told-how-to-get-one-that-has-it)
  - [m on a profile opens the conversation with them](#scenario-m-on-a-profile-opens-the-conversation-with-them)
  - [bsky chat lists, reads and sends, as text and as JSON](#scenario-bsky-chat-lists-reads-and-sends-as-text-and-as-json)
  - [the chat calls go to the account's PDS, not the entryway logged in through](#scenario-the-chat-calls-go-to-the-accounts-pds-not-the-entryway-logged-in-through)
  - [bsky chat -n 0 is a usage error, and more than a page follows the cursor](#scenario-bsky-chat--n-0-is-a-usage-error-and-more-than-a-page-follows-the-cursor)

## bsky chat
Source: `crates/bluesky-terminal-client/e2e/atago/chat.atago.yaml`
### Scenario: the Chat tab lists conversations, marks one read only when it is opened, and sends a message once
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
- mock `api` received `/xrpc/chat.bsky.convo.listConvos`
- mock `api` received `/xrpc/chat.bsky.convo.getMessages`
- mock `api` received `/xrpc/chat.bsky.convo.updateRead` exactly 1 time(s)
- mock `api` received `/xrpc/chat.bsky.convo.sendMessage` exactly 1 time(s)

### Scenario: listing conversations marks nothing read
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
- mock `api` received `/xrpc/chat.bsky.convo.updateRead` exactly 0 time(s)

### Scenario: an app password without access to messages is told how to get one that has it
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
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

### Scenario: m on a profile opens the conversation with them
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
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
- mock `api` received `/xrpc/chat.bsky.convo.getConvoForMembers` exactly 1 time(s)
- mock `api` received `/xrpc/chat.bsky.convo.getMessages`

### Scenario: bsky chat lists, reads and sends, as text and as JSON
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CACHE_DIR, BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CACHE_DIR, BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CACHE_DIR, BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky chat
bsky chat alice.test
bsky chat @alice.test "from a script" --json
```

#### Then
- after `bsky chat`:
  - exit code is `0`
  - stdout equals an exact value
- after `bsky chat alice.test`:
  - exit code is `0`
  - stdout equals an exact value
- after `bsky chat @alice.test "from a script" --json`:
  - exit code is `0`
  - stdout at `$.id` equals `m3`; at `$.convoId` equals `convo-a`
  - mock `api` received `/xrpc/chat.bsky.convo.sendMessage` exactly 1 time(s)

#### Expected output
_expected stdout:_

```text
家族👨‍👩‍👧 Alice @alice.test  (2 new)
  See you 🇯🇵
```

_expected stdout:_

```text
Alice @alice.test · 2026-09-21 10:00
  Hello there
Alice @alice.test · 2026-09-21 10:05
  See you 🇯🇵
```

### Scenario: the chat calls go to the account's PDS, not the entryway logged in through
#### Given
- Stub HTTP server `entry` serves 2 canned route(s) at `${entry.url}` and records every request (#24).
- Stub HTTP server `pds` serves 1 canned route(s) at `${pds.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CACHE_DIR, BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${entry.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky chat
```

#### Then
- exit code is `0`
- stdout contains `家族👨‍👩‍👧 Alice @alice.test  (2 new)`
- mock `pds` received `/xrpc/chat.bsky.convo.listConvos` exactly 1 time(s)
- mock `entry` received `/xrpc/chat.bsky.convo.listConvos` exactly 0 time(s)

### Scenario: bsky chat -n 0 is a usage error, and more than a page follows the cursor
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CACHE_DIR, BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CACHE_DIR, BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky chat -n 0
bsky chat -n 150
```

#### Then
- after `bsky chat -n 0`:
  - exit code is `1`
  - stdout equals an exact value
  - stderr equals an exact value
  - mock `api` received `/xrpc/chat.bsky.convo.listConvos` exactly 0 time(s)
- after `bsky chat -n 150`:
  - exit code is `0`
  - mock `api` received `/xrpc/chat.bsky.convo.listConvos` exactly 2 time(s)
  - mock `api` received `/xrpc/chat.bsky.convo.listConvos`

#### Expected output
_expected stderr:_

```text
error: invalid value '0' for '--limit <LIMIT>': it is how many to list, so 1 or more
hint: run `bsky --help` for usage
```

