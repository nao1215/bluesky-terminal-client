# atago Behavior Specs
## Summary
1 suite · 5 scenarios
## Contents
- [msky live limits](#msky-live-limits): 5 scenarios
  - [heavy: text at the advertised limit and one over](#scenario-heavy-text-at-the-advertised-limit-and-one-over)
  - [heavy: sixteen pictures and one over before upload](#scenario-heavy-sixteen-pictures-and-one-over-before-upload)
  - [heavy: alt text at the server limit and one over before upload](#scenario-heavy-alt-text-at-the-server-limit-and-one-over-before-upload)
  - [heavy: file bytes at the server limit and one over before upload](#scenario-heavy-file-bytes-at-the-server-limit-and-one-over-before-upload)
  - [behavior: revoked token remains authentication status four](#scenario-behavior-revoked-token-remains-authentication-status-four)

## msky live limits
Source: `crates/misskey-terminal-client/e2e/compat/limits.atago.yaml`

Secrets declared: `${env:MSKY_TOKEN}`, `${env:MSKY_SECOND_TOKEN}`, `${env:MSKY_THIRD_TOKEN}`, `${env:MSKY_ADMIN_TOKEN}`, `${env:MSKY_RESTRICTED_TOKEN}`.
### Scenario: heavy: text at the advertised limit and one over
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_stdin for `msky`:_

```text
${env:MSKY_SEED_MAX_TEXT_VALUE}
```

_stdin for `msky`:_

```text
${env:MSKY_SEED_OVER_TEXT_VALUE}
```

#### When

```shell
msky post - --json
# capture ${note} from stdout
msky note ${note} --json
msky post - --json
```

#### Then
- after `msky post - --json`:
  - exit code is `0`
- after `msky note ${note} --json`:
  - exit code is `0`
  - stdout at `$.text` matches `/^x+$/`
- after `msky post - --json`:
  - exit code is `1`
  - stdout contains `the limit is ${env:MSKY_SEED_MAX_TEXT}`

### Scenario: heavy: sixteen pictures and one over before upload
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post compat-sixteen --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --json
# capture ${note} from stdout
msky note ${note} --json
# HTTP POST /api/drive/files via api
# capture ${before} from the response body
msky post compat-seventeen --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --json
# HTTP POST /api/drive/files via api
```

#### Then
- after `msky post compat-sixteen --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --json`:
  - exit code is `0`
- after `msky note ${note} --json`:
  - exit code is `0`
  - stdout at `$.files` has length 16
- after `msky post compat-seventeen --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --image ${env:MSKY_SEED_PICTURE_PATH} --json`:
  - exit code is `1`
  - stdout contains `at most 16 pictures`
- after `HTTP POST /api/drive/files`:
  - body at `$[0].id` equals `${before}`

### Scenario: heavy: alt text at the server limit and one over before upload
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post compat-limit --image ${env:MSKY_SEED_PICTURE_PATH} --alt "${env:MSKY_SEED_MAX_ALT_TEXT}" --json
# capture ${note} from stdout
msky note ${note} --json
# HTTP POST /api/drive/files via api
# capture ${before} from the response body
msky post compat-limit --image ${env:MSKY_SEED_PICTURE_PATH} --alt "${env:MSKY_SEED_OVER_ALT_TEXT}" --json
# HTTP POST /api/drive/files via api
```

#### Then
- after `msky post compat-limit --image ${env:MSKY_SEED_PICTURE_PATH} --alt "${env:MSKY_SEED_MAX_ALT_TEXT}" --json`:
  - exit code is `0`
- after `msky note ${note} --json`:
  - exit code is `0`
  - stdout at `$.files[0].description` equals `${env:MSKY_SEED_MAX_ALT_TEXT}`
- after `msky post compat-limit --image ${env:MSKY_SEED_PICTURE_PATH} --alt "${env:MSKY_SEED_OVER_ALT_TEXT}" --json`:
  - exit code is `1`
  - stdout contains `${env:MSKY_SEED_MAX_ALT}`
- after `HTTP POST /api/drive/files`:
  - body at `$[0].id` equals `${before}`

### Scenario: heavy: file bytes at the server limit and one over before upload
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky post compat-limit --image ${env:MSKY_SEED_DIR}/max-file.png --json
# capture ${note} from stdout
msky note ${note} --json
# HTTP POST /api/drive/files via api
# capture ${before} from the response body
msky post compat-limit --image ${env:MSKY_SEED_DIR}/over-file.png --json
# HTTP POST /api/drive/files via api
```

#### Then
- after `msky post compat-limit --image ${env:MSKY_SEED_DIR}/max-file.png --json`:
  - exit code is `0`
- after `msky note ${note} --json`:
  - exit code is `0`
  - stdout at `$.files[0].size` equals `1048576`
- after `msky post compat-limit --image ${env:MSKY_SEED_DIR}/over-file.png --json`:
  - exit code is `1`
  - stdout contains `1048576`
- after `HTTP POST /api/drive/files`:
  - body at `$[0].id` equals `${before}`

### Scenario: behavior: revoked token remains authentication status four
#### Given
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# HTTP POST /api/miauth/gen-token via api
# capture ${revoked} from the response body
# HTTP POST /api/i/revoke-token via api
msky whoami --json
```

#### Then
- after `HTTP POST /api/i/revoke-token`:
  - HTTP status is `204`
- after `msky whoami --json`:
  - exit code is `4`
  - stdout contains `AUTHENTICATION_FAILED`
