# atago Behavior Specs
## Summary
1 suite · 1 scenario
## Contents
- [nstr search capability](#nstr-search-capability): 1 scenario
  - [missing search configuration and unadvertised NIP-50 are unsupported while hashtags work](#scenario-missing-search-configuration-and-unadvertised-nip-50-are-unsupported-while-hashtags-work)

## nstr search capability
Source: `crates/nostr-terminal-client/e2e/atago/search.atago.yaml`
### Scenario: missing search configuration and unadvertised NIP-50 are unsupported while hashtags work
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py"`.
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr search words --json
python3 -c 'import json,pathlib; p=pathlib.Path("cfg/config.json"); c=json.loads(p.read_text()); c["searchRelays"]=c["relays"]; p.write_text(json.dumps(c))'
nstr search words --json
nstr search "#absent" -n 1 --json
```

#### Then
- after `nstr search words --json`:
  - exit code is `5`
  - stdout at `$.status` equals `5`
  - stderr contains `unsupported: NIP-50 search`
- after `python3 -c 'import json,pathlib; p=pathlib.Path("cfg/config.json"); c=json.loads(p.read_text()); c["searchRelays"]=c["relays"]; p.write_text(json.dumps(c))'`:
  - exit code is `0`
- after `nstr search words --json`:
  - exit code is `5`
  - stdout at `$.status` equals `5`
  - stderr contains `not advertised`
- after `nstr search "#absent" -n 1 --json`:
  - exit code is `0`
  - stdout is empty
