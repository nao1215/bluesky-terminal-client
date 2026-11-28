# atago Behavior Specs
## Summary
1 suite · 12 scenarios
## Contents
- [mstdn shared unsupported errors](#mstdn-shared-unsupported-errors): 12 scenarios
  - [like returns shared unsupported status for HTTP 501 without retry](#scenario-like-returns-shared-unsupported-status-for-http-501-without-retry)
  - [unlike returns shared unsupported status for HTTP 501 without retry](#scenario-unlike-returns-shared-unsupported-status-for-http-501-without-retry)
  - [repost returns shared unsupported status for HTTP 501 without retry](#scenario-repost-returns-shared-unsupported-status-for-http-501-without-retry)
  - [unrepost returns shared unsupported status for HTTP 501 without retry](#scenario-unrepost-returns-shared-unsupported-status-for-http-501-without-retry)
  - [bookmark returns shared unsupported status for HTTP 501 without retry](#scenario-bookmark-returns-shared-unsupported-status-for-http-501-without-retry)
  - [unbookmark returns shared unsupported status for HTTP 501 without retry](#scenario-unbookmark-returns-shared-unsupported-status-for-http-501-without-retry)
  - [quote on 4.4.3 writes nothing in normal and dry run modes](#scenario-quote-on-443-writes-nothing-in-normal-and-dry-run-modes)
  - [quote on unknown writes nothing in normal and dry run modes](#scenario-quote-on-unknown-writes-nothing-in-normal-and-dry-run-modes)
  - [bookmarks 404 on 3.0.6 distinguishes capability from missing resources](#scenario-bookmarks-404-on-306-distinguishes-capability-from-missing-resources)
  - [bookmarks 404 on 3.1.0 distinguishes capability from missing resources](#scenario-bookmarks-404-on-310-distinguishes-capability-from-missing-resources)
  - [bookmarks 404 on unknown distinguishes capability from missing resources](#scenario-bookmarks-404-on-unknown-distinguishes-capability-from-missing-resources)
  - [absent endpoints are exit 5 with server identity but missing resources and forbidden requests remain exit 4](#scenario-absent-endpoints-are-exit-5-with-server-identity-but-missing-resources-and-forbidden-requests-remain-exit-4)

## mstdn shared unsupported errors
Source: `crates/mastodon-terminal-client/e2e/atago/unsupported.atago.yaml`
### Scenario: like returns shared unsupported status for HTTP 501 without retry
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn like 42 --host ${api.url} --json
```

#### Then
- exit code is `5`
- stdout at `$.status` equals `5`
- stderr contains `does not support`
- mock `api` received `POST /api/v1/statuses/42/favourite` exactly 1 time(s)

### Scenario: unlike returns shared unsupported status for HTTP 501 without retry
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn unlike 42 --host ${api.url} --json
```

#### Then
- exit code is `5`
- stdout at `$.status` equals `5`
- stderr contains `does not support`
- mock `api` received `POST /api/v1/statuses/42/unfavourite` exactly 1 time(s)

### Scenario: repost returns shared unsupported status for HTTP 501 without retry
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn repost 42 --host ${api.url} --json
```

#### Then
- exit code is `5`
- stdout at `$.status` equals `5`
- stderr contains `does not support`
- mock `api` received `POST /api/v1/statuses/42/reblog` exactly 1 time(s)

### Scenario: unrepost returns shared unsupported status for HTTP 501 without retry
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn unrepost 42 --host ${api.url} --json
```

#### Then
- exit code is `5`
- stdout at `$.status` equals `5`
- stderr contains `does not support`
- mock `api` received `POST /api/v1/statuses/42/unreblog` exactly 1 time(s)

### Scenario: bookmark returns shared unsupported status for HTTP 501 without retry
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn bookmark 42 --host ${api.url} --json
```

#### Then
- exit code is `5`
- stdout at `$.status` equals `5`
- stderr contains `does not support`
- mock `api` received `POST /api/v1/statuses/42/bookmark` exactly 1 time(s)

### Scenario: unbookmark returns shared unsupported status for HTTP 501 without retry
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn unbookmark 42 --host ${api.url} --json
```

#### Then
- exit code is `5`
- stdout at `$.status` equals `5`
- stderr contains `does not support`
- mock `api` received `POST /api/v1/statuses/42/unbookmark` exactly 1 time(s)

### Scenario: quote on 4.4.3 writes nothing in normal and dry run modes
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn post comment --quote 42  --host ${api.url} --json
mstdn post comment --quote 42 --dry-run --host ${api.url} --json
```

#### Then
- after `mstdn post comment --quote 42  --host ${api.url} --json`:
  - exit code is `5`
  - stdout at `$.status` equals `5`
  - mock `api` received `POST` exactly 0 time(s)
- after `mstdn post comment --quote 42 --dry-run --host ${api.url} --json`:
  - exit code is `5`
  - stdout at `$.status` equals `5`
  - mock `api` received `POST` exactly 0 time(s)

### Scenario: quote on unknown writes nothing in normal and dry run modes
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn post comment --quote 42  --host ${api.url} --json
mstdn post comment --quote 42 --dry-run --host ${api.url} --json
```

#### Then
- after `mstdn post comment --quote 42  --host ${api.url} --json`:
  - exit code is `4`
  - stdout at `$.status` equals `4`
  - mock `api` received `POST` exactly 0 time(s)
- after `mstdn post comment --quote 42 --dry-run --host ${api.url} --json`:
  - exit code is `4`
  - stdout at `$.status` equals `4`
  - mock `api` received `POST` exactly 0 time(s)

### Scenario: bookmarks 404 on 3.0.6 distinguishes capability from missing resources
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn bookmarks --host ${api.url} --json
```

#### Then
- exit code is `5`
- stdout at `$.status` equals `5`

### Scenario: bookmarks 404 on 3.1.0 distinguishes capability from missing resources
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn bookmarks --host ${api.url} --json
```

#### Then
- exit code is `4`
- stdout at `$.status` equals `4`

### Scenario: bookmarks 404 on unknown distinguishes capability from missing resources
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn bookmarks --host ${api.url} --json
```

#### Then
- exit code is `4`
- stdout at `$.status` equals `4`

### Scenario: absent endpoints are exit 5 with server identity but missing resources and forbidden requests remain exit 4
#### Given
- Stub HTTP server `api` serves 6 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn timeline public --json --host ${api.url}
mstdn status missing --json --host ${api.url}
mstdn timeline home --json --host ${api.url}
```

#### Then
- after `mstdn timeline public --json --host ${api.url}`:
  - exit code is `5`
  - stdout at `$.status` equals `5`; at `$.error` equals `127.0.0.1:${api.port} (gotosocial 0.19.0) does not support v1/timelines/public`
- after `mstdn status missing --json --host ${api.url}`:
  - exit code is `4`
  - stdout at `$.status` equals `4`
- after `mstdn timeline home --json --host ${api.url}`:
  - exit code is `4`
  - stdout at `$.status` equals `4`
  - mock `api` received `/nodeinfo/2.0` exactly 1 time(s)
