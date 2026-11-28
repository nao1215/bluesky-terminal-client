# atago Behavior Specs
## Summary
1 suite · 1 scenario
## Contents
- [bsky columns](#bsky-columns): 1 scenario
  - [columns are added beside the timeline, a like in one shows everywhere, and they are back on the next start](#scenario-columns-are-added-beside-the-timeline-a-like-in-one-shows-everywhere-and-they-are-back-on-the-next-start)

## bsky columns
Source: `crates/bluesky-terminal-client/e2e/atago/columns.atago.yaml`
### Scenario: columns are added beside the timeline, a like in one shows everywhere, and they are back on the next start
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
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)
  - mock `api` received `/xrpc/app.bsky.feed.searchPosts`
  - file `cfg/settings.json` at `$.columns['did:plc:me'][0].kind` equals `following`; at `$.columns['did:plc:me'][1].kind` equals `notifications`; at `$.columns['did:plc:me'][2].kind` equals `search`; at `$.columns['did:plc:me'][2].query` equals `猫🐈‍⬛`
- after `interactive (pty): bsky`:
  - exit code is `0`
  - file `cfg/settings.json` does not contain `did:plc:me`
