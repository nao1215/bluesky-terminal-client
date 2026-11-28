# atago Behavior Specs
## Summary
1 suite · 3 scenarios
## Contents
- [mstdn media](#mstdn-media): 3 scenarios
  - [pictures and video are drawn and original files are saved without overwriting](#scenario-pictures-and-video-are-drawn-and-original-files-are-saved-without-overwriting)
  - [hidden content does not fetch its pictures until w opens it](#scenario-hidden-content-does-not-fetch-its-pictures-until-w-opens-it)
  - [a text terminal keeps alt text and the viewer explains its fallback without fetching](#scenario-a-text-terminal-keeps-alt-text-and-the-viewer-explains-its-fallback-without-fetching)

## mstdn media
Source: `crates/mastodon-terminal-client/e2e/atago/tui-media.atago.yaml`
### Scenario: pictures and video are drawn and original files are saved without overwriting
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `want/photo.png` is created.
- Fixture file `want/clip.mp4` is created.
- Fixture file `dl/media1-1.png` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `dl/media1-1.png`:_

```text
original
```

#### When

```shell
# interactive (pty): mstdn --host ${api.url}
```

#### Then
- exit code is `0`
- file `dl/media1-1.png` contains `original`
- file `dl/media1-1 (1).png` is byte-identical to `want/photo.png`
- file `dl/media1-2.mp4` is byte-identical to `want/clip.mp4`
- dir `cache/images` has >= 1 entry
- mock `api` received `/photo.png`
- mock `api` received `/clip.mp4`

### Scenario: hidden content does not fetch its pictures until w opens it
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): mstdn --host ${api.url}
```

#### Then
- exit code is `0`
- mock `api` received `/hidden.png` exactly 0 time(s)

### Scenario: a text terminal keeps alt text and the viewer explains its fallback without fetching
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): mstdn --host ${api.url}
```

#### Then
- exit code is `0`
- mock `api` received `/photo.png` exactly 0 time(s)
