# atago Behavior Specs
## Summary
1 suite · 1 scenario
## Contents
- [nstr Blossom media](#nstr-blossom-media): 1 scenario
  - [stripped uploads imeta downloads stdin dry run and declined confirmation](#scenario-stripped-uploads-imeta-downloads-stdin-dry-run-and-declined-confirmation)

## nstr Blossom media
Source: `crates/nostr-terminal-client/e2e/atago/media.atago.yaml`
### Scenario: stripped uploads imeta downloads stdin dry run and declined confirmation
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --parity`.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
python3 "${specdir}/testdata/check_media.py" setup
nstr upload private.png --alt "fixture alt" --json
nstr post attached --image private.png --alt "fixture alt" --json
python3 "${specdir}/testdata/check_media.py" verify
nstr download 0000000000000000000000000000000000000000000000000000000000000000 --json
python3 -c "import time; time.sleep(1.1)"
nstr post video --video "${specdir}/../../../terminal-kit/src/media/testdata/clip.mp4" --alt "fixture clip" --json
# interactive (pty): nstr
# interactive (pty): nstr
python3 "${specdir}/testdata/check_media.py" tui
```

#### Then
- after `python3 "${specdir}/testdata/check_media.py" setup`:
  - exit code is `0`
- after `nstr upload private.png --alt "fixture alt" --json`:
  - exit code is `0`
  - stdout contains `imeta`
- after `nstr post attached --image private.png --alt "fixture alt" --json`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_media.py" verify`:
  - exit code is `0`
  - stdout contains `uploads downloads and no-send boundaries verified`
- after `nstr download 0000000000000000000000000000000000000000000000000000000000000000 --json`:
  - exit code is `5`
- after `nstr post video --video "${specdir}/../../../terminal-kit/src/media/testdata/clip.mp4" --alt "fixture clip" --json`:
  - exit code is `0`
- after `interactive (pty): nstr`:
  - exit code is `0`
- after `interactive (pty): nstr`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_media.py" tui`:
  - exit code is `0`
  - stdout contains `browser post and profile uploads verified`
