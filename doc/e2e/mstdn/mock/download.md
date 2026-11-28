# atago Behavior Specs
## Summary
1 suite · 2 scenarios
## Contents
- [mstdn download](#mstdn-download): 2 scenarios
  - [download saves each file of a post, and the boosted post's for a boost](#scenario-download-saves-each-file-of-a-post-and-the-boosted-posts-for-a-boost)
  - [author --download saves the files of the account's posts](#scenario-author---download-saves-the-files-of-the-accounts-posts)

## mstdn download
Source: `crates/mastodon-terminal-client/e2e/atago/download.atago.yaml`
### Scenario: download saves each file of a post, and the boosted post's for a boost
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `want/photo.png` is created.
- Fixture file `want/clip.mp4` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_DOWNLOAD_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn download 300 -o 写真 --host ${api.url}
mstdn download ${api.url}/@alice/300 -o 写真 --json
mstdn download 301 --host ${api.url}
```

#### Then
- after `mstdn download 300 -o 写真 --host ${api.url}`:
  - exit code is `0`
  - stdout matches `/^写真[/\\]299-1\.png\n写真[/\\]299-2\.mp4\n$/`
  - file `写真/299-1.png` is byte-identical to `want/photo.png`
  - file `写真/299-2.mp4` is byte-identical to `want/clip.mp4`
- after `mstdn download ${api.url}/@alice/300 -o 写真 --json`:
  - exit code is `0`
  - stdout matches `/"path":"写真(/|\\\\)299-1 \(1\)\.png","postId":"299","type":"image","url":"http://127\.0\.0\.1:[0-9]+/files/original/a\.png"/`
- after `mstdn download 301 --host ${api.url}`:
  - exit code is `1`
  - stderr contains `error: ${api.url}/@alice/301 has no files`

### Scenario: author --download saves the files of the account's posts
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_DOWNLOAD_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn author alice --download -n 2 --host ${api.url}
```

#### Then
- exit code is `0`
- stdout matches `/dl[/\\]12-1\.png\n$/`
