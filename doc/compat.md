# Mastodon API compatibility

The binaries remain `bsky`, `msky`, `mstdn` and `nstr`. These results concern mstdn and disposable servers published only on `127.0.0.1`; no public social server or saved user configuration is used. The shared acceptance catalog is [compat-tests.md](compat-tests.md).

## Evidence and matrix

`supported` means the stated client operation passed against the version shown, `partial` means only the described subset is verified, `unsupported` requires explicit capability evidence, and `not verified` means there is no live result. A documented API is not evidence that this client passed. Passing the implemented scenarios does not certify the entire shared Usable tier: ID-set comparisons and some screen observations still need atago features.

| Service | Client | Verified server version | Auth | Timeline | Search | Post | Reactions | Notifications | Pictures and videos | Known limits | Last verified date |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| GoToSocial | mstdn | 0.19.0+git-ab2044e | supported | partial | partial | partial | supported | supported | partial | Quotes unsupported; client accepts four pictures versus six advertised; alt text uses the client's 1500-character fallback | 2026-11-28 |
| Mastodon | mstdn | 4.5.0 | supported | partial | partial | supported | supported | supported | partial | No Elasticsearch in this deployment; unsupported-codec source is transcoded; federation not verified | 2026-11-28 |
| Pleroma | mstdn | not verified | not verified | not verified | not verified | not verified | not verified | not verified | not verified | Handwritten instance and NodeInfo fixtures only | not verified |
| Akkoma | mstdn | not verified | not verified | not verified | not verified | not verified | not verified | not verified | not verified | Handwritten instance and NodeInfo fixtures only | not verified |

