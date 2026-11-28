# atago Behavior Specs
## Summary
1 suite · 1 scenario
## Contents
- [nstr saved accounts](#nstr-saved-accounts): 1 scenario
  - [save select and remove a key-file account without signing](#scenario-save-select-and-remove-a-key-file-account-without-signing)

## nstr saved accounts
Source: `crates/nostr-terminal-client/e2e/atago/accounts.atago.yaml`
### Scenario: save select and remove a key-file account without signing
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py"`.
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr login fixture --public-key 76c5012018add43f4d5b3842e479d47850333e4b86525946862477546eed7742 --key-file cfg/key --relay "${relay}" --json
nstr accounts --use fixture --json
nstr --account fixture whoami --json
nstr logout fixture --yes --json
python3 "${specdir}/testdata/check_accounts.py"
# interactive (pty): nstr
```

#### Then
- after `nstr login fixture --public-key 76c5012018add43f4d5b3842e479d47850333e4b86525946862477546eed7742 --key-file cfg/key --relay "${relay}" --json`:
  - exit code is `0`
- after `nstr accounts --use fixture --json`:
  - exit code is `0`
  - stdout contains `fixture`
- after `nstr --account fixture whoami --json`:
  - exit code is `0`
  - stdout contains `alice`
- after `nstr logout fixture --yes --json`:
  - exit code is `0`
  - file `cfg/key` exists
  - file `published.json` does not exist
- after `python3 "${specdir}/testdata/check_accounts.py"`:
  - exit code is `0`
  - stdout contains `account read state isolated`
- after `interactive (pty): nstr`:
  - exit code is `0`
  - file `published.json` does not exist

#### Generated artifacts
- `cfg/key`
