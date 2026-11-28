# atago Behavior Specs
## Summary
1 suite · 20 scenarios
## Contents
- [bsky viewer](#bsky-viewer): 20 scenarios
  - [space shows a post's pictures full screen, one after another, and esc goes back](#scenario-space-shows-a-posts-pictures-full-screen-one-after-another-and-esc-goes-back)
  - [d saves a picture as what it is and refuses what is not a picture](#scenario-d-saves-a-picture-as-what-it-is-and-refuses-what-is-not-a-picture)
  - [by default selection switches inline videos and restores their thumbnails](#scenario-by-default-selection-switches-inline-videos-and-restores-their-thumbnails)
  - [timeline playback setting takes effect immediately and survives restart](#scenario-timeline-playback-setting-takes-effect-immediately-and-survives-restart)
  - [timeline playback has a Japanese settings label](#scenario-timeline-playback-has-a-japanese-settings-label)
  - [BSKY_INLINE_VIDEO=0 overrides the saved timeline playback setting](#scenario-bsky_inline_video0-overrides-the-saved-timeline-playback-setting)
  - [BSKY_INLINE_VIDEO=1 overrides the saved timeline playback setting](#scenario-bsky_inline_video1-overrides-the-saved-timeline-playback-setting)
  - [space plays a post's video, r plays it again, and esc stops it](#scenario-space-plays-a-posts-video-r-plays-it-again-and-esc-stops-it)
  - [inline video read-ahead removes all three network waits on selection](#scenario-inline-video-read-ahead-removes-all-three-network-waits-on-selection)
  - [inline video read-ahead is disabled with the flag off](#scenario-inline-video-read-ahead-is-disabled-with-the-flag-off)
  - [inline video read-ahead is disabled with pictures off](#scenario-inline-video-read-ahead-is-disabled-with-pictures-off)
  - [a video plays when its playlist names files the scheme-relative and dot-segment way](#scenario-a-video-plays-when-its-playlist-names-files-the-scheme-relative-and-dot-segment-way)
  - [a video bsky cannot play is a warning, not an error, and its thumbnail stays](#scenario-a-video-bsky-cannot-play-is-a-warning-not-an-error-and-its-thumbnail-stays)
  - [space on a post with only a link opens it in the browser, and o opens a link anywhere](#scenario-space-on-a-post-with-only-a-link-opens-it-in-the-browser-and-o-opens-a-link-anywhere)
  - [on Windows, space and o open links with the program BSKY_BROWSER names](#scenario-on-windows-space-and-o-open-links-with-the-program-bsky_browser-names)
  - [without a program to open links, it says what to set](#scenario-without-a-program-to-open-links-it-says-what-to-set)
  - [a browser typed on the settings screen opens the links](#scenario-a-browser-typed-on-the-settings-screen-opens-the-links)
  - [without BSKY_DOWNLOAD_DIR a picture is saved under the home download folder](#scenario-without-bsky_download_dir-a-picture-is-saved-under-the-home-download-folder)
  - [a download folder chosen on the settings screen is where d saves](#scenario-a-download-folder-chosen-on-the-settings-screen-is-where-d-saves)
  - [a folder made in the folder chooser becomes the download folder](#scenario-a-folder-made-in-the-folder-chooser-becomes-the-download-folder)

## bsky viewer
Source: `crates/bluesky-terminal-client/e2e/atago/viewer.atago.yaml`
### Scenario: space shows a post's pictures full screen, one after another, and esc goes back
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
- mock `api` received `/img/full1.png` exactly 1 time(s)
- mock `api` received `/img/full2.png` exactly 2 time(s)
- dir `downloads` contains `full2.png`, has 1 entry
- image `downloads/full2.png` width 100, height 100

#### Generated artifacts
- `downloads/full2.png`

### Scenario: d saves a picture as what it is and refuses what is not a picture
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
- dir `downloads` contains `setup.png`, has 1 entry
- image `downloads/setup.png` width 100, height 100

#### Generated artifacts
- `downloads/setup.png`

### Scenario: by default selection switches inline videos and restores their thumbnails
#### Given
- Stub HTTP server `api` serves 11 canned route(s) at `${api.url}` and records every request (#24).
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
- mock `api` received `/watch/v1/v/seg0.ts` exactly 1 time(s)
- mock `api` received `/watch/v2/v/seg0.ts` exactly 1 time(s)

### Scenario: timeline playback setting takes effect immediately and survives restart
#### Given
- Stub HTTP server `api` serves 11 canned route(s) at `${api.url}` and records every request (#24).
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
  - file `cfg/settings.json` at `$.inline_video` equals `false`
- after `interactive (pty): bsky`:
  - exit code is `0`
  - mock `api` received `/watch/v1/playlist.m3u8` exactly 1 time(s)
  - mock `api` received `/watch/v2/playlist.m3u8` exactly 1 time(s)

### Scenario: timeline playback has a Japanese settings label
#### Given
- Stub HTTP server `api` serves 11 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/settings.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/settings.json`:_

```text
{"inline_video":false,"language":"ja"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `api` received `/watch/v1/playlist.m3u8` exactly 0 time(s)

### Scenario: BSKY_INLINE_VIDEO=0 overrides the saved timeline playback setting
#### Given
- Stub HTTP server `api` serves 11 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/settings.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/settings.json`:_

```text
{"inline_video":true}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- file `cfg/settings.json` at `$.inline_video` equals `true`
- mock `api` received `/watch/v1/playlist.m3u8` exactly 0 time(s)

### Scenario: BSKY_INLINE_VIDEO=1 overrides the saved timeline playback setting
#### Given
- Stub HTTP server `api` serves 11 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/settings.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/settings.json`:_

```text
{"inline_video":false}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- file `cfg/settings.json` at `$.inline_video` equals `false`
- mock `api` received `/watch/v1/playlist.m3u8` exactly 1 time(s)

### Scenario: space plays a post's video, r plays it again, and esc stops it
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
- mock `api` received `/watch/v1/playlist.m3u8` exactly 3 time(s)
- mock `api` received `/watch/v1/v/seg0.ts` exactly 3 time(s)
- dir `downloads` contains `v1.ts`, has 1 entry
- mock `api` received `/watch/v1/v/video.m3u8`

### Scenario: inline video read-ahead removes all three network waits on selection
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
- mock `api` received `/watch/v3/playlist.m3u8` exactly 1 time(s)
- mock `api` received `/watch/v3/v/video.m3u8` exactly 1 time(s)
- mock `api` received `/watch/v3/v/seg0.ts` exactly 1 time(s)

### Scenario: inline video read-ahead is disabled with the flag off
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/settings.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/settings.json`:_

```text
{"pictures":"auto"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `api` received `/watch/v3/playlist.m3u8` exactly 0 time(s)
- mock `api` received `/watch/v3/v/video.m3u8` exactly 0 time(s)
- mock `api` received `/watch/v3/v/seg0.ts` exactly 0 time(s)

### Scenario: inline video read-ahead is disabled with pictures off
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/settings.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/settings.json`:_

```text
{"pictures":"off"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `api` received `/watch/v3/playlist.m3u8` exactly 0 time(s)
- mock `api` received `/watch/v3/v/video.m3u8` exactly 0 time(s)
- mock `api` received `/watch/v3/v/seg0.ts` exactly 0 time(s)

### Scenario: a video plays when its playlist names files the scheme-relative and dot-segment way
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
- mock `api` received `/watch/v2/playlist.m3u8` exactly 1 time(s)
- mock `api` received `/watch/v2/v/video.m3u8` exactly 1 time(s)
- mock `api` received `/watch/v2/v/seg0.ts` exactly 1 time(s)

### Scenario: a video bsky cannot play is a warning, not an error, and its thumbnail stays
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

### Scenario: space on a post with only a link opens it in the browser, and o opens a link anywhere
_skipped on Windows_
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `opener.sh` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `opener.sh`:_

```text
#!/bin/sh
printf '%s\n' "$@" > "$(dirname "$0")/opened.fifo"
```

#### When

```shell
mkfifo opened.fifo
# interactive (pty): bsky
```

#### Then
- after `interactive (pty): bsky`:
  - exit code is `0`
  - file `opened.txt` contains `"https://example.com/article?a=1&b=2\nhttps://example.com/photo-page\n"`

### Scenario: on Windows, space and o open links with the program BSKY_BROWSER names
_only on Windows_
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `opener.cmd` is created.
- The step is retried up to 50 times every 200ms until exit code is `0`.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `opener.cmd`:_

```text
@echo off
setlocal enabledelayedexpansion
rem Delayed expansion keeps cmd from reading the & in a URL.
set "url=%~1"
>"%~dp0opened-!url:~20,5!.txt" echo(!url!
```

#### When

```shell
# interactive (pty): bsky
cmd /c type opened-artic.txt opened-photo.txt
```

#### Then
- after `interactive (pty): bsky`:
  - exit code is `0`
- after `cmd /c type opened-artic.txt opened-photo.txt`:
  - file `opened-artic.txt` equals exact bytes
  - file `opened-photo.txt` equals exact bytes

### Scenario: without a program to open links, it says what to set
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

### Scenario: a browser typed on the settings screen opens the links
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
- file `cfg/settings.json` at `$.browser` equals `ブラウザ🦊`

### Scenario: without BSKY_DOWNLOAD_DIR a picture is saved under the home download folder
_skipped on Windows_
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
- dir `home/Downloads/bsky` contains `full.png`, has 1 entry
- image `home/Downloads/bsky/full.png` width 100, height 100

#### Generated artifacts
- `home/Downloads/bsky/full.png`

### Scenario: a download folder chosen on the settings screen is where d saves
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/settings.json` is created.
- Fixture file `first/keep.txt` is created.
- Fixture file `first-写真👨‍👩‍👧/keep.txt` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/settings.json`:_

```text
{"download_dir": "first", "theme": "nord"}
```

_Fixture `first/keep.txt`:_

```text
the folder settings.json names
```

_Fixture `first-写真👨‍👩‍👧/keep.txt`:_

```text
the folder to choose
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- dir `first-写真👨‍👩‍👧` contains `full.png`
- image `first-写真👨‍👩‍👧/full.png` width 100, height 100
- dir `first` has 1 entry
- file `cfg/settings.json` at `$.download_dir` matches `/first-写真👨‍👩‍👧$/`; at `$.theme` equals `nord`

#### Generated artifacts
- `first-写真👨‍👩‍👧/full.png`

### Scenario: a folder made in the folder chooser becomes the download folder
#### Given
- Stub HTTP server `api` serves 5 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `cfg/settings.json` is created.
- Fixture file `saved/keep.txt` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

_Fixture `cfg/settings.json`:_

```text
{"download_dir": "saved"}
```

_Fixture `saved/keep.txt`:_

```text
the folder settings.json names
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- dir `saved/写真 🏔️👨‍👩‍👧` contains `full.png`, has 1 entry
- dir `saved` has 2 entries
- file `cfg/settings.json` at `$.download_dir` matches `/saved[/\\]写真 🏔️👨‍👩‍👧$/`
