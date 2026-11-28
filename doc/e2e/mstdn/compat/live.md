# atago Behavior Specs
## Summary
1 suite · 42 scenarios
## Contents
- [mstdn live catalog](#mstdn-live-catalog): 42 scenarios
  - [usable: token identity](#scenario-usable-token-identity)
  - [usable: home timeline newest page](#scenario-usable-home-timeline-newest-page)
  - [usable: local timeline newest page](#scenario-usable-local-timeline-newest-page)
  - [usable: public timeline newest page](#scenario-usable-public-timeline-newest-page)
  - [usable: three pages of video posts and end count](#scenario-usable-three-pages-of-video-posts-and-end-count)
  - [usable: post reply and thread](#scenario-usable-post-reply-and-thread)
  - [usable: picture upload alt text and download](#scenario-usable-picture-upload-alt-text-and-download)
  - [usable: video upload](#scenario-usable-video-upload)
  - [usable: content warning and public visibility](#scenario-usable-content-warning-and-public-visibility)
  - [usable: content warning and unlisted visibility](#scenario-usable-content-warning-and-unlisted-visibility)
  - [usable: content warning and private visibility](#scenario-usable-content-warning-and-private-visibility)
  - [usable: content warning and direct visibility](#scenario-usable-content-warning-and-direct-visibility)
  - [usable: notification kinds](#scenario-usable-notification-kinds)
  - [usable: hashtag search](#scenario-usable-hashtag-search)
  - [heavy: 100 four-picture posts](#scenario-heavy-100-four-picture-posts)
  - [heavy: 50-deep thread](#scenario-heavy-50-deep-thread)
  - [heavy: 200 replies](#scenario-heavy-200-replies)
  - [heavy: at least 500 notifications](#scenario-heavy-at-least-500-notifications)
  - [heavy: longest text and one over](#scenario-heavy-longest-text-and-one-over)
  - [behavior: wrong token](#scenario-behavior-wrong-token)
  - [behavior: deleted status remains a resource error](#scenario-behavior-deleted-status-remains-a-resource-error)
  - [speed: timeline under 10s](#scenario-speed-timeline-under-10s)
  - [speed: notifications under 10s](#scenario-speed-notifications-under-10s)
  - [speed: search under 10s](#scenario-speed-search-under-10s)
  - [speed: picture post under 30s](#scenario-speed-picture-post-under-30s)
  - [usable: kitty timeline thread compose and quit](#scenario-usable-kitty-timeline-thread-compose-and-quit)
  - [heavy: scroll compatvideo down and back with kitty](#scenario-heavy-scroll-compatvideo-down-and-back-with-kitty)
  - [heavy: scroll compatpictures down and back with kitty](#scenario-heavy-scroll-compatpictures-down-and-back-with-kitty)
  - [heavy: preserved unsupported codec offers saving](#scenario-heavy-preserved-unsupported-codec-offers-saving)
  - [usable: favourite boost and take both back](#scenario-usable-favourite-boost-and-take-both-back)
  - [usable: delete own post in TUI](#scenario-usable-delete-own-post-in-tui)
  - [usable: unsupported quotes name the server and preserve navigation](#scenario-usable-unsupported-quotes-name-the-server-and-preserve-navigation)
  - [usable: quote with the server capability](#scenario-usable-quote-with-the-server-capability)
  - [heavy: four pictures and client attachment ceiling](#scenario-heavy-four-pictures-and-client-attachment-ceiling)
  - [heavy: advertised or fallback alt text limit and one over](#scenario-heavy-advertised-or-fallback-alt-text-limit-and-one-over)
  - [behavior: server restart during a TUI session](#scenario-behavior-server-restart-during-a-tui-session)
  - [behavior: a blocked account is a resource error and unblocking restores reads](#scenario-behavior-a-blocked-account-is-a-resource-error-and-unblocking-restores-reads)
  - [behavior: blocking leaves a public status readable](#scenario-behavior-blocking-leaves-a-public-status-readable)
  - [usable: search accounts in the full-screen client](#scenario-usable-search-accounts-in-the-full-screen-client)
  - [heavy: Unicode text name and alt text survive JSON](#scenario-heavy-unicode-text-name-and-alt-text-survive-json)
  - [heavy: published video byte limit and one over](#scenario-heavy-published-video-byte-limit-and-one-over)
  - [usable: unsupported quote policy preserves the draft and permits recovery](#scenario-usable-unsupported-quote-policy-preserves-the-draft-and-permits-recovery)

## mstdn live catalog
Source: `crates/mastodon-terminal-client/e2e/compat/live.atago.yaml`
### Scenario: usable: token identity
#### When

```shell
mstdn --json whoami
```

#### Then
- exit code is `0`
- stdout at `$.id` equals `${env:MSTDN_SEED_ALICE}`; at `$.username` equals `alice`

### Scenario: usable: home timeline newest page
#### When

```shell
mstdn --json timeline home -n 1
```

#### Then
- exit code is `0`
- stdout at `$.id` equals `${env:MSTDN_SEED_VIEWER_PICTURE}`; at `$.text` equals `compatibility viewer picture`

### Scenario: usable: local timeline newest page
#### When

```shell
mstdn --json timeline local -n 1
```

#### Then
- exit code is `0`
- stdout at `$.id` equals `${env:MSTDN_SEED_VIEWER_PICTURE}`; at `$.text` equals `compatibility viewer picture`

### Scenario: usable: public timeline newest page
#### When

```shell
mstdn --json timeline public -n 1
```

#### Then
- exit code is `0`
- stdout at `$.id` equals `${env:MSTDN_SEED_VIEWER_PICTURE}`; at `$.text` equals `compatibility viewer picture`

### Scenario: usable: three pages of video posts and end count
#### When

```shell
mstdn --json author videos -n 150
```

#### Then
- exit code is `0`
- stdout matches `/^(?:\{[^\n]+\}\n){100}$/`
- stdout matches `/^\{[^\n]*"text":"video099 #compatvideo"[^\n]*\n(?:[^\n]*\n){98}\{[^\n]*"text":"video000 #compatvideo"[^\n]*\n$/`

### Scenario: usable: post reply and thread
#### When

```shell
mstdn --json post "compatibility live post #compatlive"
# capture ${root} from stdout
mstdn --json post "compatibility live reply" --reply ${root}
# capture ${reply} from stdout
mstdn --json thread ${root}
```

#### Then
- after `mstdn --json post "compatibility live post #compatlive"`:
  - exit code is `0`
- after `mstdn --json post "compatibility live reply" --reply ${root}`:
  - exit code is `0`
- after `mstdn --json thread ${root}`:
  - exit code is `0`
  - stdout matches `/^(?:\{[^\n]+\}\n){2}$/`
  - stdout contains `"id":"${root}"`, `"id":"${reply}"`, `"replyId":"${root}"`

### Scenario: usable: picture upload alt text and download
#### When

```shell
mstdn --json post "compatibility live picture" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --alt "Round trip alt 👩🏽‍💻"
# capture ${picture} from stdout
mstdn --json status ${picture}
mstdn --json download ${picture} -o dl
# capture ${download} from stdout
```

#### Then
- after `mstdn --json post "compatibility live picture" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --alt "Round trip alt 👩🏽‍💻"`:
  - exit code is `0`
- after `mstdn --json status ${picture}`:
  - exit code is `0`
  - stdout at `$.files[0].type` equals `image`; at `$.files[0].description` equals `Round trip alt 👩🏽‍💻`
- after `mstdn --json download ${picture} -o dl`:
  - exit code is `0`
  - stdout at `$.postId` equals `${picture}`
  - image `${download}` width 64, height 64

#### Generated artifacts
- `${download}`

### Scenario: usable: video upload
#### When

```shell
mstdn --json post "compatibility live video" --video "${env:MSTDN_COMPAT_SEED}/clip.mp4" --alt "Round trip video"
# capture ${video} from stdout
mstdn --json status ${video}
```

#### Then
- after `mstdn --json post "compatibility live video" --video "${env:MSTDN_COMPAT_SEED}/clip.mp4" --alt "Round trip video"`:
  - exit code is `0`
- after `mstdn --json status ${video}`:
  - exit code is `0`
  - stdout at `$.files[0].type` equals `video`; at `$.files[0].description` equals `Round trip video`

### Scenario: usable: content warning and public visibility
#### When

```shell
mstdn --json post "@bob visibility round trip" --cw "Round trip CW" --visibility public
# capture ${post} from stdout
mstdn --json status ${post}
```

#### Then
- after `mstdn --json post "@bob visibility round trip" --cw "Round trip CW" --visibility public`:
  - exit code is `0`
- after `mstdn --json status ${post}`:
  - exit code is `0`
  - stdout at `$.cw` equals `Round trip CW`; at `$.visibility` equals `public`

### Scenario: usable: content warning and unlisted visibility
#### When

```shell
mstdn --json post "@bob visibility round trip" --cw "Round trip CW" --visibility unlisted
# capture ${post} from stdout
mstdn --json status ${post}
```

#### Then
- after `mstdn --json post "@bob visibility round trip" --cw "Round trip CW" --visibility unlisted`:
  - exit code is `0`
- after `mstdn --json status ${post}`:
  - exit code is `0`
  - stdout at `$.cw` equals `Round trip CW`; at `$.visibility` equals `unlisted`

### Scenario: usable: content warning and private visibility
#### When

```shell
mstdn --json post "@bob visibility round trip" --cw "Round trip CW" --visibility private
# capture ${post} from stdout
mstdn --json status ${post}
```

#### Then
- after `mstdn --json post "@bob visibility round trip" --cw "Round trip CW" --visibility private`:
  - exit code is `0`
- after `mstdn --json status ${post}`:
  - exit code is `0`
  - stdout at `$.cw` equals `Round trip CW`; at `$.visibility` equals `private`

### Scenario: usable: content warning and direct visibility
#### When

```shell
mstdn --json post "@bob visibility round trip" --cw "Round trip CW" --visibility direct
# capture ${post} from stdout
mstdn --json status ${post}
```

#### Then
- after `mstdn --json post "@bob visibility round trip" --cw "Round trip CW" --visibility direct`:
  - exit code is `0`
- after `mstdn --json status ${post}`:
  - exit code is `0`
  - stdout at `$.cw` equals `Round trip CW`; at `$.visibility` equals `direct`

### Scenario: usable: notification kinds
#### When

```shell
mstdn --json notifications -n 2000
```

#### Then
- exit code is `0`
- stdout contains `"type":"mention"`, `"type":"follow"`, `"type":"like"`, `"type":"repost"`

### Scenario: usable: hashtag search
#### When

```shell
mstdn --json search "#compatvideo" -n 150
```

#### Then
- exit code is `0`
- stdout matches `/^(?:\{[^\n]+\}\n){100}$/`

### Scenario: heavy: 100 four-picture posts
#### When

```shell
mstdn --json search "#compatpictures" -n 150
```

#### Then
- exit code is `0`
- stdout matches `/^(?:\{[^\n]+\}\n){100}$/`

### Scenario: heavy: 50-deep thread
#### When

```shell
mstdn --json thread ${env:MSTDN_SEED_DEEP_LAST}
```

#### Then
- exit code is `0`
- stdout matches `/^(?:\{[^\n]+\}\n){51}$/`
- stdout contains `"id":"${env:MSTDN_SEED_DEEP}"`, `"id":"${env:MSTDN_SEED_DEEP_LAST}"`

### Scenario: heavy: 200 replies
#### When

```shell
mstdn --json thread ${env:MSTDN_SEED_WIDE}
```

#### Then
- exit code is `0`
- stdout matches `/^(?:\{[^\n]+\}\n){201}$/`

### Scenario: heavy: at least 500 notifications
#### When

```shell
mstdn --json notifications -n 2000
```

#### Then
- exit code is `0`
- stdout matches `/^(?:\{[^\n]+\}\n){500,}$/`

### Scenario: heavy: longest text and one over
#### Given
- Fixture file `max-text.txt` is created.
- Fixture file `over-text.txt` is created.

#### Inputs
_stdin for `mstdn`:_

```text
(read from file max-text.txt)
```

_stdin for `mstdn`:_

```text
(read from file over-text.txt)
```

#### When

```shell
mstdn --json post -
mstdn --json post -
```

#### Then
- after `mstdn --json post -`:
  - exit code is `0`
- after `mstdn --json post -`:
  - exit code is `1`
  - stdout contains `takes up to`, `${env:MSTDN_LIMIT_TEXT}`

### Scenario: behavior: wrong token
#### Given
- Environment variables are set: MSTDN_TOKEN.

#### When

```shell
mstdn --json whoami
```

#### Then
- exit code is `4`
- stdout at `$.status` equals `4`

### Scenario: behavior: deleted status remains a resource error
#### When

```shell
mstdn --json status ${env:MSTDN_SEED_DELETED}
```

#### Then
- exit code is `4`
- stdout at `$.status` equals `4`

### Scenario: speed: timeline under 10s
#### When

```shell
mstdn --json timeline home -n 100
```

#### Then
- exit code is `0`
- stdout matches `/^(?:\{[^\n]+\}\n){100}$/`
- completes in under 10s

### Scenario: speed: notifications under 10s
#### When

```shell
mstdn --json notifications -n 100
```

#### Then
- exit code is `0`
- stdout matches `/^(?:\{[^\n]+\}\n){100}$/`
- completes in under 10s

### Scenario: speed: search under 10s
#### When

```shell
mstdn --json search "#compatvideo" -n 100
```

#### Then
- exit code is `0`
- stdout matches `/^(?:\{[^\n]+\}\n){100}$/`
- completes in under 10s

### Scenario: speed: picture post under 30s
#### When

```shell
mstdn --json post "speed picture" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --alt "Speed picture"
```

#### Then
- exit code is `0`
- stdout matches `/^(?:\{[^\n]+\}\n){1}$/`
- completes in under 30s

### Scenario: usable: kitty timeline thread compose and quit
#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`

### Scenario: heavy: scroll compatvideo down and back with kitty
#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`

### Scenario: heavy: scroll compatpictures down and back with kitty
#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`

### Scenario: heavy: preserved unsupported codec offers saving
_only when env MSTDN_PRESERVES_WEBM is set_
#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`

### Scenario: usable: favourite boost and take both back
#### Given
- The step is retried up to 30 times every 200ms until exit code is `0` and stdout at `$.repostCount` equals `1`.

#### When

```shell
# interactive (pty): mstdn
mstdn --json status ${env:MSTDN_SEED_TEXT}
# interactive (pty): mstdn
mstdn --json status ${env:MSTDN_SEED_TEXT}
```

#### Then
- after `interactive (pty): mstdn`:
  - exit code is `0`
- after `mstdn --json status ${env:MSTDN_SEED_TEXT}`:
  - exit code is `0`
  - stdout at `$.likeCount` equals `2`; at `$.repostCount` equals `2`
- after `interactive (pty): mstdn`:
  - exit code is `0`
- after `mstdn --json status ${env:MSTDN_SEED_TEXT}`:
  - exit code is `0`
  - stdout at `$.likeCount` equals `1`; at `$.repostCount` equals `1`

### Scenario: usable: delete own post in TUI
#### When

```shell
mstdn --json post "delete me #compatdelete"
# capture ${deleted} from stdout
# interactive (pty): mstdn
mstdn --json status ${deleted}
```

#### Then
- after `mstdn --json post "delete me #compatdelete"`:
  - exit code is `0`
- after `interactive (pty): mstdn`:
  - exit code is `0`
- after `mstdn --json status ${deleted}`:
  - exit code is `4`
  - stdout at `$.status` equals `4`

### Scenario: usable: unsupported quotes name the server and preserve navigation
_only when env MSTDN_NO_QUOTES is set_
#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`

### Scenario: usable: quote with the server capability
_only when env MSTDN_HAS_QUOTES is set_
#### When

```shell
mstdn --json post "compatibility CLI quote" --quote ${env:MSTDN_SEED_TEXT} --dry-run
mstdn --json post "compatibility CLI quote" --quote ${env:MSTDN_SEED_TEXT}
# capture ${cli_quote} from stdout
mstdn --json status ${cli_quote}
# interactive (pty): mstdn
# HTTP GET /api/v1/timelines/tag/compatmadequote via peer
```

#### Then
- after `mstdn --json post "compatibility CLI quote" --quote ${env:MSTDN_SEED_TEXT} --dry-run`:
  - exit code is `0`
  - stdout at `$.post.quoteId` equals `${env:MSTDN_SEED_TEXT}`
- after `mstdn --json post "compatibility CLI quote" --quote ${env:MSTDN_SEED_TEXT}`:
  - exit code is `0`
- after `mstdn --json status ${cli_quote}`:
  - exit code is `0`
  - stdout at `$.quoteId` equals `${env:MSTDN_SEED_TEXT}`
- after `interactive (pty): mstdn`:
  - exit code is `0`
- after `HTTP GET /api/v1/timelines/tag/compatmadequote`:
  - HTTP status is `200`
  - body at `${env:MSTDN_QUOTE_PATH}` equals `${env:MSTDN_SEED_TEXT}`

### Scenario: heavy: four pictures and client attachment ceiling
#### When

```shell
mstdn --json post "four pictures" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --image "${env:MSTDN_COMPAT_SEED}/picture.png"
# capture ${created} from stdout
mstdn --json status ${created}
mstdn --json post "five pictures" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --image "${env:MSTDN_COMPAT_SEED}/picture.png"
```

#### Then
- after `mstdn --json post "four pictures" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --image "${env:MSTDN_COMPAT_SEED}/picture.png"`:
  - exit code is `0`
- after `mstdn --json status ${created}`:
  - exit code is `0`
  - stdout at `$.files` has length 4
- after `mstdn --json post "five pictures" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --image "${env:MSTDN_COMPAT_SEED}/picture.png"`:
  - exit code is `1`
  - stdout contains `at most 4 pictures`

### Scenario: heavy: advertised or fallback alt text limit and one over
#### Given
- Fixture file `max-alt.txt` is created.
- Fixture file `over-alt.txt` is created.

#### When

```shell
# capture ${alt} from file max-alt.txt
# capture ${over} from file over-alt.txt
mstdn --json post "max alt" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --alt "${alt}"
# capture ${created} from stdout
mstdn --json status ${created}
mstdn --json post "over alt" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --alt "${over}"
```

#### Then
- after `mstdn --json post "max alt" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --alt "${alt}"`:
  - exit code is `0`
- after `mstdn --json status ${created}`:
  - exit code is `0`
  - stdout at `$.files[0].description` equals `${alt}`
- after `mstdn --json post "over alt" --image "${env:MSTDN_COMPAT_SEED}/picture.png" --alt "${over}"`:
  - exit code is `1`
  - stdout contains `takes up to`, `${env:MSTDN_LIMIT_ALT}`

### Scenario: behavior: server restart during a TUI session
#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`

### Scenario: behavior: a blocked account is a resource error and unblocking restores reads
_only when env MSTDN_BLOCKED_HIDDEN is set_
#### When

```shell
# HTTP POST /api/v1/accounts/${env:MSTDN_SEED_ALICE}/block via peer
mstdn --json status ${env:MSTDN_SEED_REPLY}
# HTTP POST /api/v1/accounts/${env:MSTDN_SEED_ALICE}/unblock via peer
mstdn --json status ${env:MSTDN_SEED_REPLY}
```

#### Then
- after `HTTP POST /api/v1/accounts/${env:MSTDN_SEED_ALICE}/block`:
  - HTTP status is `200`
- after `mstdn --json status ${env:MSTDN_SEED_REPLY}`:
  - exit code is `4`
  - stdout at `$.status` equals `4`
- after `HTTP POST /api/v1/accounts/${env:MSTDN_SEED_ALICE}/unblock`:
  - HTTP status is `200`
- after `mstdn --json status ${env:MSTDN_SEED_REPLY}`:
  - exit code is `0`
  - stdout at `$.id` equals `${env:MSTDN_SEED_REPLY}`

### Scenario: behavior: blocking leaves a public status readable
_only when env MSTDN_BLOCKED_PUBLIC is set_
#### When

```shell
# HTTP POST /api/v1/accounts/${env:MSTDN_SEED_ALICE}/block via peer
mstdn --json status ${env:MSTDN_SEED_REPLY}
# HTTP POST /api/v1/accounts/${env:MSTDN_SEED_ALICE}/unblock via peer
mstdn --json status ${env:MSTDN_SEED_REPLY}
```

#### Then
- after `HTTP POST /api/v1/accounts/${env:MSTDN_SEED_ALICE}/block`:
  - HTTP status is `200`
- after `mstdn --json status ${env:MSTDN_SEED_REPLY}`:
  - exit code is `0`
  - stdout at `$.id` equals `${env:MSTDN_SEED_REPLY}`
- after `HTTP POST /api/v1/accounts/${env:MSTDN_SEED_ALICE}/unblock`:
  - HTTP status is `200`
- after `mstdn --json status ${env:MSTDN_SEED_REPLY}`:
  - exit code is `0`
  - stdout at `$.id` equals `${env:MSTDN_SEED_REPLY}`

### Scenario: usable: search accounts in the full-screen client
#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`

### Scenario: heavy: Unicode text name and alt text survive JSON
#### When

```shell
mstdn --json status ${env:MSTDN_SEED_UNICODE}
# capture ${text} from stdout
```

#### Then
- exit code is `0`
- stdout contains `👨‍👩‍👧`, `👍🏽`, `🇯🇵`, `✈️`, `漢字`, `العربية`, `é`, `longwordlongword`
- stdout at `$.files[0].type` equals `image`
- stdout at `$.files[0].description` equals `${text}`; at `$.user.name` equals `Alice 👩🏽‍💻 🇯🇵 漢字 é`

### Scenario: heavy: published video byte limit and one over
#### When

```shell
mstdn --json post "maximum video bytes" --video "${env:MSTDN_COMPAT_SEED}/max-video.mp4"
# capture ${created} from stdout
mstdn --json status ${created}
mstdn --json post "over video bytes" --video "${env:MSTDN_COMPAT_SEED}/over-video.mp4"
```

#### Then
- after `mstdn --json post "maximum video bytes" --video "${env:MSTDN_COMPAT_SEED}/max-video.mp4"`:
  - exit code is `0`
  - stdout does not contain `"error"`
- after `mstdn --json status ${created}`:
  - exit code is `0`
  - stdout at `$.files[0].type` equals `video`
- after `mstdn --json post "over video bytes" --video "${env:MSTDN_COMPAT_SEED}/over-video.mp4"`:
  - exit code is `1`
  - stdout contains `${env:MSTDN_LIMIT_VIDEO_MESSAGE}`, `${env:MSTDN_LIMIT_VIDEO}`

### Scenario: usable: unsupported quote policy preserves the draft and permits recovery
_only when env MSTDN_NO_QUOTE_POLICY is set_
#### When

```shell
# interactive (pty): mstdn
```

#### Then
- exit code is `0`
