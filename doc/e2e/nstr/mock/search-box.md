# atago Behavior Specs
## Summary
1 suite · 3 scenarios
## Contents
- [nstr search box](#nstr-search-box): 3 scenarios
  - [search overlays the list and Escape restores its selected note without refetching](#scenario-search-overlays-the-list-and-escape-restores-its-selected-note-without-refetching)
  - [failed search preserves the previous list content](#scenario-failed-search-preserves-the-previous-list-content)
  - [search results page through a timestamp tie and remain usable as a column](#scenario-search-results-page-through-a-timestamp-tie-and-remain-usable-as-a-column)

## nstr search box
Source: `crates/nostr-terminal-client/e2e/atago/search-box.atago.yaml`
### Scenario: search overlays the list and Escape restores its selected note without refetching
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --parity`.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): nstr
python3 "${specdir}/testdata/check_search.py" restore
```

#### Then
- after `interactive (pty): nstr`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_search.py" restore`:
  - exit code is `0`
  - stdout contains `checked restore`

### Scenario: failed search preserves the previous list content
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py"`.
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
- after `python3 "${specdir}/testdata/check_tui.py" none`:
  - exit code is `0`

### Scenario: search results page through a timestamp tie and remain usable as a column
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --pages --parity`.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): nstr
python3 "${specdir}/testdata/check_search.py" pages
```

#### Then
- after `interactive (pty): nstr`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_search.py" pages`:
  - exit code is `0`
  - stdout contains `checked pages`
