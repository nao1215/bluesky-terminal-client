# atago Behavior Specs
## Summary
1 suite · 51 scenarios
## Contents
- [bsky timeline](#bsky-timeline): 51 scenarios
  - [only posts by followed accounts are shown, with their avatars and photo](#scenario-only-posts-by-followed-accounts-are-shown-with-their-avatars-and-photo)
  - [liking a post records the like and updates the count](#scenario-liking-a-post-records-the-like-and-updates-the-count)
  - [the pinned feeds are offered as columns in the pinned order, and posts there can be liked](#scenario-the-pinned-feeds-are-offered-as-columns-in-the-pinned-order-and-posts-there-can-be-liked)
  - [a feed column that fails to load says why, R asks again, and Following is unharmed](#scenario-a-feed-column-that-fails-to-load-says-why-r-asks-again-and-following-is-unharmed)
  - [with no feed pinned, Discover is offered as a column](#scenario-with-no-feed-pinned-discover-is-offered-as-a-column)
  - [l on a liked post removes the like, and l again restores it](#scenario-l-on-a-liked-post-removes-the-like-and-l-again-restores-it)
  - [a like the server names outside the account's likes is not deleted](#scenario-a-like-the-server-names-outside-the-accounts-likes-is-not-deleted)
  - [a reload that answers after a like does not undo it](#scenario-a-reload-that-answers-after-a-like-does-not-undo-it)
  - [b reposts a post, and b again removes the repost](#scenario-b-reposts-a-post-and-b-again-removes-the-repost)
  - [nearing the end fetches the next page once, and a repeated page ends the list](#scenario-nearing-the-end-fetches-the-next-page-once-and-a-repeated-page-ends-the-list)
  - [nearing the end of a feed column asks for the next page once](#scenario-nearing-the-end-of-a-feed-column-asks-for-the-next-page-once)
  - [tab moves to the next tab and shift+tab to the previous one, the search box included](#scenario-tab-moves-to-the-next-tab-and-shifttab-to-the-previous-one-the-search-box-included)
  - [a reply to a reply keeps the thread root and names the parent](#scenario-a-reply-to-a-reply-keeps-the-thread-root-and-names-the-parent)
  - [a post with a damaged reply field shows its text and can be answered](#scenario-a-post-with-a-damaged-reply-field-shows-its-text-and-can-be-answered)
  - [a post with odd fields neither fails its page nor goes missing](#scenario-a-post-with-odd-fields-neither-fails-its-page-nor-goes-missing)
  - [tabs and a long wide name are drawn as they were laid out](#scenario-tabs-and-a-long-wide-name-are-drawn-as-they-were-laid-out)
  - [the text after a VS16 emoji stays in place across tab changes](#scenario-the-text-after-a-vs16-emoji-stays-in-place-across-tab-changes)
  - [a new post carries link, tag and mention facets at UTF-8 byte offsets](#scenario-a-new-post-carries-link-tag-and-mention-facets-at-utf-8-byte-offsets)
  - [a new post from the composer carries a card for its first link](#scenario-a-new-post-from-the-composer-carries-a-card-for-its-first-link)
  - [a mention in a parenthesis after a link is its own facet](#scenario-a-mention-in-a-parenthesis-after-a-link-is-its-own-facet)
  - [a tag ends before full-width punctuation, and digits with punctuation are no tag](#scenario-a-tag-ends-before-full-width-punctuation-and-digits-with-punctuation-are-no-tag)
  - [a tag glued to a mention sends both, the tag ending where the mention begins](#scenario-a-tag-glued-to-a-mention-sends-both-the-tag-ending-where-the-mention-begins)
  - [a bare domain links with https in front, and a file name stays text](#scenario-a-bare-domain-links-with-https-in-front-and-a-file-name-stays-text)
  - [emoji are edited as whole characters and the link after them keeps its byte range](#scenario-emoji-are-edited-as-whole-characters-and-the-link-after-them-keeps-its-byte-range)
  - [a paste into the composer keeps its lines and emoji and links what it holds](#scenario-a-paste-into-the-composer-keeps-its-lines-and-emoji-and-links-what-it-holds)
  - [G and g jump to the last and first post, and l likes the one selected](#scenario-g-and-g-jump-to-the-last-and-first-post-and-l-likes-the-one-selected)
  - [a resized terminal is redrawn at its new size and keeps the selection](#scenario-a-resized-terminal-is-redrawn-at-its-new-size-and-keeps-the-selection)
  - [an empty post is refused before anything is sent](#scenario-an-empty-post-is-refused-before-anything-is-sent)
  - [300 characters is the limit, 301 is refused](#scenario-300-characters-is-the-limit-301-is-refused)
  - [a post of emoji over 3000 bytes is refused before anything is sent](#scenario-a-post-of-emoji-over-3000-bytes-is-refused-before-anything-is-sent)
  - [a post the server rejects keeps the draft and shows the reason](#scenario-a-post-the-server-rejects-keeps-the-draft-and-shows-the-reason)
  - [unfollowing an author removes their posts from the timeline](#scenario-unfollowing-an-author-removes-their-posts-from-the-timeline)
  - [a like, a repost and a delete the server fails with 502 are each sent once](#scenario-a-like-a-repost-and-a-delete-the-server-fails-with-502-are-each-sent-once)
  - [a failing timeline is reported and the client keeps running](#scenario-a-failing-timeline-is-reported-and-the-client-keeps-running)
  - [an expired access token is refreshed once and the new tokens are saved](#scenario-an-expired-access-token-is-refreshed-once-and-the-new-tokens-are-saved)
  - [an access token past the expiry it carries is refreshed before the first load](#scenario-an-access-token-past-the-expiry-it-carries-is-refreshed-before-the-first-load)
  - [loads that find the token expired together refresh it once](#scenario-loads-that-find-the-token-expired-together-refresh-it-once)
  - [a refresh token the server refuses brings back the login form](#scenario-a-refresh-token-the-server-refuses-brings-back-the-login-form)
  - [a draft is still there after the session expires while it is sent and the login form comes back](#scenario-a-draft-is-still-there-after-the-session-expires-while-it-is-sent-and-the-login-form-comes-back)
  - [a delete asked before the session expired is not made by the first y after logging in](#scenario-a-delete-asked-before-the-session-expired-is-not-made-by-the-first-y-after-logging-in)
  - [the help lists the keys by section, scrolls, and closes only on purpose](#scenario-the-help-lists-the-keys-by-section-scrolls-and-closes-only-on-purpose)
  - [pictures are kept on disk and the next start draws them without downloading](#scenario-pictures-are-kept-on-disk-and-the-next-start-draws-them-without-downloading)
  - [BSKY_CACHE_DIR=off keeps no pictures on disk](#scenario-bsky_cache_diroff-keeps-no-pictures-on-disk)
  - [a quote of a feed, a list, or a starter pack says which it is](#scenario-a-quote-of-a-feed-a-list-or-a-starter-pack-says-which-it-is)
  - [quotes name who is quoted, and o always opens something](#scenario-quotes-name-who-is-quoted-and-o-always-opens-something)
  - [D deletes your own post after a y, and any other key keeps it](#scenario-d-deletes-your-own-post-after-a-y-and-any-other-key-keeps-it)
  - [c puts the post's address in the terminal's clipboard](#scenario-c-puts-the-posts-address-in-the-terminals-clipboard)
  - [Q sends a post that quotes the selected one](#scenario-q-sends-a-post-that-quotes-the-selected-one)
  - [M mutes the selected post's author and B blocks after a y, and their posts leave the timeline](#scenario-m-mutes-the-selected-posts-author-and-b-blocks-after-a-y-and-their-posts-leave-the-timeline)
  - [a post deleted elsewhere above the selection leaves the selection on its post](#scenario-a-post-deleted-elsewhere-above-the-selection-leaves-the-selection-on-its-post)
  - [a post shown above its reply is not shown a second time](#scenario-a-post-shown-above-its-reply-is-not-shown-a-second-time)

## bsky timeline
Source: `crates/bluesky-terminal-client/e2e/atago/timeline.atago.yaml`
### Scenario: only posts by followed accounts are shown, with their avatars and photo
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
- rendered screen shows exactly 3 image(s) and an image width 40px, height 40px, like testdata/expected/alice-avatar-40.png and an image width 40px, height 40px, like testdata/expected/bob-avatar-40.png and an image width 320px, height 240px, like testdata/expected/photo-320x240.png
- mock `api` received `GET /xrpc/app.bsky.feed.getTimeline` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.feed.getTimeline`
- mock `api` received `/img/photo.png` exactly 1 time(s)
- mock `api` received `/img/alice.png` exactly 1 time(s)
- mock `api` received a request exactly 10 time(s)
- mock `api` received `HEAD /xrpc/_health` exactly 4 time(s)
- mock `api` received `/xrpc/app.bsky.feed.getPostThread` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.notification.listNotifications` exactly 1 time(s)

### Scenario: liking a post records the like and updates the count
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
- mock `api` received `POST /xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: the pinned feeds are offered as columns in the pinned order, and posts there can be liked
#### Given
- Stub HTTP server `api` serves 8 canned route(s) at `${api.url}` and records every request (#24).
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
- mock `api` received `/xrpc/app.bsky.feed.getFeed` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.feed.getFeedGenerators` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: a feed column that fails to load says why, R asks again, and Following is unharmed
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
- mock `api` received `/xrpc/app.bsky.feed.getFeed` exactly 2 time(s)
- mock `api` received `/xrpc/app.bsky.feed.getTimeline` exactly 2 time(s)

### Scenario: with no feed pinned, Discover is offered as a column
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
- mock `api` received `/xrpc/app.bsky.feed.getFeed` exactly 1 time(s)

### Scenario: l on a liked post removes the like, and l again restores it
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
- mock `api` received `/xrpc/com.atproto.repo.deleteRecord` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: a like the server names outside the account's likes is not deleted
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
- mock `api` received `/xrpc/com.atproto.repo.deleteRecord` exactly 0 time(s)

### Scenario: a reload that answers after a like does not undo it
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
- mock `api` received `/xrpc/app.bsky.feed.getTimeline` exactly 2 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.deleteRecord` exactly 1 time(s)

### Scenario: b reposts a post, and b again removes the repost
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
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.deleteRecord` exactly 1 time(s)

### Scenario: nearing the end fetches the next page once, and a repeated page ends the list
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
- mock `api` received `/xrpc/app.bsky.feed.getTimeline` exactly 2 time(s)

### Scenario: nearing the end of a feed column asks for the next page once
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
- mock `api` received `/xrpc/app.bsky.feed.getFeed` exactly 2 time(s)
- mock `api` received `/xrpc/app.bsky.feed.getTimeline` exactly 2 time(s)

### Scenario: tab moves to the next tab and shift+tab to the previous one, the search box included
#### Given
- Stub HTTP server `api` serves 8 canned route(s) at `${api.url}` and records every request (#24).
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

### Scenario: a reply to a reply keeps the thread root and names the parent
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

### Scenario: a post with a damaged reply field shows its text and can be answered
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
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: a post with odd fields neither fails its page nor goes missing
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

### Scenario: tabs and a long wide name are drawn as they were laid out
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
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

### Scenario: the text after a VS16 emoji stays in place across tab changes
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

### Scenario: a new post carries link, tag and mention facets at UTF-8 byte offsets
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
- mock `api` received `/xrpc/com.atproto.identity.resolveHandle` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: a new post from the composer carries a card for its first link
#### Given
- Stub HTTP server `api` serves 8 canned route(s) at `${api.url}` and records every request (#24).
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

### Scenario: a mention in a parenthesis after a link is its own facet
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
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: a tag ends before full-width punctuation, and digits with punctuation are no tag
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

### Scenario: a tag glued to a mention sends both, the tag ending where the mention begins
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
- mock `api` received `/xrpc/com.atproto.identity.resolveHandle` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: a bare domain links with https in front, and a file name stays text
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

### Scenario: emoji are edited as whole characters and the link after them keeps its byte range
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

### Scenario: a paste into the composer keeps its lines and emoji and links what it holds
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

### Scenario: G and g jump to the last and first post, and l likes the one selected
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
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 2 time(s)

### Scenario: a resized terminal is redrawn at its new size and keeps the selection
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

### Scenario: an empty post is refused before anything is sent
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
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 0 time(s)

### Scenario: 300 characters is the limit, 301 is refused
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

### Scenario: a post of emoji over 3000 bytes is refused before anything is sent
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

### Scenario: a post the server rejects keeps the draft and shows the reason
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

### Scenario: unfollowing an author removes their posts from the timeline
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

### Scenario: a like, a repost and a delete the server fails with 502 are each sent once
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
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 2 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord`
- mock `api` received `/xrpc/com.atproto.repo.deleteRecord` exactly 1 time(s)

### Scenario: a failing timeline is reported and the client keeps running
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
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
- mock `api` received `/xrpc/app.bsky.feed.getTimeline` exactly 4 time(s)

### Scenario: an expired access token is refreshed once and the new tokens are saved
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
- mock `api` received `/xrpc/com.atproto.server.refreshSession` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.feed.getTimeline` exactly 2 time(s)
- file `cfg/accounts/did_plc_me.json` at `$.accessJwt` equals `acc-2`; at `$.refreshJwt` equals `ref-2`; at `$.service` equals `${api.url}`

### Scenario: an access token past the expiry it carries is refreshed before the first load
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "eyJhbGciOiJFUzI1NksiLCJ0eXAiOiJhdCtqd3QifQ.eyJleHAiOjE3MDAwMDAwMDB9.sig", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `api` received `/xrpc/com.atproto.server.refreshSession` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.feed.getTimeline` exactly 1 time(s)
- file `cfg/accounts/did_plc_me.json` at `$.accessJwt` equals `acc-2`; at `$.refreshJwt` equals `ref-2`
- mock `api` received `HEAD /xrpc/_health` exactly 4 time(s)

### Scenario: loads that find the token expired together refresh it once
#### Given
- Background service `api` is started: `python3 "${specdir}/testdata/refresh_server.py"`.

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- file `refresh.log` equals exact bytes
- file `timeline.log` equals exact bytes
- file `notifications.log` equals exact bytes
- file `cfg/accounts/did_plc_me.json` at `$.refreshJwt` equals `ref-2`

### Scenario: a refresh token the server refuses brings back the login form
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
- mock `api` received `/xrpc/com.atproto.server.createSession` exactly 1 time(s)
- file `cfg/accounts/did_plc_me.json` at `$.refreshJwt` equals `ref-new`

### Scenario: a draft is still there after the session expires while it is sent and the login form comes back
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
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: a delete asked before the session expired is not made by the first y after logging in
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
- mock `api` received `/xrpc/com.atproto.repo.deleteRecord` exactly 0 time(s)

### Scenario: the help lists the keys by section, scrolls, and closes only on purpose
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
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

### Scenario: pictures are kept on disk and the next start draws them without downloading
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
  - dir `cache/images` has 3 entries
- after `interactive (pty): bsky`:
  - exit code is `0`
  - rendered screen shows exactly 3 image(s) and an image width 320px, height 240px, like testdata/expected/photo-320x240.png
  - mock `api` received `/img/photo.png` exactly 1 time(s)
  - mock `api` received `/img/alice.png` exactly 1 time(s)
  - mock `api` received `/img/bob.png` exactly 1 time(s)

### Scenario: BSKY_CACHE_DIR=off keeps no pictures on disk
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
- the step changed exactly created nothing, modified nothing, deleted nothing

### Scenario: a quote of a feed, a list, or a starter pack says which it is
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
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

### Scenario: quotes name who is quoted, and o always opens something
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

### Scenario: D deletes your own post after a y, and any other key keeps it
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
- mock `api` received `POST /xrpc/com.atproto.repo.deleteRecord` exactly 1 time(s)

### Scenario: c puts the post's address in the terminal's clipboard
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
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
- stdout contains `]52;c;aHR0cHM6Ly9ic2t5LmFwcC9wcm9maWxlL2RpZDpwbGM6Ym9iL3Bvc3QvdGhlaXJz`

### Scenario: Q sends a post that quotes the selected one
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
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: M mutes the selected post's author and B blocks after a y, and their posts leave the timeline
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
- mock `api` received `/xrpc/app.bsky.graph.muteActor` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: a post deleted elsewhere above the selection leaves the selection on its post
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

### Scenario: a post shown above its reply is not shown a second time
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
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
