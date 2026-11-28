# atago Behavior Specs
## Summary
1 suite · 15 scenarios
## Contents
- [bsky search](#bsky-search): 15 scenarios
  - [searching posts sends the typed words and shows the hits with images](#scenario-searching-posts-sends-the-typed-words-and-shows-the-hits-with-images)
  - [slash opens the search box and repeats its last query](#scenario-slash-opens-the-search-box-and-repeats-its-last-query)
  - [letters typed into the search box are text, not commands](#scenario-letters-typed-into-the-search-box-are-text-not-commands)
  - [an empty query sends nothing](#scenario-an-empty-query-sends-nothing)
  - [a paste into the search box turns its line breaks into spaces and keeps emoji whole](#scenario-a-paste-into-the-search-box-turns-its-line-breaks-into-spaces-and-keeps-emoji-whole)
  - [on Windows a paste into the search box arrives as key presses, emoji whole](#scenario-on-windows-a-paste-into-the-search-box-arrives-as-key-presses-emoji-whole)
  - [account search follows and unfollows from the result list](#scenario-account-search-follows-and-unfollows-from-the-result-list)
  - [switching from posts to accounts re-runs the same query](#scenario-switching-from-posts-to-accounts-re-runs-the-same-query)
  - [no matching accounts says so](#scenario-no-matching-accounts-says-so)
  - [enter on an account opens its profile, where f follows it](#scenario-enter-on-an-account-opens-its-profile-where-f-follows-it)
  - [post results say whether each author is followed, and f follows from them](#scenario-post-results-say-whether-each-author-is-followed-and-f-follows-from-them)
  - [nearing the end of the results asks for the next page, once for posts and once for accounts](#scenario-nearing-the-end-of-the-results-asks-for-the-next-page-once-for-posts-and-once-for-accounts)
  - [search results replace the timeline and Esc restores the selected post without fetching](#scenario-search-results-replace-the-timeline-and-esc-restores-the-selected-post-without-fetching)
  - [Esc in the search box cancels without changing the timeline selection](#scenario-esc-in-the-search-box-cancels-without-changing-the-timeline-selection)
  - [search plays the selected video inline and Esc restores timeline playback](#scenario-search-plays-the-selected-video-inline-and-esc-restores-timeline-playback)

## bsky search
Source: `crates/bluesky-terminal-client/e2e/atago/search.atago.yaml`
### Scenario: searching posts sends the typed words and shows the hits with images
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
- rendered screen shows an image width 320px, height 240px, like testdata/expected/photo-320x240.png
- mock `api` received `GET /xrpc/app.bsky.feed.searchPosts` exactly 1 time(s)

### Scenario: slash opens the search box and repeats its last query
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
- mock `api` received `/xrpc/app.bsky.feed.searchPosts` exactly 2 time(s)

### Scenario: letters typed into the search box are text, not commands
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
- mock `api` received `/xrpc/app.bsky.feed.searchPosts` exactly 1 time(s)

### Scenario: an empty query sends nothing
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
- mock `api` received `/xrpc/app.bsky.feed.searchPosts` exactly 0 time(s)

### Scenario: a paste into the search box turns its line breaks into spaces and keeps emoji whole
_skipped on Windows_
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
- mock `api` received `/xrpc/app.bsky.feed.searchPosts` exactly 1 time(s)

### Scenario: on Windows a paste into the search box arrives as key presses, emoji whole
_only on Windows_
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
- mock `api` received `/xrpc/app.bsky.feed.searchPosts` exactly 1 time(s)

### Scenario: account search follows and unfollows from the result list
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
- mock `api` received `/xrpc/app.bsky.actor.searchActors` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.deleteRecord` exactly 1 time(s)

### Scenario: switching from posts to accounts re-runs the same query
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
- mock `api` received `/xrpc/app.bsky.feed.searchPosts` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.actor.searchActors` exactly 1 time(s)

### Scenario: no matching accounts says so
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

### Scenario: enter on an account opens its profile, where f follows it
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
- rendered screen shows an image width 120px, height 120px, like testdata/expected/alice-avatar-120.png
- mock `api` received `/xrpc/app.bsky.actor.getProfile` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.actor.searchActors` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.feed.getAuthorFeed` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.feed.getAuthorFeed`
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: post results say whether each author is followed, and f follows from them
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
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: nearing the end of the results asks for the next page, once for posts and once for accounts
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
- mock `api` received `/xrpc/app.bsky.feed.searchPosts` exactly 2 time(s)
- mock `api` received `/xrpc/app.bsky.actor.searchActors` exactly 2 time(s)

### Scenario: search results replace the timeline and Esc restores the selected post without fetching
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
- mock `api` received `/xrpc/app.bsky.feed.getTimeline` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: Esc in the search box cancels without changing the timeline selection
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
- mock `api` received `/xrpc/app.bsky.feed.searchPosts` exactly 0 time(s)
- mock `api` received `/xrpc/app.bsky.feed.getTimeline` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: search plays the selected video inline and Esc restores timeline playback
#### Given
- Stub HTTP server `api` serves 10 canned route(s) at `${api.url}` and records every request (#24).
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
- mock `api` received `/xrpc/app.bsky.feed.getTimeline` exactly 1 time(s)
- mock `api` received `GET /timeline/v/seg0.ts`
- mock `api` received `GET /search/v/seg0.ts`
