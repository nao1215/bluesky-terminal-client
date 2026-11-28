# atago Behavior Specs
## Summary
1 suite · 4 scenarios
## Contents
- [msky live behavior](#msky-live-behavior): 4 scenarios
  - [behavior: server restart during a full-screen session](#scenario-behavior-server-restart-during-a-full-screen-session)
  - [behavior: stopped server gives CLI network status four](#scenario-behavior-stopped-server-gives-cli-network-status-four)
  - [behavior: rate-limited reads wait and report the retry](#scenario-behavior-rate-limited-reads-wait-and-report-the-retry)
  - [behavior: rate-limited write is refused and creates no note](#scenario-behavior-rate-limited-write-is-refused-and-creates-no-note)

## msky live behavior
Source: `crates/misskey-terminal-client/e2e/compat/zbehavior.atago.yaml`

Secrets declared: `${env:MSKY_TOKEN}`, `${env:MSKY_SECOND_TOKEN}`, `${env:MSKY_THIRD_TOKEN}`, `${env:MSKY_ADMIN_TOKEN}`, `${env:MSKY_RESTRICTED_TOKEN}`.
### Scenario: behavior: server restart during a full-screen session
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): msky
```

#### Then
- exit code is `0`

### Scenario: behavior: stopped server gives CLI network status four
#### Given
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
docker compose -p ${env:MSKY_COMPOSE_PROJECT} -f ${env:MSKY_COMPOSE_FILE} stop ingress
msky whoami --json
docker compose -p ${env:MSKY_COMPOSE_PROJECT} -f ${env:MSKY_COMPOSE_FILE} start --wait --wait-timeout 30 ingress
msky whoami --json
```

#### Then
- after `docker compose -p ${env:MSKY_COMPOSE_PROJECT} -f ${env:MSKY_COMPOSE_FILE} stop ingress`:
  - exit code is `0`
- after `msky whoami --json`:
  - exit code is `4`
  - stdout at `$.status` equals `4`; at `$.error` matches `/cannot reach the server/`
- after `docker compose -p ${env:MSKY_COMPOSE_PROJECT} -f ${env:MSKY_COMPOSE_FILE} start --wait --wait-timeout 30 ingress`:
  - exit code is `0`
- after `msky whoami --json`:
  - exit code is `0`
  - stdout at `$.username` equals `compat_admin`

### Scenario: behavior: rate-limited reads wait and report the retry
#### Given
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# HTTP POST /api/admin/roles/update-default-policies via api
# HTTP POST /api/i/notifications via api
# HTTP POST /api/i/notifications via api
# capture ${retry_after} from response header Retry-After
msky notifications -n 1 --json
# HTTP POST /api/admin/roles/update-default-policies via api
```

#### Then
- after `HTTP POST /api/admin/roles/update-default-policies`:
  - HTTP status is `204`
- after `HTTP POST /api/i/notifications`:
  - HTTP status is `200`
- after `HTTP POST /api/i/notifications`:
  - HTTP status is `429`
  - header `Retry-After` matches `/^[0-9]+$/`
  - body contains `RATE_LIMIT_EXCEEDED`
- after `msky notifications -n 1 --json`:
  - exit code is `0`
  - stderr contains `asks to wait; trying again in ${retry_after} s`
  - completes in under 45s and in at least 1s
- after `HTTP POST /api/admin/roles/update-default-policies`:
  - HTTP status is `204`

### Scenario: behavior: rate-limited write is refused and creates no note
#### Given
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSKY_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# HTTP POST /api/admin/roles/update-default-policies via api
msky post compat-rate-prime --json
msky post compat-rate-refused --json
# HTTP POST /api/admin/roles/update-default-policies via api
msky search compat-rate-refused --json
```

#### Then
- after `HTTP POST /api/admin/roles/update-default-policies`:
  - HTTP status is `204`
- after `msky post compat-rate-prime --json`:
  - exit code is `0`
- after `msky post compat-rate-refused --json`:
  - exit code is `4`
  - stdout contains `RATE_LIMIT_EXCEEDED`
  - completes in under 10s
- after `HTTP POST /api/admin/roles/update-default-policies`:
  - HTTP status is `204`
- after `msky search compat-rate-refused --json`:
  - exit code is `0`
  - stdout is empty
