# atago Behavior Specs
## Summary
1 suite · 2 scenarios
## Contents
- [nstr whoami](#nstr-whoami): 2 scenarios
  - [whoami prints the newest profile metadata](#scenario-whoami-prints-the-newest-profile-metadata)
  - [terminal errors are exit 2, usage errors exit 1, and config errors exit 3](#scenario-terminal-errors-are-exit-2-usage-errors-exit-1-and-config-errors-exit-3)

## nstr whoami
Source: `crates/nostr-terminal-client/e2e/atago/whoami.atago.yaml`
### Scenario: whoami prints the newest profile metadata
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --dead`.
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR, TZ.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr whoami
nstr whoami --json
```

#### Then
- after `nstr whoami`:
  - exit code is `0`
  - stdout equals an exact value
  - stderr contains `warning: ws://127.0.0.1:`, does not contain `${relay}`
- after `nstr whoami --json`:
  - exit code is `0`
  - stdout at `$.id` equals `76c5012018add43f4d5b3842e479d47850333e4b86525946862477546eed7742`; at `$.username` equals `npub1wmzszgqc4h2r7n2m8ppwg7w50pgrx0jtsef9j35xy3m4gmhdwapqx894z8`; at `$.npub` equals `npub1wmzszgqc4h2r7n2m8ppwg7w50pgrx0jtsef9j35xy3m4gmhdwapqx894z8`; at `$.nprofile` equals `nprofile1qqs8d3gpyqv2m4plf4dnsshy0828s5pn8e9cv5jeg6rzga65dmkhwssm2jru0`; at `$.name` equals `Alice 🐈‍⬛`; at `$.description` equals `テスト用のアカウントです`; at `$.picture` equals `http://127.0.0.1:9/alice.png`; at `$.host` equals `null`; at `$.url` equals `null`; at `$.followersCount` equals `null`
  - file `requests.jsonl` contains `["REQ", "nstr-0", {"authors": ["76c5012018add43f4d5b3842e479d47850333e4b86525946862477546eed7742"], "kinds": [0], "limit": 1}]`

#### Expected output
_expected stdout:_

```text
npub1wmzszgqc4h2r7n2m8ppwg7w50pgrx0jtsef9j35xy3m4gmhdwapqx894z8
Alice 🐈‍⬛
テスト用のアカウントです
```

### Scenario: terminal errors are exit 2, usage errors exit 1, and config errors exit 3
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Fixture file `bad/config.json` is created.
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Fixture file `http/config.json` is created.
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `bad/config.json`:_

```text
{"publicKey": "npub1notakey", "relays": ["ws://127.0.0.1:1/"]}
```

_Fixture `http/config.json`:_

```text
{"publicKey": "76c5012018add43f4d5b3842e479d47850333e4b86525946862477546eed7742", "relays": ["https://127.0.0.1:1/"]}
```

#### When

```shell
nstr --json
nstr timeline -n 0
nstr notifications -n x --json
nstr post "  "
nstr whoami --json
nstr timeline
nstr notifications
```

#### Then
- after `nstr --json`:
  - exit code is `2`
  - stdout at `$.error` equals `nstr needs an interactive terminal`; at `$.status` equals `2`
  - stderr contains `error: nstr needs an interactive terminal`
- after `nstr timeline -n 0`:
  - exit code is `1`
  - stderr contains `error: invalid value '0' for '--limit <LIMIT>'`
- after `nstr notifications -n x --json`:
  - exit code is `1`
  - stdout at `$.status` equals `1`
- after `nstr post "  "`:
  - exit code is `1`
  - stderr equals an exact value
- after `nstr whoami --json`:
  - exit code is `3`
  - stdout at `$.status` equals `3`; at `$.hint` equals `null`
  - stderr contains `error: cannot open config `
- after `nstr timeline`:
  - exit code is `3`
  - stderr equals an exact value
- after `nstr notifications`:
  - exit code is `3`
  - stderr equals an exact value

#### Expected output
_expected stderr:_

```text
error: post text must not be empty
hint: run `nstr --help` for usage
```

_expected stderr:_

```text
error: publicKey must be hex, npub or nprofile
```

_expected stderr:_

```text
error: relay URLs require ws:// or wss://, a host, no credentials and no fragment
```