The API says: Mastodon documents [instance configuration](https://docs.joinmastodon.org/entities/Instance/), [status methods](https://docs.joinmastodon.org/methods/statuses/), [media](https://docs.joinmastodon.org/methods/media/) and [quote approval](https://docs.joinmastodon.org/entities/Status/#quote_approval). GoToSocial documents its [0.19.0 API](https://docs.gotosocial.org/en/v0.19.0/api/swagger/), [OAuth procedure](https://docs.gotosocial.org/en/v0.19.0/api/authentication/) and [admin CLI](https://docs.gotosocial.org/en/v0.19.0/admin/cli/). These are contract references, separate from the observations below.

The suite checked: token identity; home, local and public timelines; a 100-post author feed spanning multiple pages with oldest/newest endpoints and repeat-read equality; text, reply and thread reads; four visibility values and content warnings; pictures with alt text and download dimensions; H.264 upload; hashtag and TUI account search; mention, follow, favourite and boost notifications; TUI composition, deletion, favourite/boost and their reversal; Mastodon quote creation; GoToSocial quote refusal and continued navigation. Favourite, boost and delete use existing TUI keys because there are no corresponding CLI commands. Replies addressed to Alice appear as mention notifications.

The heavy checks include 100 video-only posts, 100 posts with four pictures, 50-deep and 200-reply threads, at least 500 paged notifications, text and alt-text boundaries, four pictures and the client's fifth-picture refusal, Unicode preservation in JSON, and kitty scrolling down and back across both 100-post feeds. Each move checks the selected post marker and expected text, with at least one active image; this does not prove that every visible attachment has its own correctly positioned thumbnail. GoToSocial preserves the VP9 WebM source, allowing a viewer warning and save check; Mastodon converts it to MP4, so that unsupported-codec scenario is skipped there.

## Reproducing the runs

Install Rust, Docker with Compose supporting `!override`, Python 3.10 or later, curl, ffmpeg with libx264/libvpx-vp9, and atago. Local validation used atago v0.27.0; the scheduled workflow follows the workspace's existing v0.27.1 pin. The scripts build mstdn with the workspace `e2e` Cargo profile.

```sh
scripts/compat/gotosocial.sh
scripts/compat/mastodon.sh
python3 scripts/compat/report.py target/compat/gotosocial
python3 scripts/compat/report.py target/compat/mastodon
```

GoToSocial is pinned to the official `superseriousbusiness/gotosocial:0.19.0` image and digest in `gotosocial.sh`, using SQLite and port 18080. Mastodon uses the official [v4.5.0 compose file](https://github.com/mastodon/mastodon/blob/v4.5.0/docker-compose.yml), checked by SHA-256, and the official v4.5.0 image pinned by digest, with PostgreSQL 14 Alpine, Redis 7 Alpine, web and Sidekiq on port 13000. PostgreSQL and Redis are also pinned to the digests used in these runs. Both use separate throwaway volumes and federation allowlists with no allowed external domains. Streaming, email delivery and federation are not exercised.

Each script creates Alice, Bob and Videos. GoToSocial uses `admin account create` and promotes Alice for its documented emoji API. Mastodon uses `tootctl accounts create --confirmed --approve`, with Alice as Owner. Passwords and application credentials are generated per run. `server.py` registers applications, submits each local server's sign-in and authorization forms with cookies and hidden fields, extracts its out-of-band authorization code, and exchanges it at `/oauth/token`. Neither script relies on an undocumented password grant, database token insertion or a real browser. Tokens live only in the environment and the mode-0700 temporary run directory; reports contain no passwords or access tokens.

The Mastodon override configures HTTP for the loopback test deployment, permits the throwaway email domain without DNS mail checks, disables Rack::Attack, and raises the status limiter from 300 to 10000 per three hours so seeding can finish. GoToSocial's request-rate limiter is disabled. These are test deployment settings, not claims about production rate-limit behavior; all data seeding still uses public HTTP APIs. Both scripts remove containers, volumes, networks and private temporary files on success or failure.

`seed.py` makes 968 posts on GoToSocial and 967 on Mastodon, deleting one in each run before testing. It prints counts, saves a completed-run manifest to avoid reseeding the same run, and records actual instance configuration, NodeInfo software, codec and verification time. The extra GoToSocial post contains an emoji created through its documented admin API. Mastodon has no equivalent public emoji-creation API, so Mastodon custom emoji remain not verified. The 100-video and 100-four-picture feeds use separate tags, with the Videos account containing only its 100 video posts.

`run.sh` rejects non-loopback origins, disables proxies, gives mstdn an empty temporary configuration directory, and executes native atago commands and PTY interactions sequentially. Reports and the public seed manifest are copied to `target/compat/<system>/`; `MSTDN_COMPAT_REPORT_DIR` overrides that location. `MSTDN_COMPAT_FILTER` selects scenario-name substrings for diagnosis; a filtered report cannot certify a whole tier. The report helper prints tier outcome counts and speed scenario durations; the feature matrix remains manual because counts cannot establish coverage of missing catalog checks.

The GoToSocial workflow is configured for nightly runs and `workflow_dispatch`, with actions pinned by SHA. It has not been executed on GitHub from this checkout. It is not a pull-request workflow or a required check.

## Unsupported features and recovery

[Exit status 5](cli-surface/exit-status.md) means an unsupported server feature, shared as a meaning by all four clients. mstdn implements it here. Statuses 1 (usage), 2 (terminal), 3 (local I/O) and 4 (network/server, including HTTP authentication and permission refusals) retain their meanings. A separate value lets callers distinguish an absent feature from credentials, permissions, outages and missing objects without changing the JSON error keys (`error`, `hint`, `status`).

HTTP 501 is unsupported. A 404 is classified as unsupported only for known collection/capability endpoints: home/public timelines, notifications, search, media upload, conversations, lists, followed tags and editable account credentials. A status lookup, a hashtag lookup, a creation/report request that can reference another resource, or an action on a particular resource keeps 404 as status 4: an absent, private, blocked or deleted object does not establish an absent endpoint. HTTP 401, 403, 422, 429 and other 5xx statuses retain their existing error classes.

Unsupported endpoint diagnostics name the feature and server. Public instance metadata and `/.well-known/nodeinfo` add software identity when available. NodeInfo discovery links are followed only on the configured origin, without authorization headers or redirects. Discovery failure does not replace the original error. Pleroma/Akkoma feature arrays are not treated as exhaustive Mastodon capability lists.

Quote posting no longer compares the instance's Mastodon-like version string with 4.5. The client checks the documented `quote_approval` field of a successfully fetched status, or the authenticated account's `source.quote_policy` when changing the policy of a new post, before uploads or writes. Authentication and resource errors during that probe propagate normally. The TUI applies the same status-field check when opening a quote composer. An explicit attachment limit of zero is unsupported; a missing limit keeps the existing defaults. Missing editable profile source fields close that editor with a feature-and-server message.

Unsupported reactions restore optimistic state and clear the pending operation. A refused composer retains its draft and clears its sending state. Unsupported pagination stops automatic fetching until explicit refresh. Duplicate worker results do not repeat an error. Retry notices now travel from workers to the TUI message area instead of writing to stderr and corrupting the screen; a progress notice does not consume the pending job. CLI retry notices still go to stderr.

The byte ceilings are exclusive: Mastodon [validates with `less_than`](https://github.com/mastodon/mastodon/blob/v4.5.0/app/models/media_attachment.rb), and GoToSocial rejects a reader that reaches its configured limit. Live uploads at the advertised values returned HTTP 422 on both servers. mstdn now rejects that boundary before upload and names the ceiling; the suite uploads a valid padded H.264 MP4 one byte below it and checks rejection at the ceiling. Text and attachment-count limits remain inclusive.

## Timing and catalog gaps

The four CLI speed assertions bound the run step at 10 seconds for a 100-post timeline, 100 notifications and hashtag search, and 30 seconds for a picture post. The JSON report exposes scenario durations, which include assertions and runner overhead; these are not isolated HTTP timings. First screen/image checks and each heavy movement have their own 10-second deadline. Each complete heavy scrolling session has a 480-second limit, independently of those per-move deadlines. Server restart recovery allows a longer wait because an in-flight read may already be honoring Retry-After.

Measured values are recorded below from the final retained reports.

| Scenario | Bound | GoToSocial (ms) | Mastodon (ms) |
| --- | --- | --- | --- |
| speed: timeline under 10s | 10000 ms | 11 | 288 |
| speed: notifications under 10s | 10000 ms | 11 | 218 |
| speed: search under 10s | 10000 ms | 38 | 223 |
| speed: picture post under 30s | 30000 ms | 17 | 101 |
| heavy: scroll compatvideo down and back with kitty | 480000 ms per PTY session | 80473 | 107648 |
| heavy: scroll compatpictures down and back with kitty | 480000 ms per PTY session | 219455 | 186110 |

These are single runs on this Linux host while both local compatibility deployments were active; they are not comparative benchmarks. Individual screen-match times are not exposed by the report, so their deadlines are recorded instead of invented measurements.

| Implemented scenarios by tier | GoToSocial passed / skipped | Mastodon passed / skipped |
| --- | --- | --- |
| usable | 19 / 1 | 19 / 1 |
| heavy | 12 / 0 | 11 / 1 |
| speed | 4 / 0 | 4 / 0 |
| behavior | 4 / 0 | 4 / 0 |

All 40 implemented scenarios were selected on each server, with zero failures or errors: 39 passed and one skipped on GoToSocial; 38 passed and two skipped on Mastodon. The positive quote scenario is inapplicable to GoToSocial, whose negative capability scenario passes. The negative quote scenario is inapplicable to Mastodon; its positive scenario passes. Mastodon also skips the preserved-WebM case because the uploaded source becomes MP4. These counts do not certify missing catalog checks.

The full shared Usable, Heavy and Behavior tiers remain partial rather than certified. Words/full-text search, federation-only behavior, expired tokens on systems issuing non-expiring tokens, all thumbnails' placement, Unicode cell boundaries and frame-rate/memory claims are not inferred from passing checks. Native server rate-limit configuration and permanently unfinished media processing are not exercised by the live suite; TCP fixtures cover Retry-After/reset handling, read retries, write non-retry and processing failures separately. Inline timeline video playback is not an implemented mstdn feature.

The following are proposed atago features, not accepted spec keys in the current version; no shell or Python assertion driver substitutes for them.

| Proposed feature | Required behavior | Catalog checks needing it |
| --- | --- | --- |
| `stdout.jsonl` with `unique_by`, `ordered_by` and `equals_set` | Parse all JSONL records and compare projected IDs with stored records or the seed manifest | Exact deduplication, no gaps across pages and after refresh |
| `run.skip_exit_codes` with a JSON reason selector | Skip only exit 5, record the feature/server message, and fail all other unexpected exits | Generic capability-based skipping without hiding auth/network errors |
| `expect_screen.images` placement and distinct-frame assertions | Associate images with visible post cells and distinguish a decoded video frame from its thumbnail | Every visible thumbnail, first video frame, frame rate and repeated playback |
| Per-step/per-expect timing fields in the JSON report | Record start-to-match latency for each named screen expectation | Measured first paint, first picture/video frame and each movement, rather than only bounds |
| `expect_screen.layout` cell-boundary assertions | Check grapheme display widths and protected hint/border rows | ZWJ, combining, CJK and RTL layout under resizing |
| A forwarding HTTP observer/fault controller | Observe requests to a real server, count writes, delay media completion and inject controlled read failures without replacing server responses | Live idempotency, Retry-After/reset timing and processing timeout checks |
| `pty.session.repeat` with an index | Repeat send/expect pairs with an indexed expected label | Compact maintenance of the currently explicit 100-post scroll scripts |

## Fixtures and validation

`testdata/compat/servers.json` contains reduced handwritten Mastodon, GoToSocial, Pleroma and Akkoma instance/NodeInfo shapes. The live runs confirm Mastodon 4.5.0 and GoToSocial 0.19.0 instance configuration and NodeInfo software objects. The GoToSocial fixture's `4.2.0 (compatible; GoToSocial 0.19.0)` string remains a deliberate compatibility-version example: this image actually reports `0.19.0+git-ab2044e`. Pleroma/Akkoma version strings, v1/v2 availability, discovery links, schema versions and extension feature arrays still require live checking. The Akkoma `quote_posting` example does not imply that mstdn supports its quoting extension.

Workspace build, tests including TCP tests, clippy with warnings denied, formatting, rustdoc and `just rules` were run. The existing mstdn mock/PTY suite was also run. Reports distinguish these checks from the local live-server suites; there are no socket-only failures or exclusions in this environment. The default workspace run passed 2072 tests and retained 13 existing ignored tests: ten measurements, one comparison tool and two tests fetching public Bluesky videos. The public-video tests were not enabled under this task's network restriction.
