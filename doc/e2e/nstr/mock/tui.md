# atago Behavior Specs
## Summary
1 suite · 5 scenarios
## Contents
- [nstr full-screen client](#nstr-full-screen-client): 5 scenarios
  - [tabs thread help settings and cancellation never publish](#scenario-tabs-thread-help-settings-and-cancellation-never-publish)
  - [ctrl+s sends new note and nested reply and l b send exact signed events](#scenario-ctrls-sends-new-note-and-nested-reply-and-l-b-send-exact-signed-events)
  - [rejected writes roll back their marks and keep the composer text](#scenario-rejected-writes-roll-back-their-marks-and-keep-the-composer-text)
  - [inclusive timestamp boundary paging reaches older notes and thread parents](#scenario-inclusive-timestamp-boundary-paging-reaches-older-notes-and-thread-parents)
  - [content URLs and imeta pictures use terminal graphics](#scenario-content-urls-and-imeta-pictures-use-terminal-graphics)

## nstr full-screen client
Source: `crates/nostr-terminal-client/e2e/atago/tui.atago.yaml`
### Scenario: tabs thread help settings and cancellation never publish
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --notice --dead`.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): nstr
python3 "${specdir}/testdata/check_tui.py" none
# interactive (pty): nstr
```

#### Then
- after `interactive (pty): nstr`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_tui.py" none`:
  - exit code is `0`
  - stdout contains `checked none`
  - file `cfg/settings.json` contains `"language": "ja"`
  - file `cfg/settings.json` contains `"theme": "ayu-dark"`
- after `interactive (pty): nstr`:
  - exit code is `0`

### Scenario: ctrl+s sends new note and nested reply and l b send exact signed events
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py"`.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): nstr
python3 "${specdir}/testdata/check_tui.py" writes
```

#### Then
- after `interactive (pty): nstr`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_tui.py" writes`:
  - exit code is `0`
  - stdout contains `checked writes`

### Scenario: rejected writes roll back their marks and keep the composer text
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --reject "blocked by fixture"`.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): nstr
python3 "${specdir}/testdata/check_tui.py" reject
```

#### Then
- after `interactive (pty): nstr`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_tui.py" reject`:
  - exit code is `0`
  - stdout contains `checked reject`

### Scenario: inclusive timestamp boundary paging reaches older notes and thread parents
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --pages`.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): nstr
python3 "${specdir}/testdata/check_tui.py" pages
```

#### Then
- after `interactive (pty): nstr`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_tui.py" pages`:
  - exit code is `0`
  - stdout contains `checked pages`

### Scenario: content URLs and imeta pictures use terminal graphics
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay_without_dns.py" --pictures`.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): nstr
python3 "${specdir}/testdata/check_tui.py" none
```

#### Then
- after `interactive (pty): nstr`:
  - exit code is `0`
  - file `images.jsonl` contains `"/picture.png"`
  - file `images.jsonl` contains `"/imeta"`
- after `python3 "${specdir}/testdata/check_tui.py" none`:
  - exit code is `0`
