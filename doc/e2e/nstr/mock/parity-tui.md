# atago Behavior Specs
## Summary
1 suite · 4 scenarios
## Contents
- [nstr parity screens](#nstr-parity-screens): 4 scenarios
  - [warnings profiles search columns actions and declined confirmations](#scenario-warnings-profiles-search-columns-actions-and-declined-confirmations)
  - [first run configuration can be cancelled without creating an account](#scenario-first-run-configuration-can-be-cancelled-without-creating-an-account)
  - [profile form and private message writes reach only local services](#scenario-profile-form-and-private-message-writes-reach-only-local-services)
  - [first page fetches mutes concurrently and never draws muted authors](#scenario-first-page-fetches-mutes-concurrently-and-never-draws-muted-authors)

## nstr parity screens
Source: `crates/nostr-terminal-client/e2e/atago/parity-tui.atago.yaml`
### Scenario: warnings profiles search columns actions and declined confirmations
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --parity`.
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
  - stdout contains `checked none`

### Scenario: first run configuration can be cancelled without creating an account
#### Given
- The command runs with a cleared environment (passing through: PATH, HOME, USERPROFILE, APPDATA, LOCALAPPDATA).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): nstr
```

#### Then
- exit code is `0`
- file `empty/config.json` does not exist

### Scenario: profile form and private message writes reach only local services
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --parity`.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): nstr
python3 "${specdir}/testdata/check_dm.py"
python3 "${specdir}/testdata/check_tui.py" profile-chat
```

#### Then
- after `interactive (pty): nstr`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_dm.py"`:
  - exit code is `0`
  - stdout contains `gift wraps verified independently`
- after `python3 "${specdir}/testdata/check_tui.py" profile-chat`:
  - exit code is `0`

### Scenario: first page fetches mutes concurrently and never draws muted authors
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --overlap-mute`.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): nstr
```

#### Then
- exit code is `0`
