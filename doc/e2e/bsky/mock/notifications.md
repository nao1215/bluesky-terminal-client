# atago Behavior Specs
## Summary
1 suite · 5 scenarios
## Contents
- [bsky notifications](#bsky-notifications): 5 scenarios
  - [notifications show who did what and are marked seen](#scenario-notifications-show-who-did-what-and-are-marked-seen)
  - [notifications load at the start, but are seen only when their tab is](#scenario-notifications-load-at-the-start-but-are-seen-only-when-their-tab-is)
  - [a reply notification can be answered, and a follow opens the profile](#scenario-a-reply-notification-can-be-answered-and-a-follow-opens-the-profile)
  - [failing notifications are reported and R tries again](#scenario-failing-notifications-are-reported-and-r-tries-again)
  - [nearing the end of the notifications asks for the next page once](#scenario-nearing-the-end-of-the-notifications-asks-for-the-next-page-once)

## bsky notifications
Source: `crates/bluesky-terminal-client/e2e/atago/notifications.atago.yaml`
### Scenario: notifications show who did what and are marked seen
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
- mock `api` received `/xrpc/app.bsky.notification.listNotifications` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.feed.getPosts` exactly 1 time(s)
- mock `api` received `POST /xrpc/app.bsky.notification.updateSeen` exactly 1 time(s)

### Scenario: notifications load at the start, but are seen only when their tab is
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
- mock `api` received `/xrpc/app.bsky.notification.listNotifications` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.notification.updateSeen` exactly 0 time(s)

### Scenario: a reply notification can be answered, and a follow opens the profile
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
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
- mock `api` received `/xrpc/app.bsky.actor.getProfile` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.notification.listNotifications` exactly 1 time(s)

### Scenario: failing notifications are reported and R tries again
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
- mock `api` received `/xrpc/app.bsky.notification.listNotifications` exactly 2 time(s)
- mock `api` received `/xrpc/app.bsky.notification.updateSeen` exactly 0 time(s)

### Scenario: nearing the end of the notifications asks for the next page once
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
- mock `api` received `/xrpc/app.bsky.notification.listNotifications` exactly 2 time(s)
