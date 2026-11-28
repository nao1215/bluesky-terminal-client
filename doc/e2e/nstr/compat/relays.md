# atago Behavior Specs
## Summary
1 suite · 15 scenarios
## Contents
- [nstr live relay catalog](#nstr-live-relay-catalog): 15 scenarios
  - [usable: generated identity and ordered contact timeline](#scenario-usable-generated-identity-and-ordered-contact-timeline)
  - [behavior: repeated multi-page reads are byte-identical](#scenario-behavior-repeated-multi-page-reads-are-byte-identical)
  - [heavy: 806 notifications including 802 mentions reach the end](#scenario-heavy-806-notifications-including-802-mentions-reach-the-end)
  - [heavy: fifty-deep ancestry and two hundred direct replies are stored](#scenario-heavy-fifty-deep-ancestry-and-two-hundred-direct-replies-are-stored)
  - [usable: content-warning and one to four pictures with imeta alt text survive reads](#scenario-usable-content-warning-and-one-to-four-pictures-with-imeta-alt-text-survive-reads)
  - [usable: unadvertised text search is unsupported and hashtag search still works](#scenario-usable-unadvertised-text-search-is-unsupported-and-hashtag-search-still-works)
  - [heavy: picture profile navigation graphics and Unicode text](#scenario-heavy-picture-profile-navigation-graphics-and-unicode-text)
  - [heavy: video profile stays responsive and plays supported media](#scenario-heavy-video-profile-stays-responsive-and-plays-supported-media)
  - [usable: full-screen thread reaction repost reply and composition](#scenario-usable-full-screen-thread-reaction-repost-reply-and-composition)
  - [behavior: published reactions and reposts are readable by the target](#scenario-behavior-published-reactions-and-reposts-are-readable-by-the-target)
  - [speed: read 100 timeline notes under 10s](#scenario-speed-read-100-timeline-notes-under-10s)
  - [speed: read 100 notifications under 10s](#scenario-speed-read-100-notifications-under-10s)
  - [speed: search 100 hashtag notes under 10s](#scenario-speed-search-100-hashtag-notes-under-10s)
  - [speed: publish a picture URL under 10s](#scenario-speed-publish-a-picture-url-under-10s)
  - [speed: first paint and first picture session](#scenario-speed-first-paint-and-first-picture-session)

## nstr live relay catalog
Source: `crates/nostr-terminal-client/e2e/compat/relays.atago.yaml`
### Scenario: usable: generated identity and ordered contact timeline
#### Given
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr whoami --json
nstr timeline -n 1 --json
nstr search "#catalog" -n 2000 --json
```

#### Then
- after `nstr whoami --json`:
  - exit code is `0`
  - stdout at `$.id` equals `${env:NSTR_COMPAT_PUBLIC}`; at `$.name` equals `compat-user`
- after `nstr timeline -n 1 --json`:
  - exit code is `0`
  - stdout at `$.text` equals `compat reply`
- after `nstr search "#catalog" -n 2000 --json`:
  - exit code is `0`
  - stdout matches `/(?m)^\{/` exactly 1056 times
  - stdout contains `"text":"text 000 `, `"text":"video 099 `, `"text":"pictures 099"`

### Scenario: behavior: repeated multi-page reads are byte-identical
#### Given
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr timeline -n 350 --json
# capture ${first} from stdout
nstr timeline -n 350 --json
```

#### Then
- after `nstr timeline -n 350 --json`:
  - exit code is `0`
  - stdout matches `/(?m)^\{/` exactly 350 times
- after `nstr timeline -n 350 --json`:
  - exit code is `0`
  - stdout equals an exact value

### Scenario: heavy: 806 notifications including 802 mentions reach the end
#### Given
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr notifications -n 2000 --json
```

#### Then
- exit code is `0`
- stdout matches `/(?m)^\{/` exactly 806 times
- stdout contains `"type":"like"`, `"type":"repost"`, `"type":"follow"`, `"type":"reply"`, `"type":"mention"`

### Scenario: heavy: fifty-deep ancestry and two hundred direct replies are stored
#### Given
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr search "#deep" -n 100 --json
nstr search "#wide" -n 300 --json
```

#### Then
- after `nstr search "#deep" -n 100 --json`:
  - exit code is `0`
  - stdout matches `/(?m)^\{/` exactly 51 times
  - stdout contains `"text":"depth 000"`, `"text":"depth 050"`, `"reply"`, `"root"`
- after `nstr search "#wide" -n 300 --json`:
  - exit code is `0`
  - stdout matches `/(?m)^\{/` exactly 201 times

### Scenario: usable: content-warning and one to four pictures with imeta alt text survive reads
#### Given
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr search "#cw" -n 1 --json
nstr search "#media-1" -n 1 --json
nstr search "#media-2" -n 1 --json
nstr search "#media-3" -n 1 --json
nstr search "#pictures" -n 1 --json
```

#### Then
- after `nstr search "#cw" -n 1 --json`:
  - exit code is `0`
  - stdout at `$.cw` equals `spoilers`
- after `nstr search "#media-1" -n 1 --json`:
  - exit code is `0`
  - stdout at `$.tags[0][1]` equals `url ${env:NSTR_COMPAT_MEDIA}/picture-0.png`; at `$.tags[1][0]` equals `t`
- after `nstr search "#media-2" -n 1 --json`:
  - exit code is `0`
  - stdout at `$.tags[1][1]` equals `url ${env:NSTR_COMPAT_MEDIA}/picture-1.png`; at `$.tags[2][0]` equals `t`
- after `nstr search "#media-3" -n 1 --json`:
  - exit code is `0`
  - stdout at `$.tags[2][1]` equals `url ${env:NSTR_COMPAT_MEDIA}/picture-2.png`; at `$.tags[3][0]` equals `t`
- after `nstr search "#pictures" -n 1 --json`:
  - exit code is `0`
  - stdout at `$.tags[0][1]` equals `url ${env:NSTR_COMPAT_MEDIA}/picture-0.png`; at `$.tags[3][1]` equals `url ${env:NSTR_COMPAT_MEDIA}/picture-3.png`; at `$.tags[0][3]` matches `/^alt CJK 日本語/`

### Scenario: usable: unadvertised text search is unsupported and hashtag search still works
#### Given
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr search words --json
nstr search "#video" -n 200 --json
```

#### Then
- after `nstr search words --json`:
  - exit code is `5`
  - stdout at `$.status` equals `5`
  - stderr contains `unsupported: NIP-50 search`
- after `nstr search "#video" -n 200 --json`:
  - exit code is `0`
  - stdout matches `/(?m)^\{/` exactly 100 times
  - stdout contains `pattern.mp4`, `unsupported.webm`, `"m video/mp4"`, `"m video/webm"`

### Scenario: heavy: picture profile navigation graphics and Unicode text
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): nstr
```

#### Then
- exit code is `0`

### Scenario: heavy: video profile stays responsive and plays supported media
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): nstr
```

#### Then
- exit code is `0`

### Scenario: usable: full-screen thread reaction repost reply and composition
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): nstr
nstr timeline -n 10 --json
```

#### Then
- after `interactive (pty): nstr`:
  - exit code is `0`
- after `nstr timeline -n 10 --json`:
  - exit code is `0`
  - stdout contains `catalog live reply`, `catalog live composition`

### Scenario: behavior: published reactions and reposts are readable by the target
#### Given
- Environment variables are set: LC_ALL, NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr notifications -n 100 --json
```

#### Then
- exit code is `0`
- stdout contains `"type":"like"`, `"type":"repost"`, `"text":"catalog live reply"`

### Scenario: speed: read 100 timeline notes under 10s
#### Given
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr timeline -n 100 --json
```

#### Then
- exit code is `0`
- stdout matches `/(?m)^\{/` exactly 100 times
- completes in under 10s

### Scenario: speed: read 100 notifications under 10s
#### Given
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr notifications -n 100 --json
```

#### Then
- exit code is `0`
- stdout matches `/(?m)^\{/` exactly 100 times
- completes in under 10s

### Scenario: speed: search 100 hashtag notes under 10s
#### Given
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr search "#video" -n 100 --json
```

#### Then
- exit code is `0`
- stdout matches `/(?m)^\{/` exactly 100 times
- completes in under 10s

### Scenario: speed: publish a picture URL under 10s
#### Given
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr post "catalog picture ${env:NSTR_COMPAT_MEDIA}/picture-0.png" --json
```

#### Then
- exit code is `0`
- stdout at `$.action` equals `post`
- completes in under 10s
- stdout contains `"acceptedRelays":${env:NSTR_COMPAT_COUNT}`

### Scenario: speed: first paint and first picture session
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): nstr
```

#### Then
- exit code is `0`
- completes in under 20s
