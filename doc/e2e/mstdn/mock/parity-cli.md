# atago Behavior Specs
## Summary
1 suite · 40 scenarios
## Contents
- [mstdn CLI parity](#mstdn-cli-parity): 40 scenarios
  - [like sends its exact request once successfully](#scenario-like-sends-its-exact-request-once-successfully)
  - [like sends its exact request once and propagates refusal](#scenario-like-sends-its-exact-request-once-and-propagates-refusal)
  - [unlike sends its exact request once successfully](#scenario-unlike-sends-its-exact-request-once-successfully)
  - [unlike sends its exact request once and propagates refusal](#scenario-unlike-sends-its-exact-request-once-and-propagates-refusal)
  - [repost sends its exact request once successfully](#scenario-repost-sends-its-exact-request-once-successfully)
  - [repost sends its exact request once and propagates refusal](#scenario-repost-sends-its-exact-request-once-and-propagates-refusal)
  - [unrepost sends its exact request once successfully](#scenario-unrepost-sends-its-exact-request-once-successfully)
  - [unrepost sends its exact request once and propagates refusal](#scenario-unrepost-sends-its-exact-request-once-and-propagates-refusal)
  - [delete sends its exact request once successfully](#scenario-delete-sends-its-exact-request-once-successfully)
  - [delete sends its exact request once and propagates refusal](#scenario-delete-sends-its-exact-request-once-and-propagates-refusal)
  - [bookmark sends its exact request once successfully](#scenario-bookmark-sends-its-exact-request-once-successfully)
  - [bookmark sends its exact request once and propagates refusal](#scenario-bookmark-sends-its-exact-request-once-and-propagates-refusal)
  - [unbookmark sends its exact request once successfully](#scenario-unbookmark-sends-its-exact-request-once-successfully)
  - [unbookmark sends its exact request once and propagates refusal](#scenario-unbookmark-sends-its-exact-request-once-and-propagates-refusal)
  - [follow uses the resolved local account ID](#scenario-follow-uses-the-resolved-local-account-id)
  - [unfollow uses the resolved local account ID](#scenario-unfollow-uses-the-resolved-local-account-id)
  - [mute uses the resolved local account ID](#scenario-mute-uses-the-resolved-local-account-id)
  - [unmute uses the resolved local account ID](#scenario-unmute-uses-the-resolved-local-account-id)
  - [block uses the resolved local account ID](#scenario-block-uses-the-resolved-local-account-id)
  - [unblock uses the resolved local account ID](#scenario-unblock-uses-the-resolved-local-account-id)
  - [report requires confirmation and sends the category and status once](#scenario-report-requires-confirmation-and-sends-the-category-and-status-once)
  - [likes 42 reads its matching API and common JSON](#scenario-likes-42-reads-its-matching-api-and-common-json)
  - [reposts 42 reads its matching API and common JSON](#scenario-reposts-42-reads-its-matching-api-and-common-json)
  - [followers reads its matching API and common JSON](#scenario-followers-reads-its-matching-api-and-common-json)
  - [follows reads its matching API and common JSON](#scenario-follows-reads-its-matching-api-and-common-json)
  - [followers --pending reads its matching API and common JSON](#scenario-followers---pending-reads-its-matching-api-and-common-json)
  - [mutes reads its matching API and common JSON](#scenario-mutes-reads-its-matching-api-and-common-json)
  - [blocks reads its matching API and common JSON](#scenario-blocks-reads-its-matching-api-and-common-json)
  - [favourites reads its matching API and common JSON](#scenario-favourites-reads-its-matching-api-and-common-json)
  - [bookmarks reads its matching API and common JSON](#scenario-bookmarks-reads-its-matching-api-and-common-json)
  - [lists reads its matching API and common JSON](#scenario-lists-reads-its-matching-api-and-common-json)
  - [list 9 reads its matching API and common JSON](#scenario-list-9-reads-its-matching-api-and-common-json)
  - [search bob --accounts reads its matching API and common JSON](#scenario-search-bob---accounts-reads-its-matching-api-and-common-json)
  - [chat reads its matching API and common JSON](#scenario-chat-reads-its-matching-api-and-common-json)
  - [chat sends a direct status with idempotency key](#scenario-chat-sends-a-direct-status-with-idempotency-key)
  - [post quote and dry run resolve the quote ID](#scenario-post-quote-and-dry-run-resolve-the-quote-id)
  - [notifications seen writes exactly the latest read marker](#scenario-notifications-seen-writes-exactly-the-latest-read-marker)
  - [interactive report decline sends no request](#scenario-interactive-report-decline-sends-no-request)
  - [received quote exposes quoteId and boost preserves it](#scenario-received-quote-exposes-quoteid-and-boost-preserves-it)
  - [chat history excludes public posts and other participants](#scenario-chat-history-excludes-public-posts-and-other-participants)

## mstdn CLI parity
Source: `crates/mastodon-terminal-client/e2e/atago/parity-cli.atago.yaml`
### Scenario: like sends its exact request once successfully
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn like 42 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stdout at `$.action` equals `like`
- stderr is empty
- mock `api` received `POST /api/v1/statuses/42/favourite` exactly 1 time(s)

### Scenario: like sends its exact request once and propagates refusal
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn like 42 --host ${api.url} --json
```

#### Then
- exit code is `4`
- stdout does not contain `"action"`
- mock `api` received `POST /api/v1/statuses/42/favourite` exactly 1 time(s)

### Scenario: unlike sends its exact request once successfully
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn unlike 42 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stdout at `$.action` equals `unlike`
- stderr is empty
- mock `api` received `POST /api/v1/statuses/42/unfavourite` exactly 1 time(s)

### Scenario: unlike sends its exact request once and propagates refusal
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn unlike 42 --host ${api.url} --json
```

#### Then
- exit code is `4`
- stdout does not contain `"action"`
- mock `api` received `POST /api/v1/statuses/42/unfavourite` exactly 1 time(s)

### Scenario: repost sends its exact request once successfully
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn repost 42 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stdout at `$.action` equals `repost`
- stderr is empty
- mock `api` received `POST /api/v1/statuses/42/reblog` exactly 1 time(s)

### Scenario: repost sends its exact request once and propagates refusal
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn repost 42 --host ${api.url} --json
```

#### Then
- exit code is `4`
- stdout does not contain `"action"`
- mock `api` received `POST /api/v1/statuses/42/reblog` exactly 1 time(s)

### Scenario: unrepost sends its exact request once successfully
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn unrepost 42 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stdout at `$.action` equals `unrepost`
- stderr is empty
- mock `api` received `POST /api/v1/statuses/42/unreblog` exactly 1 time(s)

### Scenario: unrepost sends its exact request once and propagates refusal
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn unrepost 42 --host ${api.url} --json
```

#### Then
- exit code is `4`
- stdout does not contain `"action"`
- mock `api` received `POST /api/v1/statuses/42/unreblog` exactly 1 time(s)

### Scenario: delete sends its exact request once successfully
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn delete 42 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stdout at `$.action` equals `delete`
- stderr is empty
- mock `api` received `DELETE /api/v1/statuses/42` exactly 1 time(s)

### Scenario: delete sends its exact request once and propagates refusal
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn delete 42 --host ${api.url} --json
```

#### Then
- exit code is `4`
- stdout does not contain `"action"`
- mock `api` received `DELETE /api/v1/statuses/42` exactly 1 time(s)

### Scenario: bookmark sends its exact request once successfully
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn bookmark 42 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stdout at `$.action` equals `bookmark`
- stderr is empty
- mock `api` received `POST /api/v1/statuses/42/bookmark` exactly 1 time(s)

### Scenario: bookmark sends its exact request once and propagates refusal
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn bookmark 42 --host ${api.url} --json
```

#### Then
- exit code is `4`
- stdout does not contain `"action"`
- mock `api` received `POST /api/v1/statuses/42/bookmark` exactly 1 time(s)

### Scenario: unbookmark sends its exact request once successfully
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn unbookmark 42 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stdout at `$.action` equals `unbookmark`
- stderr is empty
- mock `api` received `POST /api/v1/statuses/42/unbookmark` exactly 1 time(s)

### Scenario: unbookmark sends its exact request once and propagates refusal
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn unbookmark 42 --host ${api.url} --json
```

#### Then
- exit code is `4`
- stdout does not contain `"action"`
- mock `api` received `POST /api/v1/statuses/42/unbookmark` exactly 1 time(s)

### Scenario: follow uses the resolved local account ID
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn follow @bob --host ${api.url} --json
```

#### Then
- exit code is `0`
- stdout at `$.action` equals `follow`
- stderr is empty
- mock `api` received `POST /api/v1/accounts/7/follow` exactly 1 time(s)

### Scenario: unfollow uses the resolved local account ID
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn unfollow @bob --host ${api.url} --json
```

#### Then
- exit code is `0`
- stdout at `$.action` equals `unfollow`
- stderr is empty
- mock `api` received `POST /api/v1/accounts/7/unfollow` exactly 1 time(s)

### Scenario: mute uses the resolved local account ID
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn mute @bob --host ${api.url} --json
```

#### Then
- exit code is `0`
- stdout at `$.action` equals `mute`
- stderr is empty
- mock `api` received `POST /api/v1/accounts/7/mute` exactly 1 time(s)

### Scenario: unmute uses the resolved local account ID
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn unmute @bob --host ${api.url} --json
```

#### Then
- exit code is `0`
- stdout at `$.action` equals `unmute`
- stderr is empty
- mock `api` received `POST /api/v1/accounts/7/unmute` exactly 1 time(s)

### Scenario: block uses the resolved local account ID
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn block @bob --host ${api.url} --json
```

#### Then
- exit code is `0`
- stdout at `$.action` equals `block`
- stderr is empty
- mock `api` received `POST /api/v1/accounts/7/block` exactly 1 time(s)

### Scenario: unblock uses the resolved local account ID
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn unblock @bob --host ${api.url} --json
```

#### Then
- exit code is `0`
- stdout at `$.action` equals `unblock`
- stderr is empty
- mock `api` received `POST /api/v1/accounts/7/unblock` exactly 1 time(s)

### Scenario: report requires confirmation and sends the category and status once
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn report 42 --reason spam --comment unsolicited --host ${api.url} --json
mstdn report 42 --reason spam --comment unsolicited --yes --host ${api.url} --json
```

#### Then
- after `mstdn report 42 --reason spam --comment unsolicited --host ${api.url} --json`:
  - exit code is `1`
  - mock `api` received `POST /api/v1/reports` exactly 0 time(s)
- after `mstdn report 42 --reason spam --comment unsolicited --yes --host ${api.url} --json`:
  - exit code is `0`
  - stdout at `$.action` equals `report`
  - stderr is empty
  - mock `api` received `POST /api/v1/reports` exactly 1 time(s)

### Scenario: likes 42 reads its matching API and common JSON
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn likes 42 -n 1 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `GET /api/v1/statuses/42/favourited_by` exactly 1 time(s)

### Scenario: reposts 42 reads its matching API and common JSON
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn reposts 42 -n 1 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `GET /api/v1/statuses/42/reblogged_by` exactly 1 time(s)

### Scenario: followers reads its matching API and common JSON
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn followers -n 1 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `GET /api/v1/accounts/7/followers` exactly 1 time(s)

### Scenario: follows reads its matching API and common JSON
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn follows -n 1 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `GET /api/v1/accounts/7/following` exactly 1 time(s)

### Scenario: followers --pending reads its matching API and common JSON
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn followers --pending -n 1 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `GET /api/v1/follow_requests` exactly 1 time(s)

### Scenario: mutes reads its matching API and common JSON
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn mutes -n 1 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `GET /api/v1/mutes` exactly 1 time(s)

### Scenario: blocks reads its matching API and common JSON
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn blocks -n 1 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `GET /api/v1/blocks` exactly 1 time(s)

### Scenario: favourites reads its matching API and common JSON
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn favourites -n 1 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `GET /api/v1/favourites` exactly 1 time(s)

### Scenario: bookmarks reads its matching API and common JSON
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn bookmarks -n 1 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `GET /api/v1/bookmarks` exactly 1 time(s)

### Scenario: lists reads its matching API and common JSON
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn lists -n 1 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `GET /api/v1/lists` exactly 1 time(s)

### Scenario: list 9 reads its matching API and common JSON
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn list 9 -n 1 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `GET /api/v1/timelines/list/9` exactly 1 time(s)

### Scenario: search bob --accounts reads its matching API and common JSON
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn search bob --accounts -n 1 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `GET /api/v2/search` exactly 1 time(s)

### Scenario: chat reads its matching API and common JSON
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn chat -n 1 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `GET /api/v1/conversations` exactly 1 time(s)

### Scenario: chat sends a direct status with idempotency key
#### Given
- Stub HTTP server `api` serves 3 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn chat bob hello --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `POST /api/v1/statuses` exactly 1 time(s)
- stdout at `$.id` equals `43`; at `$.text` equals `@bob hello`

### Scenario: post quote and dry run resolve the quote ID
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn post comment --quote 42 --dry-run --host ${api.url} --json
mstdn post comment --quote 42 --host ${api.url} --json
```

#### Then
- after `mstdn post comment --quote 42 --dry-run --host ${api.url} --json`:
  - exit code is `0`
  - stderr is empty
  - stdout at `$.post.quoteId` equals `42`
  - mock `api` received `POST /api/v1/statuses` exactly 0 time(s)
- after `mstdn post comment --quote 42 --host ${api.url} --json`:
  - exit code is `0`
  - stdout at `$.action` equals `post`
  - stderr is empty
  - mock `api` received `POST /api/v1/statuses` exactly 1 time(s)

### Scenario: notifications seen writes exactly the latest read marker
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn notifications --seen -n 1 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- mock `api` received `POST /api/v1/markers` exactly 1 time(s)

### Scenario: interactive report decline sends no request
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
# interactive (pty): mstdn report 42 --reason spam --host ${api.url} --json
```

#### Then
- exit code is `1`
- mock `api` received `POST /api/v1/reports` exactly 0 time(s)

### Scenario: received quote exposes quoteId and boost preserves it
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn status 43 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- stdout at `$.repostOf.quoteId` equals `41`

### Scenario: chat history excludes public posts and other participants
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Environment variables are set: MSTDN_CACHE_DIR, MSTDN_CONFIG_DIR, MSTDN_TOKEN.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
mstdn chat bob -n 1 --host ${api.url} --json
```

#### Then
- exit code is `0`
- stderr is empty
- stdout at `$.id` equals `42`
- mock `api` received `POST /api/v1/statuses` exactly 0 time(s)
