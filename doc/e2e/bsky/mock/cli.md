# atago Behavior Specs
## Summary
1 suite · 42 scenarios
## Contents
- [bsky commands](#bsky-commands): 42 scenarios
  - [timeline prints the posts the client shows, as text and as JSON lines](#scenario-timeline-prints-the-posts-the-client-shows-as-text-and-as-json-lines)
  - [timeline leaves out the control characters others wrote, and --json keeps them escaped](#scenario-timeline-leaves-out-the-control-characters-others-wrote-and---json-keeps-them-escaped)
  - [post sends the record the composer would, and prints its URI or the server's answer as JSON](#scenario-post-sends-the-record-the-composer-would-and-prints-its-uri-or-the-servers-answer-as-json)
  - [post makes a card from the page of its first link, for a reply too, but not beside pictures or a quote](#scenario-post-makes-a-card-from-the-page-of-its-first-link-for-a-reply-too-but-not-beside-pictures-or-a-quote)
  - [a card links where the post does, and reads a relative picture against the page a redirect ended on](#scenario-a-card-links-where-the-post-does-and-reads-a-relative-picture-against-the-page-a-redirect-ended-on)
  - [a card is made for a link whose address is written in Japanese, its page asked for percent-encoded](#scenario-a-card-is-made-for-a-link-whose-address-is-written-in-japanese-its-page-asked-for-percent-encoded)
  - [post --no-card, Link cards off in the settings, and BSKY_LINK_CARDS=off do not read the page](#scenario-post---no-card-link-cards-off-in-the-settings-and-bsky_link_cardsoff-do-not-read-the-page)
  - [a page that gives no card still lets the post go out, and a picture that cannot be had leaves the card without one](#scenario-a-page-that-gives-no-card-still-lets-the-post-go-out-and-a-picture-that-cannot-be-had-leaves-the-card-without-one)
  - [a card whose picture the server will not take is sent without it](#scenario-a-card-whose-picture-the-server-will-not-take-is-sent-without-it)
  - [a post too long, or with pictures and a video, is refused before anything is sent](#scenario-a-post-too-long-or-with-pictures-and-a-video-is-refused-before-anything-is-sent)
  - [like and unlike take a bsky.app address, and a write the server fails is sent once](#scenario-like-and-unlike-take-a-bskyapp-address-and-a-write-the-server-fails-is-sent-once)
  - [delete takes only your own post](#scenario-delete-takes-only-your-own-post)
  - [delete takes your own post by an at:// address written with your handle](#scenario-delete-takes-your-own-post-by-an-at-address-written-with-your-handle)
  - [notifications are listed, and marked seen only with --seen](#scenario-notifications-are-listed-and-marked-seen-only-with---seen)
  - [profile, followers and a search for accounts print accounts as lines](#scenario-profile-followers-and-a-search-for-accounts-print-accounts-as-lines)
  - [thread shows the posts above a reply and its replies indented](#scenario-thread-shows-the-posts-above-a-reply-and-its-replies-indented)
  - [login reads the password from stdin, takes a handle written with @, adds the account, and accounts lists it](#scenario-login-reads-the-password-from-stdin-takes-a-handle-written-with--adds-the-account-and-accounts-lists-it)
  - [feed takes a pinned feed by name, and an unknown name lists the pinned ones](#scenario-feed-takes-a-pinned-feed-by-name-and-an-unknown-name-lists-the-pinned-ones)
  - [without an account a command is a usage error that says how to log in](#scenario-without-an-account-a-command-is-a-usage-error-that-says-how-to-log-in)
  - [an account named by BSKY_ACCOUNT can be logged in, and accounts lists them all the same](#scenario-an-account-named-by-bsky_account-can-be-logged-in-and-accounts-lists-them-all-the-same)
  - [with --json a usage error and logout print JSON too](#scenario-with---json-a-usage-error-and-logout-print-json-too)
  - [timeline -n reads further pages until it has that many posts](#scenario-timeline--n-reads-further-pages-until-it-has-that-many-posts)
  - [mute and block send once what the client sends, and mutes and blocks list the accounts](#scenario-mute-and-block-send-once-what-the-client-sends-and-mutes-and-blocks-list-the-accounts)
  - [unmute and unblock undo a mute and the block record, and yourself is refused](#scenario-unmute-and-unblock-undo-a-mute-and-the-block-record-and-yourself-is-refused)
  - [follow sends the follow record once, and follows lists the accounts an account follows](#scenario-follow-sends-the-follow-record-once-and-follows-lists-the-accounts-an-account-follows)
  - [unfollow deletes the follow record, and follow of an account already followed sends nothing](#scenario-unfollow-deletes-the-follow-record-and-follow-of-an-account-already-followed-sends-nothing)
  - [repost sends the repost record once, and unrepost of a post not reposted is a usage error](#scenario-repost-sends-the-repost-record-once-and-unrepost-of-a-post-not-reposted-is-a-usage-error)
  - [unrepost deletes the repost record, and repost of a post already reposted sends nothing](#scenario-unrepost-deletes-the-repost-record-and-repost-of-a-post-already-reposted-sends-nothing)
  - [likes and reposts list who liked and reposted a post named by its bsky.app address](#scenario-likes-and-reposts-list-who-liked-and-reposted-a-post-named-by-its-bskyapp-address)
  - [lists prints the lists an account made, and list prints a list's members](#scenario-lists-prints-the-lists-an-account-made-and-list-prints-a-lists-members)
  - [report sends a post or an account to the moderators once, and an unknown reason is a usage error](#scenario-report-sends-a-post-or-an-account-to-the-moderators-once-and-an-unknown-reason-is-a-usage-error)
  - [app-passwords lists, adds and revokes, and an app-password session is told why it cannot](#scenario-app-passwords-lists-adds-and-revokes-and-an-app-password-session-is-told-why-it-cannot)
  - [a pipe closed early ends the output quietly with exit 0](#scenario-a-pipe-closed-early-ends-the-output-quietly-with-exit-0)
  - [help and logout that cannot write their output exit 3](#scenario-help-and-logout-that-cannot-write-their-output-exit-3)
  - [unmute of an account muted by a list says which list, and sends nothing](#scenario-unmute-of-an-account-muted-by-a-list-says-which-list-and-sends-nothing)
  - [author prints an account's posts page by page, reposts only with --reposts, and sends the filter](#scenario-author-prints-an-accounts-posts-page-by-page-reposts-only-with---reposts-and-sends-the-filter)
  - [download saves the four pictures of a post, and a file already there is not overwritten](#scenario-download-saves-the-four-pictures-of-a-post-and-a-file-already-there-is-not-overwritten)
  - [download saves a post's video as one .ts file in the download folder](#scenario-download-saves-a-posts-video-as-one-ts-file-in-the-download-folder)
  - [download of a post without pictures or video is a usage error and writes nothing](#scenario-download-of-a-post-without-pictures-or-video-is-a-usage-error-and-writes-nothing)
  - [author --download saves the pictures and videos of the posts it read](#scenario-author---download-saves-the-pictures-and-videos-of-the-posts-it-read)
  - [post --dry-run shows the post and its card and sends nothing](#scenario-post---dry-run-shows-the-post-and-its-card-and-sends-nothing)
  - [whoami prints the account in use](#scenario-whoami-prints-the-account-in-use)

## bsky commands
Source: `crates/bluesky-terminal-client/e2e/atago/cli.atago.yaml`

Secrets declared: `app-pass-cli`.
### Scenario: timeline prints the posts the client shows, as text and as JSON lines
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky tl -n 5
bsky timeline --json -n 1
bsky timeline --raw -n 1
```

#### Then
- after `bsky tl -n 5`:
  - exit code is `0`
  - stdout equals an exact value
  - stderr is empty
- after `bsky timeline --json -n 1`:
  - exit code is `0`
  - stdout at `$.id` equals `at://did:plc:alice/app.bsky.feed.post/p1`; at `$.url` equals `https://bsky.app/profile/did:plc:alice/post/p1`; at `$.user.id` equals `did:plc:alice`; at `$.repostOf` equals `null`
- after `bsky timeline --raw -n 1`:
  - exit code is `0`
  - stdout matches `/^\{"post":\{.*"uri":"at://did:plc:alice/app.bsky.feed.post/p1".*\}\n$/`
  - mock `api` received `/xrpc/app.bsky.feed.getTimeline`

#### Expected output
_expected stdout:_

```text
家族👨‍👩‍👧 Alice @alice.test · 2026-09-20 10:00
Photo from the mountains 🇯🇵
second line
[1 picture]
♡ 2  ⟳ 1  ↩ 0
at://did:plc:alice/app.bsky.feed.post/p1

@me.test · 2026-09-20 07:00
My own post
♡ 0  ⟳ 0  ↩ 0
at://did:plc:me/app.bsky.feed.post/p4
```

### Scenario: timeline leaves out the control characters others wrote, and --json keeps them escaped
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky tl
bsky tl --json
```

#### Then
- after `bsky tl`:
  - exit code is `0`
  - stdout equals an exact value
- after `bsky tl --json`:
  - exit code is `0`
  - stdout contains `hi\u001b]52;c;cHduZWQ=\u0007\u001b[2J`

#### Expected output
_expected stdout:_

```text
Bob]0;title Eve 👨‍👩‍👧 @alice.test · 2026-09-20 10:00
hi]52;c;cHduZWQ=[2J 🇯🇵
♡ 0  ⟳ 0  ↩ 0
at://did:plc:alice/app.bsky.feed.post/p1
```

### Scenario: post sends the record the composer would, and prints its URI or the server's answer as JSON
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, LC_ALL, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_stdin for `bsky`:_

```text
from a pipe 🇯🇵
```

#### When

```shell
bsky post --no-card "日本語👍🏽 #タグ https://example.com @bob.test"
bsky post - --json
```

#### Then
- after `bsky post --no-card "日本語👍🏽 #タグ https://example.com @bob.test"`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)
- after `bsky post - --json`:
  - exit code is `0`
  - stdout at `$.action` equals `post`; at `$.id` equals `at://did:plc:me/app.bsky.feed.post/new1`; at `$.url` equals `https://bsky.app/profile/did:plc:me/post/new1`; at `$.cid` equals `cid-new1`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 2 time(s)

#### Expected output
_expected stdout:_

```text
at://did:plc:me/app.bsky.feed.post/new1
```

### Scenario: post makes a card from the page of its first link, for a reply too, but not beside pictures or a quote
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `photo.png` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky post "A note ${api.url}/page"
bsky post --reply at://did:plc:alice/app.bsky.feed.post/p1 "${api.url}/page"
bsky post --image photo.png "${api.url}/page"
bsky post --quote at://did:plc:alice/app.bsky.feed.post/p1 "${api.url}/page"
bsky post --quote at://did:plc:alice/app.bsky.feed.post/p1 ""
```

#### Then
- after `bsky post "A note ${api.url}/page"`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.uploadBlob` exactly 1 time(s)
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)
- after `bsky post --reply at://did:plc:alice/app.bsky.feed.post/p1 "${api.url}/page"`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 2 time(s)
- after `bsky post --image photo.png "${api.url}/page"`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 3 time(s)
- after `bsky post --quote at://did:plc:alice/app.bsky.feed.post/p1 "${api.url}/page"`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 4 time(s)
  - mock `api` received `/page` exactly 2 time(s)
- after `bsky post --quote at://did:plc:alice/app.bsky.feed.post/p1 ""`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 5 time(s)

### Scenario: a card links where the post does, and reads a relative picture against the page a redirect ended on
#### Given
- Stub HTTP server `site` serves 2 canned route(s) at `${site.url}` and records every request (#24).
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky post "${api.url}/old"
```

#### Then
- exit code is `0`
- mock `site` received `/dir/photo.png` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: a card is made for a link whose address is written in Japanese, its page asked for percent-encoded
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky post "【Golang】アプリ層のログ ${api.url}/post/ja/2026-11-17-golangアプリ層👍🏽/"
```

#### Then
- exit code is `0`
- mock `api` received `/post/ja/2026-11-17-golangアプリ層👍🏽/写真.png` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: post --no-card, Link cards off in the settings, and BSKY_LINK_CARDS=off do not read the page
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, BSKY_LINK_CARDS, TZ.
- Fixture file `cfg/settings.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/settings.json`:_

```text
{"link_cards": "off"}
```

#### When

```shell
bsky post --no-card "${api.url}/page"
bsky post "${api.url}/page"
bsky post "${api.url}/page"
```

#### Then
- after `bsky post --no-card "${api.url}/page"`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)
- after `bsky post "${api.url}/page"`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 2 time(s)
- after `bsky post "${api.url}/page"`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 3 time(s)
  - mock `api` received `/page` exactly 0 time(s)

### Scenario: a page that gives no card still lets the post go out, and a picture that cannot be had leaves the card without one
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky post "${api.url}/sjis"
bsky post "${api.url}/meta-sjis"
bsky post "${api.url}/download"
bsky post "${api.url}/empty"
bsky post "${api.url}/missing"
bsky post "http://127.0.0.1:1/x"
bsky post "${api.url}/title-only"
bsky post "${api.url}/nopic"
```

#### Then
- after `bsky post "${api.url}/sjis"`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)
- after `bsky post "${api.url}/meta-sjis"`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 2 time(s)
- after `bsky post "${api.url}/download"`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 3 time(s)
- after `bsky post "${api.url}/empty"`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 4 time(s)
- after `bsky post "${api.url}/missing"`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 5 time(s)
- after `bsky post "http://127.0.0.1:1/x"`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 6 time(s)
- after `bsky post "${api.url}/title-only"`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 7 time(s)
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 7 time(s)
- after `bsky post "${api.url}/nopic"`:
  - exit code is `0`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 8 time(s)
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 8 time(s)
  - mock `api` received `/xrpc/com.atproto.repo.uploadBlob` exactly 0 time(s)

### Scenario: a card whose picture the server will not take is sent without it
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky post "${api.url}/page"
```

#### Then
- exit code is `0`
- mock `api` received `/xrpc/com.atproto.repo.uploadBlob` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: a post too long, or with pictures and a video, is refused before anything is sent
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `clip.mp4` is created.
- Fixture file `photo.png` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky post "two things" --image photo.png --video clip.mp4
bsky post "あああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああ"
```

#### Then
- after `bsky post "two things" --image photo.png --video clip.mp4`:
  - exit code is `1`
  - stderr contains `error: a post can have up to 4 pictures or one video, not both`
- after `bsky post "あああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああああ"`:
  - exit code is `1`
  - stderr contains `error: the post is 301 characters; the limit is 300`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 0 time(s)

### Scenario: like and unlike take a bsky.app address, and a write the server fails is sent once
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky like https://bsky.app/profile/alice.test/post/p1
bsky unlike at://did:plc:alice/app.bsky.feed.post/p1 --json
```

#### Then
- after `bsky like https://bsky.app/profile/alice.test/post/p1`:
  - exit code is `0`
  - stdout equals an exact value
- after `bsky unlike at://did:plc:alice/app.bsky.feed.post/p1 --json`:
  - exit code is `4`
  - stdout at `$.status` equals `4`; at `$.error` equals `com.atproto.repo.deleteRecord failed: HTTP 502`
  - stderr contains `error: com.atproto.repo.deleteRecord failed: HTTP 502`
  - mock `api` received `/xrpc/com.atproto.repo.deleteRecord` exactly 1 time(s)
  - mock `api` received `/xrpc/app.bsky.feed.getPosts`

#### Expected output
_expected stdout:_

```text
already liked: at://did:plc:me/app.bsky.feed.like/l1
```

### Scenario: delete takes only your own post
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky delete at://did:plc:alice/app.bsky.feed.post/p1
bsky delete at://did:plc:me/app.bsky.feed.post/p9 --json
```

#### Then
- after `bsky delete at://did:plc:alice/app.bsky.feed.post/p1`:
  - exit code is `1`
  - stderr contains `error: you can only delete your own posts`
- after `bsky delete at://did:plc:me/app.bsky.feed.post/p9 --json`:
  - exit code is `0`
  - stdout at `$.action` equals `delete`; at `$.target` equals `at://did:plc:me/app.bsky.feed.post/p9`
  - mock `api` received `/xrpc/com.atproto.repo.deleteRecord` exactly 1 time(s)

### Scenario: delete takes your own post by an at:// address written with your handle
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky delete at://me.test/app.bsky.feed.post/p8
```

#### Then
- exit code is `0`
- stdout equals an exact value
- mock `api` received `/xrpc/com.atproto.repo.deleteRecord` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.identity.resolveHandle`

#### Expected output
_expected stdout:_

```text
deleted at://did:plc:me/app.bsky.feed.post/p8
```

### Scenario: notifications are listed, and marked seen only with --seen
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky notif
bsky notifications --seen --json
```

#### Then
- after `bsky notif`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/app.bsky.notification.updateSeen` exactly 0 time(s)
- after `bsky notifications --seen --json`:
  - exit code is `0`
  - stdout line `2` matches `/"type":"like"/`
  - mock `api` received `/xrpc/app.bsky.notification.updateSeen` exactly 1 time(s)

#### Expected output
_expected stdout:_

```text
● Bob @bob.test replied to you · 2026-09-21 09:00
Nice photo 👍🏽
at://did:plc:bob/app.bsky.feed.post/r1

@carol.test liked your post · 2026-09-21 08:00
at://did:plc:me/app.bsky.feed.post/p4
```

### Scenario: profile, followers and a search for accounts print accounts as lines
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky profile @bob.test
bsky followers bob.test
bsky search erin --accounts --json
```

#### Then
- after `bsky profile @bob.test`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/app.bsky.actor.getProfile`
- after `bsky followers bob.test`:
  - exit code is `0`
  - stdout equals an exact value
- after `bsky search erin --accounts --json`:
  - exit code is `0`
  - stdout at `$.username` equals `erin.test`; at `$.url` equals `https://bsky.app/profile/erin.test`
  - mock `api` received `/xrpc/app.bsky.actor.searchActors`

#### Expected output
_expected stdout:_

```text
Bob 🇯🇵 @bob.test
did:plc:bob
3 followers  4 following  5 posts

hello
world
```

_expected stdout:_

```text
Alice @alice.test  (following)  did:plc:alice
@erin.test  did:plc:erin
```

### Scenario: thread shows the posts above a reply and its replies indented
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky thread https://bsky.app/profile/bob.test/post/b1
```

#### Then
- exit code is `0`
- stdout equals an exact value
- mock `api` received `/xrpc/app.bsky.feed.getPostThread`

#### Expected output
_expected stdout:_

```text
@alice.test · 2026-09-20 10:00
Alice asks
♡ 0  ⟳ 0  ↩ 0
at://did:plc:alice/app.bsky.feed.post/a1

@bob.test · 2026-09-20 11:00
Bob answers
♡ 0  ⟳ 0  ↩ 0
at://did:plc:bob/app.bsky.feed.post/b1

  @carol.test · 2026-09-20 12:00
  Carol agrees
  ♡ 0  ⟳ 0  ↩ 0
  at://did:plc:carol/app.bsky.feed.post/c1
```

### Scenario: login reads the password from stdin, takes a handle written with @, adds the account, and accounts lists it
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_stdin for `bsky`:_

```text
app-pass-cli
```

#### When

```shell
bsky login @new.test --password-stdin --service ${api.url}
bsky accounts
bsky accounts --json
```

#### Then
- after `bsky login @new.test --password-stdin --service ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/com.atproto.server.createSession` exactly 1 time(s)
  - file `cfg/accounts/did_plc_new.json` at `$.accessJwt` equals `acc-new`
- after `bsky accounts`:
  - exit code is `0`
  - stdout equals an exact value
- after `bsky accounts --json`:
  - exit code is `0`
  - stdout line `2` matches `/"current":true.*"username":"new.test"|"username":"new.test".*"current":true/`

#### Expected output
_expected stdout:_

```text
logged in as @new.test
```

_expected stdout:_

```text
  @me.test  did:plc:me
* @new.test  did:plc:new
```

### Scenario: feed takes a pinned feed by name, and an unknown name lists the pinned ones
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky feed "cats 🐈"
bsky feed dogs
```

#### Then
- after `bsky feed "cats 🐈"`:
  - exit code is `0`
  - stdout contains `A cat on a keyboard`, `at://did:plc:erin/app.bsky.feed.post/s1`
  - mock `api` received `/xrpc/app.bsky.feed.getFeed`
- after `bsky feed dogs`:
  - exit code is `1`
  - stderr contains `error: no pinned feed is named "dogs"`, `hint: pinned: Cats 🐈`

### Scenario: without an account a command is a usage error that says how to log in
#### Given
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### When

```shell
bsky tl --json
```

#### Then
- exit code is `1`
- stdout at `$.status` equals `1`; at `$.error` equals `no account is logged in`
- stderr contains `error: no account is logged in`, `` hint: run `bsky login` ``

### Scenario: an account named by BSKY_ACCOUNT can be logged in, and accounts lists them all the same
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_ACCOUNT, BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_ACCOUNT, BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_stdin for `bsky`:_

```text
app-pass-cli
```

#### When

```shell
bsky accounts
bsky login @new.test --password-stdin --service ${api.url}
```

#### Then
- after `bsky accounts`:
  - exit code is `0`
  - stdout equals an exact value
- after `bsky login @new.test --password-stdin --service ${api.url}`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/com.atproto.server.createSession` exactly 1 time(s)

#### Expected output
_expected stdout:_

```text
* @me.test  did:plc:me
```

_expected stdout:_

```text
logged in as @new.test
```

### Scenario: with --json a usage error and logout print JSON too
#### Given
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky --json like
bsky logout --json
bsky logout --json
```

#### Then
- after `bsky --json like`:
  - exit code is `1`
  - stdout at `$.status` equals `1`; at `$.hint` equals `` run `bsky --help` for usage ``
  - stderr contains `` hint: run `bsky --help` for usage ``
- after `bsky logout --json`:
  - exit code is `0`
  - stdout at `$.loggedOut[0].username` equals `me.test`; at `$.loggedOut[0].id` equals `did:plc:me`
- after `bsky logout --json`:
  - exit code is `0`
  - stdout at `$.loggedOut` equals `[]`

### Scenario: timeline -n reads further pages until it has that many posts
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky tl -n 2
```

#### Then
- exit code is `0`
- stdout equals an exact value
- mock `api` received `/xrpc/app.bsky.feed.getTimeline` exactly 2 time(s)

#### Expected output
_expected stdout:_

```text
@me.test · 2026-09-20 07:00
First page
♡ 0  ⟳ 0  ↩ 0
at://did:plc:me/app.bsky.feed.post/p1
```

### Scenario: mute and block send once what the client sends, and mutes and blocks list the accounts
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky mute @alice.test
bsky block alice.test --json
bsky unmute alice.test
bsky unblock alice.test
bsky mutes
bsky blocks --json
```

#### Then
- after `bsky mute @alice.test`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/app.bsky.graph.muteActor` exactly 1 time(s)
- after `bsky block alice.test --json`:
  - exit code is `0`
  - stdout at `$.action` equals `block`; at `$.target` equals `did:plc:alice`; at `$.id` equals `at://did:plc:me/app.bsky.graph.block/b1`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)
- after `bsky unmute alice.test`:
  - exit code is `1`
  - stderr contains `error: you have not muted @alice.test`
- after `bsky unblock alice.test`:
  - exit code is `1`
  - stderr contains `error: you do not block @alice.test`
- after `bsky mutes`:
  - exit code is `0`
  - stdout equals an exact value
- after `bsky blocks --json`:
  - exit code is `0`
  - stdout at `$.id` equals `did:plc:alice`

#### Expected output
_expected stdout:_

```text
muted @alice.test
```

_expected stdout:_

```text
家族👨‍👩‍👧 Alice @alice.test  did:plc:alice
```

### Scenario: unmute and unblock undo a mute and the block record, and yourself is refused
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky unmute alice.test
bsky unblock alice.test
bsky mute alice.test
```

#### Then
- after `bsky unmute alice.test`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/app.bsky.graph.unmuteActor` exactly 1 time(s)
- after `bsky unblock alice.test`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/com.atproto.repo.deleteRecord` exactly 1 time(s)
- after `bsky mute alice.test`:
  - exit code is `0`
  - stdout equals an exact value

#### Expected output
_expected stdout:_

```text
unmuted @alice.test
```

_expected stdout:_

```text
unblocked @alice.test
```

_expected stdout:_

```text
already muted @alice.test
```

### Scenario: follow sends the follow record once, and follows lists the accounts an account follows
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky follow @alice.test
bsky unfollow alice.test
bsky follows alice.test --json
```

#### Then
- after `bsky follow @alice.test`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)
- after `bsky unfollow alice.test`:
  - exit code is `1`
  - stderr contains `error: you do not follow @alice.test`
- after `bsky follows alice.test --json`:
  - exit code is `0`
  - stdout at `$.username` equals `bob.test`; at `$.id` equals `did:plc:bob`
  - mock `api` received `/xrpc/app.bsky.graph.getFollows`

#### Expected output
_expected stdout:_

```text
following @alice.test
```

### Scenario: unfollow deletes the follow record, and follow of an account already followed sends nothing
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky unfollow alice.test
bsky follow alice.test
```

#### Then
- after `bsky unfollow alice.test`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/com.atproto.repo.deleteRecord` exactly 1 time(s)
- after `bsky follow alice.test`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 0 time(s)

#### Expected output
_expected stdout:_

```text
unfollowed @alice.test
```

_expected stdout:_

```text
already following @alice.test
```

### Scenario: repost sends the repost record once, and unrepost of a post not reposted is a usage error
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky repost https://bsky.app/profile/alice.test/post/p1 --json
bsky unrepost at://did:plc:alice/app.bsky.feed.post/p1
```

#### Then
- after `bsky repost https://bsky.app/profile/alice.test/post/p1 --json`:
  - exit code is `0`
  - stdout at `$.action` equals `repost`; at `$.target` equals `at://did:plc:alice/app.bsky.feed.post/p1`; at `$.id` equals `at://did:plc:me/app.bsky.feed.repost/r1`
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)
- after `bsky unrepost at://did:plc:alice/app.bsky.feed.post/p1`:
  - exit code is `1`
  - stderr contains `is not reposted`

### Scenario: unrepost deletes the repost record, and repost of a post already reposted sends nothing
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky unrepost at://did:plc:alice/app.bsky.feed.post/p1
bsky repost at://did:plc:alice/app.bsky.feed.post/p1
```

#### Then
- after `bsky unrepost at://did:plc:alice/app.bsky.feed.post/p1`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/com.atproto.repo.deleteRecord` exactly 1 time(s)
- after `bsky repost at://did:plc:alice/app.bsky.feed.post/p1`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 0 time(s)

#### Expected output
_expected stdout:_

```text
removed at://did:plc:me/app.bsky.feed.repost/r1
```

_expected stdout:_

```text
already reposted: at://did:plc:me/app.bsky.feed.repost/r1
```

### Scenario: likes and reposts list who liked and reposted a post named by its bsky.app address
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky likes https://bsky.app/profile/alice.test/post/p1
bsky likes at://did:plc:alice/app.bsky.feed.post/p1/ --json -n 1
bsky reposts at://did:plc:alice/app.bsky.feed.post/p1
bsky likes https://bsky.app/profile/alice.test
```

#### Then
- after `bsky likes https://bsky.app/profile/alice.test/post/p1`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/app.bsky.feed.getLikes`
- after `bsky likes at://did:plc:alice/app.bsky.feed.post/p1/ --json -n 1`:
  - exit code is `0`
  - stdout at `$.username` equals `bob.test`
  - mock `api` received `/xrpc/app.bsky.feed.getLikes`
- after `bsky reposts at://did:plc:alice/app.bsky.feed.post/p1`:
  - exit code is `0`
  - stdout equals an exact value
- after `bsky likes https://bsky.app/profile/alice.test`:
  - exit code is `1`
  - stderr contains `is not a post`

#### Expected output
_expected stdout:_

```text
Bob 🇯🇵 @bob.test  did:plc:bob
@erin.test  did:plc:erin
```

_expected stdout:_

```text
家族👨‍👩‍👧 Carol @carol.test  did:plc:carol
```

### Scenario: lists prints the lists an account made, and list prints a list's members
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky lists alice.test
bsky list https://bsky.app/profile/alice.test/lists/l1
bsky list at://did:plc:alice/app.bsky.graph.list/l1 --json
```

#### Then
- after `bsky lists alice.test`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/app.bsky.graph.getLists`
- after `bsky list https://bsky.app/profile/alice.test/lists/l1`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/app.bsky.graph.getList`
- after `bsky list at://did:plc:alice/app.bsky.graph.list/l1 --json`:
  - exit code is `0`
  - stdout line `1` matches `/"username":"bob.test"/`

#### Expected output
_expected stdout:_

```text
Rust 🦀 仲間 · curation list · 2 members
People who write Rust
at://did:plc:alice/app.bsky.graph.list/l1
```

_expected stdout:_

```text
Bob @bob.test  did:plc:bob
@erin.test  did:plc:erin
```

### Scenario: report sends a post or an account to the moderators once, and an unknown reason is a usage error
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky report https://bsky.app/profile/spammer.test/post/s1 --reason spam --comment "同じ宣伝を何度も 🔁"
bsky report @spammer.test --reason rude --json
bsky report @spammer.test --reason angry
```

#### Then
- after `bsky report https://bsky.app/profile/spammer.test/post/s1 --reason spam --comment "同じ宣伝を何度も 🔁"`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/com.atproto.moderation.createReport` exactly 1 time(s)
- after `bsky report @spammer.test --reason rude --json`:
  - exit code is `0`
  - stdout at `$.action` equals `report`; at `$.id` equals `7`
  - mock `api` received `/xrpc/com.atproto.moderation.createReport` exactly 2 time(s)
- after `bsky report @spammer.test --reason angry`:
  - exit code is `1`
  - mock `api` received `/xrpc/com.atproto.moderation.createReport` exactly 2 time(s)

#### Expected output
_expected stdout:_

```text
reported at://did:plc:spammer/app.bsky.feed.post/s1
```

### Scenario: app-passwords lists, adds and revokes, and an app-password session is told why it cannot
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky app-passwords
bsky app-passwords add bsky
bsky app-passwords revoke phone
```

#### Then
- after `bsky app-passwords`:
  - exit code is `0`
  - stdout equals an exact value
- after `bsky app-passwords add bsky`:
  - exit code is `0`
  - stdout contains `abcd-efgh-ijkl-mnop`, `it is not shown again`
  - mock `api` received `/xrpc/com.atproto.server.createAppPassword` exactly 1 time(s)
- after `bsky app-passwords revoke phone`:
  - exit code is `4`
  - stderr contains `hint: log in with the account's password`
  - mock `api` received `/xrpc/com.atproto.server.revokeAppPassword` exactly 1 time(s)

#### Expected output
_expected stdout:_

```text
laptop 💻  2026-09-20 10:00  (direct messages)
phone  2026-09-19 08:30
```

### Scenario: a pipe closed early ends the output quietly with exit 0
_skipped on Windows_
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
{ bsky tl -n 300 --json; echo "bsky=$?" >&2; } | head -c 1
{ bsky tl -n 300; echo "bsky=$?" >&2; } | head -c 1
```

#### Then
- after `{ bsky tl -n 300 --json; echo "bsky=$?" >&2; } | head -c 1`:
  - stderr contains `bsky=0`, does not contain `panicked`, `Broken pipe`
- after `{ bsky tl -n 300; echo "bsky=$?" >&2; } | head -c 1`:
  - stderr contains `bsky=0`, does not contain `Broken pipe`

### Scenario: help and logout that cannot write their output exit 3
_only on Linux_
#### Given
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "https://bsky.social", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky --help > /dev/full
bsky logout > /dev/full
bsky accounts
```

#### Then
- after `bsky --help > /dev/full`:
  - exit code is `3`
  - stderr contains `error: cannot write the output`
- after `bsky logout > /dev/full`:
  - exit code is `3`
  - stderr contains `error: cannot write the output`
- after `bsky accounts`:
  - exit code is `0`
  - stdout equals an exact value

#### Expected output
_expected stdout:_

```text
not logged in
```

### Scenario: unmute of an account muted by a list says which list, and sends nothing
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky unmute alice.test
bsky mute alice.test
```

#### Then
- after `bsky unmute alice.test`:
  - exit code is `1`
  - stderr contains `@alice.test is muted by your list "Spam 🚫"`, `hint: take them off that list`
  - mock `api` received `/xrpc/app.bsky.graph.unmuteActor` exactly 0 time(s)
- after `bsky mute alice.test`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/app.bsky.graph.muteActor` exactly 1 time(s)

#### Expected output
_expected stdout:_

```text
muted @alice.test
```

### Scenario: author prints an account's posts page by page, reposts only with --reposts, and sends the filter
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky author @alice.test -n 10
bsky author alice.test --reposts --filter no-replies -n 2
bsky author alice.test --filter posts_with_media --json
bsky author alice.test --filter posts_with_media --raw
bsky author alice.test --filter everything
```

#### Then
- after `bsky author @alice.test -n 10`:
  - exit code is `0`
  - stdout equals an exact value
  - stderr is empty
  - mock `api` received `/xrpc/app.bsky.feed.getAuthorFeed` exactly 2 time(s)
  - mock `api` received `/xrpc/app.bsky.feed.getAuthorFeed`
  - mock `api` received `/xrpc/app.bsky.feed.getAuthorFeed`
- after `bsky author alice.test --reposts --filter no-replies -n 2`:
  - exit code is `0`
  - stdout equals an exact value
  - mock `api` received `/xrpc/app.bsky.feed.getAuthorFeed` exactly 3 time(s)
  - mock `api` received `/xrpc/app.bsky.feed.getAuthorFeed`
- after `bsky author alice.test --filter posts_with_media --json`:
  - exit code is `0`
  - stdout matches `/^\{[^\n]*"id":"at://did:plc:alice/app.bsky.feed.post/p1"[^\n]*\}\n\{[^\n]*"id":"at://did:plc:alice/app.bsky.feed.post/p2"[^\n]*\}\n$/`
- after `bsky author alice.test --filter posts_with_media --raw`:
  - exit code is `0`
  - stdout matches `/^\{"post":\{.*"uri":"at://did:plc:alice/app.bsky.feed.post/p1".*\}\n\{"post":\{.*"uri":"at://did:plc:alice/app.bsky.feed.post/p2".*\}\n$/`
  - mock `api` received `/xrpc/app.bsky.feed.getAuthorFeed`
- after `bsky author alice.test --filter everything`:
  - exit code is `1`
  - stderr contains `invalid value 'everything' for '--filter <FILTER>'`
  - mock `api` received `/xrpc/app.bsky.feed.getAuthorFeed` exactly 7 time(s)

#### Expected output
_expected stdout:_

```text
家族👨‍👩‍👧 Alice @alice.test · 2026-09-20 10:00
山の写真 🇯🇵 1️⃣ ❤️
二行目 👋🏽
[4 pictures]
♡ 3  ⟳ 0  ↩ 0
at://did:plc:alice/app.bsky.feed.post/p1

家族👨‍👩‍👧 Alice @alice.test · 2026-09-20 08:00
返信です
♡ 0  ⟳ 0  ↩ 0
at://did:plc:alice/app.bsky.feed.post/p2
```

_expected stdout:_

```text
家族👨‍👩‍👧 Alice @alice.test · 2026-09-20 10:00
山の写真 🇯🇵 1️⃣ ❤️
二行目 👋🏽
[4 pictures]
♡ 3  ⟳ 0  ↩ 0
at://did:plc:alice/app.bsky.feed.post/p1

⟳ reposted by @alice.test
@dave.test · 2026-09-20 09:00
Dave's post, reposted
♡ 0  ⟳ 0  ↩ 0
at://did:plc:dave/app.bsky.feed.post/r1
```

### Scenario: download saves the four pictures of a post, and a file already there is not overwritten
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `写真 👨‍👩‍👧/bafy1.png` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, BSKY_DOWNLOAD_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `写真 👨‍👩‍👧/bafy1.png`:_

```text
old
```

#### When

```shell
bsky download https://bsky.app/profile/alice.test/post/p1 -o "${workdir}/写真 👨‍👩‍👧"
bsky download at://did:plc:alice/app.bsky.feed.post/p1 --json
```

#### Then
- after `bsky download https://bsky.app/profile/alice.test/post/p1 -o "${workdir}/写真 👨‍👩‍👧"`:
  - exit code is `0`
  - stdout matches `/^\S.*写真 👨‍👩‍👧[/\\]bafy1 \(1\)\.png\n.*[/\\]bafy2\.png\n.*[/\\]bafy3\.png\n.*[/\\]bafy4\.png\n$/`
  - stderr is empty
  - dir `写真 👨‍👩‍👧` contains `bafy1.png`, contains `bafy1 (1).png`, contains `bafy2.png`, contains `bafy3.png`, contains `bafy4.png`, has 5 entries
  - file `写真 👨‍👩‍👧/bafy1.png` equals exact bytes
  - image `写真 👨‍👩‍👧/bafy1 (1).png` width 120, height 90
  - image `写真 👨‍👩‍👧/bafy4.png` width 120, height 90
  - mock `api` received `/xrpc/app.bsky.feed.getPosts`
- after `bsky download at://did:plc:alice/app.bsky.feed.post/p1 --json`:
  - exit code is `0`
  - stdout matches `/^\{"path":"[^"]*dl[/\\]+bafy1\.png","postId":"at://did:plc:alice/app\.bsky\.feed\.post/p1","type":"image","url":"http://[^"]+/img/plain/did:plc:alice/bafy1@jpeg"\}\n(\{"path":[^\n]*bafy[234]\.png","postId":[^\n]*"type":"image"[^\n]*\}\n){3}$/`
  - dir `dl` contains `bafy1.png`, contains `bafy2.png`, contains `bafy3.png`, contains `bafy4.png`, has 4 entries
  - mock `api` received `/img/plain/did:plc:alice/bafy4@jpeg` exactly 2 time(s)

#### Generated artifacts
- `写真 👨‍👩‍👧/bafy1 (1).png`
- `写真 👨‍👩‍👧/bafy4.png`

### Scenario: download saves a post's video as one .ts file in the download folder
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `seg0.ts` is created.
- Environment variables are set: BSKY_CONFIG_DIR, BSKY_DOWNLOAD_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky download at://did:plc:alice/app.bsky.feed.post/v1 --json
```

#### Then
- exit code is `0`
- stdout at `$.type` equals `video`; at `$.url` matches `//watch/v1/playlist\.m3u8$/`; at `$.path` matches `/downloads[/\\]v1\.ts$/`
- dir `downloads` contains `v1.ts`, has 1 entry
- file `downloads/v1.ts` is byte-identical to `seg0.ts`
- mock `api` received `/watch/v1/v/seg0.ts` exactly 1 time(s)

### Scenario: download of a post without pictures or video is a usage error and writes nothing
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky download at://did:plc:alice/app.bsky.feed.post/p1 -o out
bsky download not-a-post
```

#### Then
- after `bsky download at://did:plc:alice/app.bsky.feed.post/p1 -o out`:
  - exit code is `1`
  - stdout is empty
  - stderr contains `error: at://did:plc:alice/app.bsky.feed.post/p1 has no pictures or video`
  - file `out` does not exist
- after `bsky download not-a-post`:
  - exit code is `1`
  - stderr contains `error: not-a-post is not a post`

### Scenario: author --download saves the pictures and videos of the posts it read
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, BSKY_DOWNLOAD_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky author alice.test --download
bsky author alice.test -n 1 --download picked --json
```

#### Then
- after `bsky author alice.test --download`:
  - exit code is `0`
  - stdout matches `/^.*downloads[/\\]bafy1\.png\n.*downloads[/\\]v1\.ts\n$/`
  - dir `downloads` contains `bafy1.png`, contains `v1.ts`, has 2 entries
- after `bsky author alice.test -n 1 --download picked --json`:
  - exit code is `0`
  - stdout at `$.postId` equals `at://did:plc:alice/app.bsky.feed.post/p1`; at `$.path` matches `/picked[/\\]bafy1\.png$/`
  - dir `picked` contains `bafy1.png`, has 1 entry

### Scenario: post --dry-run shows the post and its card and sends nothing
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `photo.png` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky post --dry-run --reply at://did:plc:alice/app.bsky.feed.post/p1 "新しい記事 🐈‍⬛ ${api.url}/page"
bsky post --dry-run --json --image photo.png --alt "写真 📷" "pictures, so no card ${api.url}/page"
```

#### Then
- after `bsky post --dry-run --reply at://did:plc:alice/app.bsky.feed.post/p1 "新しい記事 🐈‍⬛ ${api.url}/page"`:
  - exit code is `0`
  - stdout contains `"dry run: nothing was sent to 127.0.0.1:${api.port}\nas @me.test\n"`, `"reply to: at://did:plc:alice/app.bsky.feed.post/p1\n"`, `" of 300 characters):\n新しい記事 🐈\u200d⬛ ${api.url}/page\n"`, `"card of ${api.url}/page:\n  title: A note & its card\n  description: What the page is about\n  image: ${api.url}/img/photo.png\n"`
- after `bsky post --dry-run --json --image photo.png --alt "写真 📷" "pictures, so no card ${api.url}/page"`:
  - exit code is `0`
  - stdout at `$.dryRun` equals `true`; at `$.host` equals `127.0.0.1:${api.port}`; at `$.username` equals `me.test`; at `$.post.text` equals `pictures, so no card ${api.url}/page`; at `$.post.files[0].type` equals `image`; at `$.post.files[0].alt` equals `写真 📷`; at `$.maxCharacters` equals `300`; at `$.card` equals `null`
  - mock `api` received `/page` exactly 1 time(s)
  - mock `api` received `/img/photo.png` exactly 0 time(s)
  - mock `api` received `/xrpc/com.atproto.repo.uploadBlob` exactly 0 time(s)
  - mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 0 time(s)

### Scenario: whoami prints the account in use
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.
- Environment variables are set: BSKY_CONFIG_DIR, TZ.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
bsky whoami --json
bsky whoami
```

#### Then
- after `bsky whoami --json`:
  - exit code is `0`
  - stdout at `$.id` equals `did:plc:me`; at `$.username` equals `me.test`; at `$.name` equals `私 🇯🇵`; at `$.postsCount` equals `3`
  - mock `api` received `/xrpc/app.bsky.actor.getProfile`
- after `bsky whoami`:
  - exit code is `0`
  - stdout equals an exact value

#### Expected output
_expected stdout:_

```text
私 🇯🇵 @me.test
did:plc:me
1 followers  2 following  3 posts
```

