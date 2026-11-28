# atago Behavior Specs
## Summary
1 suite · 48 scenarios
## Contents
- [msky live catalog](#msky-live-catalog): 48 scenarios
  - [usable: token identifies the account](#scenario-usable-token-identifies-the-account)
  - [usable: home timeline reads seeded notes](#scenario-usable-home-timeline-reads-seeded-notes)
  - [usable: local timeline reads seeded notes](#scenario-usable-local-timeline-reads-seeded-notes)
  - [usable: global timeline reads seeded notes](#scenario-usable-global-timeline-reads-seeded-notes)
  - [usable: three pages of seeded text notes](#scenario-usable-three-pages-of-seeded-text-notes)
  - [usable: post reply thread and delete](#scenario-usable-post-reply-thread-and-delete)
  - [usable: reactions custom emoji and renote removal](#scenario-usable-reactions-custom-emoji-and-renote-removal)
  - [usable: renote count returns to zero](#scenario-usable-renote-count-returns-to-zero)
  - [usable: known stale renote count from the server](#scenario-usable-known-stale-renote-count-from-the-server)
  - [usable: upload picture with alt text](#scenario-usable-upload-picture-with-alt-text)
  - [usable: upload video with alt text](#scenario-usable-upload-video-with-alt-text)
  - [usable: content warning and public visibility](#scenario-usable-content-warning-and-public-visibility)
  - [usable: content warning and home visibility](#scenario-usable-content-warning-and-home-visibility)
  - [usable: content warning and followers visibility](#scenario-usable-content-warning-and-followers-visibility)
  - [usable: read specified visibility seeded through API](#scenario-usable-read-specified-visibility-seeded-through-api)
  - [usable: notifications include seeded interactions](#scenario-usable-notifications-include-seeded-interactions)
  - [usable: download preserves the picture](#scenario-usable-download-preserves-the-picture)
  - [usable: kitty opens thread pictures video searches accounts composes and quits](#scenario-usable-kitty-opens-thread-pictures-video-searches-accounts-composes-and-quits)
  - [heavy: video author timeline paged to its end](#scenario-heavy-video-author-timeline-paged-to-its-end)
  - [heavy: pictures author timeline paged to its end](#scenario-heavy-pictures-author-timeline-paged-to-its-end)
  - [heavy: fifty deep replies](#scenario-heavy-fifty-deep-replies)
  - [heavy: two hundred direct replies](#scenario-heavy-two-hundred-direct-replies)
  - [heavy: at least five hundred notifications paged to the end](#scenario-heavy-at-least-five-hundred-notifications-paged-to-the-end)
  - [heavy: scroll one hundred video posts down and back](#scenario-heavy-scroll-one-hundred-video-posts-down-and-back)
  - [heavy: scroll one hundred picture posts down and back](#scenario-heavy-scroll-one-hundred-picture-posts-down-and-back)
  - [heavy: unsupported codec offers saving](#scenario-heavy-unsupported-codec-offers-saving)
  - [heavy: unicode text keeps the hint row](#scenario-heavy-unicode-text-keeps-the-hint-row)
  - [speed: read 100 timeline notes under 10s](#scenario-speed-read-100-timeline-notes-under-10s)
  - [speed: read 100 notifications under 10s](#scenario-speed-read-100-notifications-under-10s)
  - [speed: search 100 hashtag notes under 10s](#scenario-speed-search-100-hashtag-notes-under-10s)
  - [speed: publish a picture under 15s](#scenario-speed-publish-a-picture-under-15s)
  - [speed: first populated timeline session](#scenario-speed-first-populated-timeline-session)
  - [speed: first picture session](#scenario-speed-first-picture-session)
  - [speed: short video playback session](#scenario-speed-short-video-playback-session)
  - [behavior: refresh preserves the seeded search result](#scenario-behavior-refresh-preserves-the-seeded-search-result)
  - [behavior: wrong token remains authentication status four](#scenario-behavior-wrong-token-remains-authentication-status-four)
  - [behavior: missing permission is not unsupported](#scenario-behavior-missing-permission-is-not-unsupported)
  - [behavior: deleted post](#scenario-behavior-deleted-post)
  - [behavior: blocked followers-only post is not exposed](#scenario-behavior-blocked-followers-only-post-is-not-exposed)
  - [usable: gap: JSON Lines unique ordered ID set across pages](#scenario-usable-gap-json-lines-unique-ordered-id-set-across-pages)
  - [usable: gap: specified visibility writes have no recipient command flag](#scenario-usable-gap-specified-visibility-writes-have-no-recipient-command-flag)
  - [heavy: gap: each visible media post has its own thumbnail](#scenario-heavy-gap-each-visible-media-post-has-its-own-thumbnail)
  - [heavy: gap: inline timeline video playback is not implemented](#scenario-heavy-gap-inline-timeline-video-playback-is-not-implemented)
  - [heavy: gap: grapheme cell spill detection needs screen geometry predicates](#scenario-heavy-gap-grapheme-cell-spill-detection-needs-screen-geometry-predicates)
  - [speed: gap: per-expect latency and distinct video frame measurements](#scenario-speed-gap-per-expect-latency-and-distinct-video-frame-measurements)
  - [behavior: gap: real-server request tracing for writes sent exactly once](#scenario-behavior-gap-real-server-request-tracing-for-writes-sent-exactly-once)
  - [behavior: gap: federation requires more than one server](#scenario-behavior-gap-federation-requires-more-than-one-server)
  - [behavior: gap: video processing readiness cannot be forced by these upload APIs](#scenario-behavior-gap-video-processing-readiness-cannot-be-forced-by-these-upload-apis)

## msky live catalog
Source: `crates/misskey-terminal-client/e2e/compat/catalog.atago.yaml`

Secrets declared: `${env:MSKY_TOKEN}`, `${env:MSKY_SECOND_TOKEN}`, `${env:MSKY_THIRD_TOKEN}`, `${env:MSKY_ADMIN_TOKEN}`, `${env:MSKY_RESTRICTED_TOKEN}`.
### Scenario: usable: token identifies the account
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky whoami --json
```

#### Then
- exit code is `0`
- stdout at `$.username` equals `compat_admin`

### Scenario: usable: home timeline reads seeded notes
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky timeline home -n 100 --json
```

#### Then
- exit code is `0`
- stdout matches `/(?m)^\{/` exactly 100 times
- stdout contains `compat-`

### Scenario: usable: local timeline reads seeded notes
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky timeline local -n 100 --json
```

#### Then
- exit code is `0`
- stdout matches `/(?m)^\{/` exactly 100 times
- stdout contains `compat-`

### Scenario: usable: global timeline reads seeded notes
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky timeline global -n 100 --json
```

#### Then
- exit code is `0`
- stdout matches `/(?m)^\{/` exactly 100 times
- stdout contains `compat-`

### Scenario: usable: three pages of seeded text notes
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky search "#compatcatalog" -n 1000 --json
```

#### Then
- exit code is `0`
- stdout matches `/(?m)^\{/` exactly 500 times
- stdout contains `compat-text-499`, `compat-text-000`

### Scenario: usable: post reply thread and delete
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post "compat-native-post" --json
# capture ${note} from stdout
msky post "compat-native-reply" --reply ${note} --json
# capture ${reply} from stdout
msky thread ${note} --json
msky delete ${reply} --json
msky delete ${note} --json
msky note ${note} --json
```

#### Then
- after `msky post "compat-native-post" --json`:
  - exit code is `0`
  - stdout at `$.action` equals `post`
- after `msky thread ${note} --json`:
  - exit code is `0`
  - stdout matches `/(?m)^\{/` exactly 2 times
  - stdout contains `${reply}`, `${note}`, `compat-native-reply`
- after `msky delete ${reply} --json`:
  - exit code is `0`
  - stdout at `$.action` equals `delete`
- after `msky delete ${note} --json`:
  - exit code is `0`
  - stdout at `$.action` equals `delete`
- after `msky note ${note} --json`:
  - exit code is `4`
  - stdout at `$.error` matches `/NO_SUCH_NOTE/`; at `$.status` equals `4`

### Scenario: usable: reactions custom emoji and renote removal
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The step is retried up to 20 times every 250ms until exit code is `0` and stdout is empty.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post "compat-native-react" --json
# capture ${note} from stdout
msky like ${note} --reaction 👍 --json
msky note ${note} --json
msky reactions ${note} --json
msky unlike ${note} --json
msky note ${note} --json
msky like ${note} --reaction :compat: --json
msky reactions ${note} --json
msky repost ${note} --json
msky note ${note} --json
msky unrepost ${note} --json
msky renotes ${note} --json
```

#### Then
- after `msky note ${note} --json`:
  - exit code is `0`
  - stdout at `$.likeCount` equals `1`
- after `msky reactions ${note} --json`:
  - exit code is `0`
  - stdout matches `/(?m)^\{/` exactly 1 time
- after `msky note ${note} --json`:
  - exit code is `0`
  - stdout at `$.likeCount` equals `0`
- after `msky reactions ${note} --json`:
  - exit code is `0`
  - stdout at `$.reaction` equals `:compat@.:`
- after `msky note ${note} --json`:
  - exit code is `0`
  - stdout at `$.repostCount` equals `1`
- after `msky renotes ${note} --json`:
  - exit code is `0`
  - stdout is empty

### Scenario: usable: renote count returns to zero
_skipped when env MSKY_STALE_RENOTE is set_
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky note ${env:MSKY_SEED_RENOTE_TARGET} --json
```

#### Then
- exit code is `0`
- stdout at `$.repostCount` equals `0`

### Scenario: usable: known stale renote count from the server
_only when env MSKY_STALE_RENOTE is set · expected to FAIL (known bug): The server removed the renote but retained renoteCount; see COMPAT.md._
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky note ${env:MSKY_SEED_RENOTE_TARGET} --json
```

#### Then
- exit code is `0`
- stdout at `$.repostCount` equals `0`

### Scenario: usable: upload picture with alt text
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post "compat-upload-picture" --image ${env:MSKY_SEED_PICTURE_PATH} --alt "compat alt 👨‍👩‍👧 👍🏽 🇯🇵 漢字 العربية é" --json
# capture ${note} from stdout
msky note ${note} --json
```

#### Then
- after `msky note ${note} --json`:
  - exit code is `0`
  - stdout at `$.files[0].type` equals `image`; at `$.files[0].description` equals `compat alt 👨‍👩‍👧 👍🏽 🇯🇵 漢字 العربية é`

### Scenario: usable: upload video with alt text
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post "compat-upload-video" --video ${env:MSKY_SEED_VIDEO_PATH} --alt "compat alt 👨‍👩‍👧 👍🏽 🇯🇵 漢字 العربية é" --json
# capture ${note} from stdout
msky note ${note} --json
```

#### Then
- after `msky note ${note} --json`:
  - exit code is `0`
  - stdout at `$.files[0].type` equals `video`; at `$.files[0].description` equals `compat alt 👨‍👩‍👧 👍🏽 🇯🇵 漢字 العربية é`

### Scenario: usable: content warning and public visibility
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post "compat-warning" --cw "compat warning" --visibility public --json
# capture ${note} from stdout
msky note ${note} --json
```

#### Then
- after `msky note ${note} --json`:
  - exit code is `0`
  - stdout at `$.cw` equals `compat warning`; at `$.visibility` equals `public`

### Scenario: usable: content warning and home visibility
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post "compat-warning" --cw "compat warning" --visibility home --json
# capture ${note} from stdout
msky note ${note} --json
```

#### Then
- after `msky note ${note} --json`:
  - exit code is `0`
  - stdout at `$.cw` equals `compat warning`; at `$.visibility` equals `home`

### Scenario: usable: content warning and followers visibility
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post "compat-warning" --cw "compat warning" --visibility followers --json
# capture ${note} from stdout
msky note ${note} --json
```

#### Then
- after `msky note ${note} --json`:
  - exit code is `0`
  - stdout at `$.cw` equals `compat warning`; at `$.visibility` equals `followers`

### Scenario: usable: read specified visibility seeded through API
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky note ${env:MSKY_SEED_SPECIFIED} --json
```

#### Then
- exit code is `0`
- stdout at `$.visibility` equals `specified`

### Scenario: usable: notifications include seeded interactions
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky notifications -n 2000 --json
```

#### Then
- exit code is `0`
- stdout contains `"type":"mention"`, `"type":"reply"`, `"type":"like"`, `"type":"repost"`, `"type":"follow"`

### Scenario: usable: download preserves the picture
#### Given
- Fixture file `expected.png` is created.
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky download ${env:MSKY_SEED_PICTURE_FIRST} -o downloaded --json
```

#### Then
- exit code is `0`
- stdout matches `/(?m)^\{/` exactly 4 times
- dir `downloaded` has 4 entries
- file `downloaded/${env:MSKY_SEED_PICTURE_FIRST}-1.png` is byte-identical to `expected.png`
- file `downloaded/${env:MSKY_SEED_PICTURE_FIRST}-2.png` is byte-identical to `expected.png`
- file `downloaded/${env:MSKY_SEED_PICTURE_FIRST}-3.png` is byte-identical to `expected.png`
- file `downloaded/${env:MSKY_SEED_PICTURE_FIRST}-4.png` is byte-identical to `expected.png`

### Scenario: usable: kitty opens thread pictures video searches accounts composes and quits
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`

### Scenario: heavy: video author timeline paged to its end
#### Given
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky author compat_user --filter video -n 1000 --json
```

#### Then
- exit code is `0`
- stdout matches `/(?m)^\{/` exactly 101 times

### Scenario: heavy: pictures author timeline paged to its end
#### Given
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky author compat_pictures --filter media -n 1000 --json
```

#### Then
- exit code is `0`
- stdout matches `/(?m)^\{/` exactly 101 times

### Scenario: heavy: fifty deep replies
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky thread ${env:MSKY_SEED_DEEP_LEAF} --json
```

#### Then
- exit code is `0`
- stdout matches `/(?m)^\{/` exactly 51 times
- stdout contains `compat-deep-root`, `compat-deep-049`

### Scenario: heavy: two hundred direct replies
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky thread ${env:MSKY_SEED_WIDE_ROOT} --json
```

#### Then
- exit code is `0`
- stdout matches `/(?m)^\{/` exactly 201 times
- stdout contains `compat-wide-000`, `compat-wide-199`

### Scenario: heavy: at least five hundred notifications paged to the end
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky notifications -n 2000 --json
```

#### Then
- exit code is `0`
- stdout matches `/(?m)^\{/` at least 500 times
- stdout contains `"type":"follow"`

### Scenario: heavy: scroll one hundred video posts down and back
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`

### Scenario: heavy: scroll one hundred picture posts down and back
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`

### Scenario: heavy: unsupported codec offers saving
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`

### Scenario: heavy: unicode text keeps the hint row
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`

### Scenario: speed: read 100 timeline notes under 10s
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky timeline -n 100 --json
```

#### Then
- exit code is `0`
- completes in under 10s

### Scenario: speed: read 100 notifications under 10s
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky notifications -n 100 --json
```

#### Then
- exit code is `0`
- completes in under 10s

### Scenario: speed: search 100 hashtag notes under 10s
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky search "#compatcatalog" -n 100 --json
```

#### Then
- exit code is `0`
- completes in under 10s

### Scenario: speed: publish a picture under 15s
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post "compat-speed-picture" --image ${env:MSKY_SEED_PICTURE_PATH} --json
```

#### Then
- exit code is `0`
- completes in under 15s

### Scenario: speed: first populated timeline session
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`

### Scenario: speed: first picture session
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`

### Scenario: speed: short video playback session
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`

### Scenario: behavior: refresh preserves the seeded search result
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky search "#compatcatalog" -n 1000 --json
```

#### Then
- exit code is `0`
- stdout matches `/(?m)^\{/` exactly 500 times

### Scenario: behavior: wrong token remains authentication status four
#### Given
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky whoami --json
```

#### Then
- exit code is `4`
- stdout at `$.status` equals `4`; at `$.error` matches `/AUTHENTICATION_FAILED/`

### Scenario: behavior: missing permission is not unsupported
#### Given
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky chat --json
```

#### Then
- exit code is `4`
- stdout at `$.status` equals `4`; at `$.error` matches `/PERMISSION_DENIED/`

### Scenario: behavior: deleted post
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky note ${env:MSKY_SEED_DELETED} --json
```

#### Then
- exit code is `4`
- stdout contains `NO_SUCH_NOTE`

### Scenario: behavior: blocked followers-only post is not exposed
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky note ${env:MSKY_SEED_BLOCKED} --json
```

#### Then
- exit code is `4`

### Scenario: usable: gap: JSON Lines unique ordered ID set across pages
_skipped when env MSKY_SKIP_GAP is set_
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky whoami --json
```

#### Then
- exit code is `0`

### Scenario: usable: gap: specified visibility writes have no recipient command flag
_skipped when env MSKY_SKIP_GAP is set_
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky whoami --json
```

#### Then
- exit code is `0`

### Scenario: heavy: gap: each visible media post has its own thumbnail
_skipped when env MSKY_SKIP_GAP is set_
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky whoami --json
```

#### Then
- exit code is `0`

### Scenario: heavy: gap: inline timeline video playback is not implemented
_skipped when env MSKY_SKIP_GAP is set_
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky whoami --json
```

#### Then
- exit code is `0`

### Scenario: heavy: gap: grapheme cell spill detection needs screen geometry predicates
_skipped when env MSKY_SKIP_GAP is set_
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky whoami --json
```

#### Then
- exit code is `0`

### Scenario: speed: gap: per-expect latency and distinct video frame measurements
_skipped when env MSKY_SKIP_GAP is set_
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky whoami --json
```

#### Then
- exit code is `0`

### Scenario: behavior: gap: real-server request tracing for writes sent exactly once
_skipped when env MSKY_SKIP_GAP is set_
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky whoami --json
```

#### Then
- exit code is `0`

### Scenario: behavior: gap: federation requires more than one server
_skipped when env MSKY_SKIP_GAP is set_
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky whoami --json
```

#### Then
- exit code is `0`

### Scenario: behavior: gap: video processing readiness cannot be forced by these upload APIs
_skipped when env MSKY_SKIP_GAP is set_
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky whoami --json
```

#### Then
- exit code is `0`
