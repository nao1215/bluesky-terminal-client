# atago Behavior Specs
## Summary
1 suite · 5 scenarios
## Contents
- [bsky threads](#bsky-threads): 5 scenarios
  - [a reply is shown under its thread, and v opens the whole thread](#scenario-a-reply-is-shown-under-its-thread-and-v-opens-the-whole-thread)
  - [a thread opens and a like is sent while slower loads are still waiting](#scenario-a-thread-opens-and-a-like-is-sent-while-slower-loads-are-still-waiting)
  - [R in a thread reloads it and keeps the reply that was selected](#scenario-r-in-a-thread-reloads-it-and-keeps-the-reply-that-was-selected)
  - [a thread that cannot be loaded says why and R tries again](#scenario-a-thread-that-cannot-be-loaded-says-why-and-r-tries-again)
  - [the thread of the post the selection rests on opens at once](#scenario-the-thread-of-the-post-the-selection-rests-on-opens-at-once)

## bsky threads
Source: `crates/bluesky-terminal-client/e2e/atago/thread.atago.yaml`
### Scenario: a reply is shown under its thread, and v opens the whole thread
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
- mock `api` received `/xrpc/app.bsky.feed.getPostThread` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.feed.getPostThread`
- mock `api` received `/xrpc/app.bsky.feed.getPostThread`
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: a thread opens and a like is sent while slower loads are still waiting
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
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: R in a thread reloads it and keeps the reply that was selected
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
- mock `api` received `/xrpc/app.bsky.feed.getPostThread` exactly 2 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: a thread that cannot be loaded says why and R tries again
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
- mock `api` received `/xrpc/app.bsky.feed.getPostThread` exactly 2 time(s)

### Scenario: the thread of the post the selection rests on opens at once
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
- mock `api` received `/xrpc/app.bsky.feed.getPostThread` exactly 1 time(s)
- mock `api` received `/img/photo.png` exactly 1 time(s)
