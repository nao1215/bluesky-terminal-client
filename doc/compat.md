# Compatibility results

The binaries remain `bsky`, `msky`, `mstdn` and `nstr`. These results concern mstdn, msky and nstr with disposable servers published only on `127.0.0.1`; no public social server or saved user configuration is used. The shared acceptance catalog is [compat-tests.md](compat-tests.md), and the generated [scenario index](e2e/) separates mock E2E from live compatibility tests.

## mstdn results

### Evidence and matrix

`supported` means the stated client operation passed against the version shown, `partial` means only the described subset is verified, `unsupported` requires explicit capability evidence, and `not verified` means there is no live result. A documented API is not evidence that this client passed. Passing the implemented scenarios does not certify the entire shared Usable tier: ID-set comparisons and some screen observations still need atago features.

| Service | Client | Verified server version | Auth | Timeline | Search | Post | Reactions | Notifications | Pictures and videos | Known limits | Last verified date |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| GoToSocial | mstdn | 0.19.0+git-ab2044e | supported | partial | partial | partial | supported | supported | partial | Quotes unsupported; client accepts four pictures versus six advertised; alt text uses the client's 1500-character fallback | 2026-11-28 |
| Mastodon | mstdn | 4.5.0 | supported | partial | partial | supported | supported | supported | partial | No Elasticsearch in this deployment; unsupported-codec source is transcoded; federation not verified | 2026-11-28 |
| Pleroma | mstdn | 2.11.0-develop (v2.11.0 image) | supported | partial | partial | supported | supported | supported | partial | Multipart request budget; four-picture client ceiling versus 1000 advertised; quote approval policies unsupported | 2026-11-28 |
| Akkoma | mstdn | 3.20.0-v3-20-0 | supported | partial | partial | supported | supported | supported | partial | Multipart request budget; four-picture client fallback; quote approval policies unsupported | 2026-11-28 |

The API says: Mastodon documents [instance configuration](https://docs.joinmastodon.org/entities/Instance/), [status methods](https://docs.joinmastodon.org/methods/statuses/), [media](https://docs.joinmastodon.org/methods/media/) and [quote approval](https://docs.joinmastodon.org/entities/Status/#quote_approval). GoToSocial documents its [0.19.0 API](https://docs.gotosocial.org/en/v0.19.0/api/swagger/), [OAuth procedure](https://docs.gotosocial.org/en/v0.19.0/api/authentication/) and [admin CLI](https://docs.gotosocial.org/en/v0.19.0/admin/cli/). These are contract references, separate from the observations below.

The suite checked: token identity; home, local and public timelines; a 100-post author feed spanning multiple pages with oldest/newest endpoints and repeat-read equality; text, reply and thread reads; four visibility values and content warnings; pictures with alt text and download dimensions; H.264 upload; hashtag and TUI account search; mention, follow, favourite and boost notifications; TUI composition, deletion, favourite/boost and their reversal; Mastodon quote creation; GoToSocial quote refusal and continued navigation. Favourite, boost and delete use existing TUI keys because there are no corresponding CLI commands. Replies addressed to Alice appear as mention notifications.

The heavy checks include 100 video-only posts, 100 posts with four pictures, 50-deep and 200-reply threads, at least 500 paged notifications, text and alt-text boundaries, four pictures and the client's fifth-picture refusal, Unicode preservation in JSON, and kitty scrolling down and back across both 100-post feeds. Each move checks the selected post marker and expected text, with at least one active image; this does not prove that every visible attachment has its own correctly positioned thumbnail. GoToSocial preserves the VP9 WebM source, allowing a viewer warning and save check; Mastodon converts it to MP4, so that unsupported-codec scenario is skipped there.

### Reproducing the runs

Install Rust, Docker with Compose supporting `!override`, Python 3.10 or later, curl, ffmpeg with libx264/libvpx-vp9, and atago. Local validation used atago v0.27.0; CI and documentation generation pin atago v0.28.0. The scripts build mstdn with the workspace `e2e` Cargo profile.

```sh
scripts/compat/gotosocial.sh
scripts/compat/mastodon.sh
scripts/compat/akkoma.sh
scripts/compat/pleroma.sh
python3 scripts/compat/report.py target/compat/gotosocial
python3 scripts/compat/report.py target/compat/mastodon
python3 scripts/compat/report.py target/compat/akkoma
python3 scripts/compat/report.py target/compat/pleroma
```

GoToSocial is pinned to the official `superseriousbusiness/gotosocial:0.19.0` image and digest in `gotosocial.sh`, using SQLite and port 18080. Mastodon uses the official [v4.5.0 compose file](https://github.com/mastodon/mastodon/blob/v4.5.0/docker-compose.yml), checked by SHA-256, and the official v4.5.0 image pinned by digest, with PostgreSQL 14 Alpine, Redis 7 Alpine, web and Sidekiq on port 13000. PostgreSQL and Redis are also pinned to the digests used in these runs. Both use separate throwaway volumes and federation allowlists with no allowed external domains. Streaming, email delivery and federation are not exercised.

Each script creates Alice, Bob and Videos. GoToSocial uses `admin account create` and promotes Alice for its documented emoji API. Mastodon uses `tootctl accounts create --confirmed --approve`, with Alice as Owner. Passwords and application credentials are generated per run. `server.py` registers applications, submits each local server's sign-in and authorization forms with cookies and hidden fields, extracts its out-of-band authorization code, and exchanges it at `/oauth/token`. Neither script relies on an undocumented password grant, database token insertion or a real browser. Tokens live only in the environment and the mode-0700 temporary run directory; reports contain no passwords or access tokens.

The Mastodon override configures HTTP for the loopback test deployment, permits the throwaway email domain without DNS mail checks, disables Rack::Attack, and raises the status limiter from 300 to 10000 per three hours so seeding can finish. GoToSocial's request-rate limiter is disabled. These are test deployment settings, not claims about production rate-limit behavior; all data seeding still uses public HTTP APIs. Both scripts remove containers, volumes, networks and private temporary files on success or failure.

`seed.py` makes 968 posts on GoToSocial and 967 on Mastodon, deleting one in each run before testing. It prints counts, saves a completed-run manifest to avoid reseeding the same run, and records actual instance configuration, NodeInfo software, codec and verification time. The extra GoToSocial post contains an emoji created through its documented admin API. Mastodon has no equivalent public emoji-creation API, so Mastodon custom emoji remain not verified. The 100-video and 100-four-picture feeds use separate tags, with the Videos account containing only its 100 video posts.

`run.sh` rejects non-loopback origins, disables proxies, gives mstdn an empty temporary configuration directory, and executes native atago commands and PTY interactions sequentially. Reports and the public seed manifest are copied to `target/compat/<system>/`; `MSTDN_COMPAT_REPORT_DIR` overrides that location. `MSTDN_COMPAT_FILTER` selects scenario-name substrings for diagnosis; a filtered report cannot certify a whole tier. The report helper prints tier outcome counts and speed scenario durations; the feature matrix remains manual because counts cannot establish coverage of missing catalog checks.

The workflow configuration and report regeneration command are described below; historical measurements in this section come from local runs.

### Unsupported features and recovery

[Exit status 5](cli-surface/exit-status.md) means an unsupported server feature, shared as a meaning by all four clients. mstdn implements it here. Statuses 1 (usage), 2 (terminal), 3 (local I/O) and 4 (network/server, including HTTP authentication and permission refusals) retain their meanings. A separate value lets callers distinguish an absent feature from credentials, permissions, outages and missing objects without changing the JSON error keys (`error`, `hint`, `status`).

HTTP 501 is unsupported. A 404 is classified as unsupported only for known collection/capability endpoints: home/public timelines, notifications, search, media upload, conversations, lists, followed tags and editable account credentials. A status lookup, a hashtag lookup, a creation/report request that can reference another resource, or an action on a particular resource keeps 404 as status 4: an absent, private, blocked or deleted object does not establish an absent endpoint. HTTP 401, 403, 422, 429 and other 5xx statuses retain their existing error classes.

Unsupported endpoint diagnostics name the feature and server. Public instance metadata and `/.well-known/nodeinfo` add software identity when available. NodeInfo discovery links are followed only on the configured origin, without authorization headers or redirects. Discovery failure does not replace the original error. Pleroma/Akkoma feature arrays are not treated as exhaustive Mastodon capability lists.

Quote posting does not compare the instance's Mastodon-like version string with 4.5. The client accepts the documented `quote_approval` field on a fetched status, or the positive `quote_posting` extension in instance metadata; it sends `quote_id` for the latter contract. Changing a post's approval policy still requires the authenticated account's `source.quote_policy`, independently of quote creation. These checks precede uploads and writes, and authentication/resource errors propagate. The TUI uses the same capabilities, reads Akkoma's top-level quote and Pleroma's nested quote, and retains a draft when an approval policy is unsupported. An explicit attachment limit of zero is unsupported; a missing limit keeps the existing defaults. Missing editable profile source fields close that editor with a feature-and-server message.

Unsupported reactions restore optimistic state and clear the pending operation. A refused composer retains its draft and clears its sending state. Unsupported pagination stops automatic fetching until explicit refresh. Duplicate worker results do not repeat an error. Retry notices now travel from workers to the TUI message area instead of writing to stderr and corrupting the screen; a progress notice does not consume the pending job. CLI retry notices still go to stderr.

Mastodon and GoToSocial byte ceilings are exclusive: Mastodon [validates with `less_than`](https://github.com/mastodon/mastodon/blob/v4.5.0/app/models/media_attachment.rb), and GoToSocial rejects a reader that reaches its configured limit. Live uploads at the advertised values returned HTTP 422 on both servers. mstdn now rejects that boundary before upload and names the ceiling; the suite uploads a valid padded H.264 MP4 one byte below it and checks rejection at the ceiling. Text and attachment-count limits remain inclusive.

### Timing and catalog gaps

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

The earlier 40-scenario suite was selected in full on GoToSocial and Mastodon, with zero failures or errors: 39 passed and one skipped on GoToSocial; 38 passed and two skipped on Mastodon. The positive quote scenario is inapplicable to GoToSocial, whose negative capability scenario passes. The negative quote scenario is inapplicable to Mastodon; its positive scenario passes. Mastodon also skips the preserved-WebM case because the uploaded source becomes MP4. These counts do not certify missing catalog checks.

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

### Fixtures and validation

`testdata/compat/servers.json` retains the reduced handwritten examples. `testdata/compat/akkoma-live.json` and `pleroma-live.json` contain actual local HTTP bodies, statuses and pagination headers captured by `capture.py` from the generated accounts, with no OAuth responses or tokens. Tests cover v1/v2 instance shapes, NodeInfo identity, URL counting, media budgets, video-as-preview URLs, quote creation and rendering, rendered Markdown/MFM, extension reactions, pagination and error classification. Mastodon and GoToSocial retain their earlier live evidence; the handwritten GoToSocial compatibility-version example remains distinct from its image's actual `0.19.0+git-ab2044e` response.

Workspace build, tests including TCP tests, clippy with warnings denied, formatting, rustdoc and `just rules` were run. For this run, `just rules` used the same pinned jscpd 5.4.3 in its container image with networking disabled, avoiding an npm download outside the permitted network scope. The existing mstdn mock/PTY suite passed all 61 scenarios. Reports distinguish these checks from the local live-server suites; there are no socket-only failures or exclusions in this environment. The default workspace run passed 2089 tests and retained 13 existing ignored tests: ten measurements, one comparison tool and two tests fetching public Bluesky videos. The public-video tests were not enabled under this task's network restriction.

### Akkoma and Pleroma

The deployments pin Akkoma's v3.20.0 amd64-musl OTP archive by SHA-256 and Pleroma's official v2.11.0 container by digest. Their NodeInfo software versions are `3.20.0-v3-20-0` and `2.11.0-develop`; the Pleroma image's release tag does not change its reported build string. Akkoma runs the official OTP archive inside the project's digest-pinned `akkoma/ci-base:1.15-otp25` image, which supplies ffmpeg and ImageMagick. This is a local runtime wrapper around the official artifacts, not a claim that Akkoma publishes a complete production server image. The wrapper targets linux/amd64 and builds with networking disabled, using no Hex or OS package downloads. The upstream references are [Akkoma OTP installation](https://docs.akkoma.dev/stable/installation/otp_en/), [Akkoma's CI image](https://akkoma.dev/AkkomaGang/akkoma/src/tag/v3.20.0/.woodpecker/test.yml) and [Pleroma's container publishing configuration](https://git.pleroma.social/pleroma/pleroma/src/tag/v2.11.0/.woodpecker/docker-combine.yaml).

Both entry scripts call `pleroma-family.sh`, then the existing `server.py`, `seed.py` and `run.sh`; the shared catalog and spec directory are reused. PostgreSQL 14 Alpine and nginx 1.28.0 Alpine are also digest-pinned. The database and application join an internal Docker network with no outgoing route. A fixed-upstream nginx proxy publishes only `127.0.0.1:14000` for Akkoma or `127.0.0.1:14001` for Pleroma. Federation, mail, rich-media fetching and timezone updates are disabled; documented request limiters are disabled for seeding. The restart scenario stops and restarts both application and proxy so connection failure is observable while the actual server restarts. Containers, anonymous data volumes, networks and private temporary directories are removed on success and failure; downloaded images remain cached.

`pleroma_ctl user new --password ... --assume-yes` creates confirmed Alice, Bob and Videos accounts, with Alice as administrator. The helper registers a local OAuth application and uses each server's own password-grant implementation at `/oauth/token`, with `read write` scopes. Credentials are generated only for these disposable users; no database token insertion, public account or saved user configuration is involved. This flow is implemented by the releases' [Akkoma OAuth controller](https://akkoma.dev/AkkomaGang/akkoma/src/tag/v3.20.0/lib/pleroma/web/oauth/oauth_controller.ex) and [Pleroma OAuth controller](https://git.pleroma.social/pleroma/pleroma/src/tag/v2.11.0/lib/pleroma/web/oauth/oauth_controller.ex). Account creation output is suppressed because the admin tool prints the password. Docker also receives an empty, private configuration directory.

The suite checked: authentication; home/local/public timelines and a 100-post paged author feed; replies and threads; favourites and boosts with reversal; all four visibilities and content warnings; image alt text, H.264 uploads and the preserved WebM viewer warning; notification kinds and at least 500 notifications; hashtag/account search; TUI composition, deletion, quote creation and quote-target readback; recovery after server restart; public-status reads while blocked; and the shared heavy and speed scenarios. Seeding creates 970 posts, deletes one, and additionally captures Markdown/MFM posts, a quoting extension and one emoji reaction through the local APIs. These extra captures do not imply that mstdn can create formatted posts or emoji reactions.

The API says: both projects document [Akkoma's Mastodon API differences](https://docs.akkoma.dev/stable/development/API/differences_in_mastoapi_responses/) and [Pleroma's Mastodon API differences](https://docs.pleroma.social/backend/development/API/differences_in_mastoapi_responses/); Pleroma also documents a separate [Chats API](https://docs.pleroma.social/backend/development/API/chats/). Instance metadata advertises `quote_posting` and supported post formats. The local responses advertise and retain emoji reactions, Markdown and MFM content types; mstdn reads their rendered HTML but has no emoji-reaction operation, content-type selector or MFM animation. Pleroma advertises `pleroma_chat_messages` and returns an empty chat list; Akkoma returns 404 for that extension endpoint. Both return a Mastodon conversation containing the seeded direct status, and fixture tests decode it. The TUI's Chat tab uses Mastodon conversations; Pleroma chat-message sending was not tested or added.

The live instance answers publish 5000 text characters, 5000 alt-text characters and a 16000000-byte upload budget. Akkoma's v2 instance route returns 404, and its v1 response carries these limits at the top level. Pleroma's v2 response puts alt-text limits in `pleroma.metadata`, reserves zero characters for URLs and advertises 1000 attachments. mstdn reads those shapes, treats zero URL reservation as counting the original text, and keeps its four-picture client ceiling; Akkoma does not advertise an attachment count in this response, so its four-picture value is a client fallback. The suite checks the text and alt-text boundaries, four pictures and the client's fifth-picture refusal; it does not claim an Akkoma server attachment ceiling or a 1000-attachment Pleroma upload.

Both servers also apply their upload budget to the complete multipart body. A file of 16000000 bytes therefore receives HTTP 413 once framing is added. mstdn calculates the framing, escaped filename and UTF-8 alt text before upload, and rejects requests above the advertised budget locally. The suite pads a valid MP4 to the largest payload fitting that request, then checks a request one byte over. This differs from Mastodon/GoToSocial's exclusive file-byte ceilings. Captured oversized text receives HTTP 422 with an `error` string; oversized uploads use an `errors.detail` body and HTTP 413. Wrong tokens return 403 and deleted statuses 404. These retain exit status 4; metadata shape or error wording does not turn credentials, size failures or missing resources into unsupported features.

The video preview URL is the original MP4 on both deployments. The shared image loader now decodes one H.264 frame through the existing player code and keeps the image loader's 10 MiB download bound. This fixes the seeded video thumbnails without adding a server-name branch. Larger original videos and unsupported codecs do not acquire unlimited thumbnail decoding. Fast movement at a page boundary also revealed a lost down key while the next page was in flight; mstdn now retains that movement until the page arrives, cancels it on an explicit selection or reverse movement, and ignores stale responses.

Quote creation uses the positive `quote_posting` feature and the supported `quote_id` parameter; approval-policy changes still require Mastodon's separate policy field. Akkoma supplies a top-level quote object, while Pleroma nests it under `pleroma`; both render through the existing quote view. The suite verifies the stored quote target, and a rejected approval policy preserves the draft so it can be sent after clearing that policy. No software-name dispatch was added to mstdn: instance fields and positive extension markers select these contracts, while NodeInfo remains diagnostic identity. The non-exhaustive feature list never establishes absence of unrelated Mastodon features.

The pagination fixtures retain alphanumeric IDs and Link headers with extra parameters. The CLI and TUI use the returned cursor rather than treating IDs as numbers. Both servers leave the second account's public status readable after that account blocks the reader; the suite checks that behavior separately from Mastodon/GoToSocial's hidden-resource case. Blocking in fixture capture is undone and the seed follows are restored before the suite starts.

Both complete bootstrap scripts exited successfully on 2026-11-28. Each selected all 42 scenarios: 40 passed, two skipped, and none failed or errored. The skipped scenarios require either absent quote support or hidden public statuses after blocking; the corresponding positive quote and public-readable block scenarios passed. The separate negative quote-approval-policy scenario passed on both. These results cover the implemented scenarios, with the shared catalog gaps above still applicable.

| Tier | Akkoma passed / skipped | Pleroma passed / skipped |
| --- | --- | --- |
| usable | 20 / 1 | 20 / 1 |
| heavy | 12 / 0 | 12 / 0 |
| speed | 4 / 0 | 4 / 0 |
| behavior | 4 / 1 | 4 / 1 |

| Scenario | Bound | Akkoma (ms) | Pleroma (ms) |
| --- | --- | --- | --- |
| speed: timeline under 10s | 10000 ms | 739 | 419 |
| speed: notifications under 10s | 10000 ms | 734 | 473 |
| speed: search under 10s | 10000 ms | 548 | 364 |
| speed: picture post under 30s | 30000 ms | 1049 | 44 |
| heavy: scroll compatvideo down and back with kitty | 480000 ms per PTY session | 33503 | 36905 |
| heavy: scroll compatpictures down and back with kitty | 480000 ms per PTY session | 17964 | 17536 |
| Complete atago suite | Per-scenario bounds above | 91476 | 76618 |

These are scenario durations from the retained `target/compat/akkoma/atago.json` and `target/compat/pleroma/atago.json`, including runner and assertion overhead. The complete-suite duration excludes image acquisition, database initialization, account creation and seeding. The Linux host also ran other local builds and containers; these single observations are not comparative benchmarks. First-screen and image checks use the shared ten-second deadlines, with no invented per-frame measurements.

## msky results

Checked on 2026-11-28 with msky 0.1.1 from this workspace, Misskey 2025.12.0 and Sharkey 2025.4.3, Linux amd64, Docker Engine 29.2.1, Compose 5.0.2 and atago 0.27.0.

### Running the catalog

Run from the workspace root with Docker Compose, Cargo, Python 3, ffmpeg and atago on PATH:

```sh
./scripts/compat/misskey.sh
./scripts/compat/sharkey.sh
```

Set `MSKY_REPORT_DIR=/tmp/msky-reports` to retain atago JSON reports, or `MSKY_CAPTURE_DIR=/tmp/msky-captures` to retain non-secret setup responses for fixture review; no credentials are committed or needed from the user's configuration.

The runners download each release's official Compose example, use images pinned by digest, replace data binds with disposable named volumes, correct the PostgreSQL 18 mount, initialize the Misskey files volume's ownership, disable automatic restarts and use `/api/meta` for readiness, including after a restart.

Only a fixed-upstream nginx proxy publishes a randomly selected port on 127.0.0.1; the application, database and Redis use an internal network without external egress, and the proxy uses bounded connection and response timeouts for the restart test.

The runner creates the first administrator and two other accounts through `admin/accounts/create`, generates scoped tokens with `miauth/gen-token`, exports `MSKY_HOST` and `MSKY_TOKEN` with an isolated client configuration directory, and removes containers, volumes, networks and temporary files in `finally`, including after a failed suite or SIGTERM.

The three accounts initially follow each other; the picture account later blocks the administrator for the visibility test, so its own token is used for its media timeline and download checks.

Seeding uses only public APIs, enables the documented `sqlLike` search provider and `canSearchNotes` policy, disables rate limits during seeding, and sets the role file limit to 1 MiB for a practical exact-byte boundary test; behavior scenarios subsequently lower the rate limit through the same policy API.

Each fresh run creates 500 text notes and 500 reaction notifications, 100 video-only notes plus a video mention and reply, 100 four-picture notes plus a followers-only four-picture note, a 50-reply chain, a root with 200 direct replies, one-to-four-picture posts, every visibility, content warnings, a local custom emoji, Unicode text and names, and H.264 MP4 and unsupported VP9 WebM clips generated by ffmpeg.

Seed counts are printed before atago starts, asynchronous notifications are paged until all 500 reactions exist, and a completed seed manifest makes repeated seeding within that volume set a no-op; deleting a renote can shorten a notification page, so only an empty page indicates the end.

The checked seed had 509 notifications before client scenarios added more; tests assert counts and returned shapes rather than fixed server IDs.

All client assertions are native atago steps in `crates/misskey-terminal-client/e2e/compat/`, with `usable:`, `heavy:`, `speed:` and `behavior:` scenario prefixes; Python only provisions the servers and seeds data, and no shell or Python checker substitutes for missing atago assertions.

### What the suite checked

Both runner scripts exited 0; each full run contained 57 scenarios, and a focused native atago rerun on the same servers additionally compared all four downloaded PNGs byte-for-byte with the original fixture.

| Tier | Misskey 2025.12.0 | Sharkey 2025.4.3 |
| --- | --- | --- |
| Usable | 16 passed, 3 skipped, 1 xfail | 17 passed, 3 skipped |
| Heavy | 13 passed, 3 skipped | 13 passed, 3 skipped |
| Speed | 7 passed, 1 skipped | 7 passed, 1 skipped |
| Behavior | 10 passed, 3 skipped | 10 passed, 3 skipped |

Misskey totals 46 passed, 10 skipped and one expected failure; Sharkey totals 47 passed and 10 skipped, with no unexpected failure or error in either final run.

Counts describe implemented checks; complete-tier status remains partial because of the explicit gaps below, and an expected failure is not counted as supported behavior.

The usable scenarios cover token identity; home, local and global timelines; 500 hashtag results over multiple pages; text posts, replies, threads and deletion; Unicode and custom emoji reactions and removal; renotes and removal; picture alt text and video uploads; content warnings and public, home and followers visibility; reading an API-seeded specified-visibility note; mention, reply, reaction, renote and follow notifications; downloads; and a Kitty PTY session with browsing, media, account search and composition.

Misskey retains `renoteCount: 1` after deleting a renote even when `notes/renotes` is empty; the live probe selects an explicit expected-failure scenario, msky preserves the server's count, and this discrepancy prevents a claim that the complete usable catalog passed.

Heavy scenarios page the media authors to the end, read 51 notes in the deep thread and 201 in the wide thread, page more than 500 notifications, scroll through 100 video posts and 100 four-picture posts in both directions with bounded screen waits, play H.264, show the WebM decoding explanation and download action, check Unicode text and the hint row, and exercise the exact text, picture-count, alt-text and configured file-size limits plus one over each.

The limit checks use 3000 ASCII characters, 16 pictures, 512 code points of alt text on Misskey or 20000 UTF-16 units on Sharkey, and 1048576 file bytes; rejected media leaves the newest drive file unchanged, while fixture unit tests also verify that preflight rejection sends no upload.

Behavior scenarios check repeatable search output, wrong and revoked tokens, insufficient token permissions, deleted and blocked notes, server restart during a running TUI session, rate-limited read retries and a refused write that creates no note; request-count guarantees remain covered by unit tests and the live tracing gap below.

No tested endpoint was absent on either configured server; missing-endpoint status 5 is covered by structured-error unit tests, while actual authentication, permission, role-policy and blocked-note responses are captured from both servers and remain status 4.

### Measured speeds

The following values are atago JSON `duration_ms` converted to seconds from the final runs; CLI scenarios contain one command and its assertions, while PTY values include startup, navigation and quitting, so they are not isolated first-paint or first-frame measurements.

| Scenario | Bound | Misskey seconds | Sharkey seconds |
| --- | --- | ---: | ---: |
| CLI timeline, 100 notes | 10 s | 0.039 | 0.047 |
| CLI notifications, 100 items | 10 s | 0.065 | 0.052 |
| CLI hashtag search, 100 notes | 10 s | 0.035 | 0.039 |
| CLI picture post | 15 s | 0.297 | 0.053 |
| TUI first populated timeline | 10 s screen wait | 0.268 | 0.269 |
| TUI first picture | 10 s screen wait | 0.495 | 0.494 |
| TUI short video playback | 15 s screen wait | 4.129 | 3.953 |
| 100 video posts, down and back | 3 s per move; 240 s session | 49.286 | 50.129 |
| 100 four-picture posts, down and back | 3 s per move; 240 s session | 50.329 | 47.047 |
| Rate-limited read | 1–45 s command | 30.115 | 1.074 |

The video expectation waits for the two-second clip to finish with a Kitty image delivered; this gives a conservative playback bound, while a separately measured first video frame requires the atago feature below.

The live rate-limit header requested 30 seconds on Misskey and one second on Sharkey, and the retry notice matched that header; the measured successful-read scenarios took 30.115 and 1.074 seconds respectively.

### Client changes verified by fixtures

The shared `terminal-kit::error` module replaces duplicate Error and Kind implementations in bsky, msky and mstdn; nstr keeps its serializable error shape and uses the common formatter and exit mapping, and the shared public API baseline is updated.

msky retains structured API error codes, assigns status 5 only to an explicit missing endpoint, names the feature and host in capability messages, and distinguishes search role policy, token permission, role permission and network failures instead of inferring capability from message substrings or an HTTP 404.

CLI threads now page all direct replies instead of stopping at 100; fixtures preserve two actual 100-reply pages from each server, while the interactive thread view still bounds its initial fetch and opens deeper replies on demand.

Picture attachments use the public `force` upload option because content-hash deduplication otherwise silently keeps a previous file's alt text; the picture limit is the documented 16 files, shared by the CLI and composer.

The client reads Sharkey's configured alt limit from public NodeInfo metadata and counts UTF-16 units for that configured limit; absent metadata retains the standard 512-code-point schema, without a version table.

A failed timeline refresh now reports its error even when cached posts remain on screen, and the subsequent refresh can recover after the server restarts.

A hidden note returned with HTTP 200 and `isHidden: true` by Misskey is now an explicit status-4 refusal naming the host, consistent with Sharkey's HTTP-400 `NO_SUCH_NOTE` response for the same blocked followers-only note.

The response fixtures and their normalization are documented in [response fixtures](../crates/misskey-terminal-client/src/testdata/compat/README.md); local custom reactions use `:compat@.:`, the note's local `reactionEmojis` map is empty, and `/api/emojis` supplies the image on both releases.

### Remaining catalog limits

Neither system is declared fully usable by the entire shared catalog: the ordered, duplicate-free, gap-free ID comparison cannot currently be expressed as a native JSON Lines assertion, and the CLI cannot write specified visibility because it has no recipient flag; those cases are explicitly skipped with their reasons in scenario names.

The recorded pagination checks cover 500 results, the first and last seeded markers, and a repeated read; they do not compare typed ID sets across every captured page, so uniqueness and absence of gaps remain unverified.

The recorded image assertions use `min_count: 1` and selected-note markers; an avatar or another post's thumbnail can satisfy that count, so the checks do not associate each image with its note.

The recorded Unicode checks verify displayed strings and the intact hint row, not screen-region cell boundaries for every grapheme, border and Kitty placeholder.

The recorded atago reports contain scenario durations and enforce screen timeouts; they do not measure each successful screen assertion or distinguish video frames from redraws, so separate first-paint, movement, frame-rate and memory measurements are unavailable.

The rate-limit scenario compares the retry notice to the live `Retry-After` header and bounds the successful read between 1 and 45 seconds; Misskey and Sharkey use different rate-limit algorithms, so the configured factor does not imply the same wait.

Atago 0.27.0 rejected variable interpolation in `assert.duration.gte` at load time during these runs, so a dynamic lower-bound comparison against the captured header was not asserted; measured durations are recorded separately.

The recorded live checks verify the resulting post state and a refused write whose preflight reads may retry; they do not observe and count forwarded HTTP requests. Unit and mock tests check request counts, but no live request-count guarantee is claimed.

Inline timeline video playback is not implemented; video playback is checked in the viewer, and that client limitation is recorded separately from missing atago capabilities.

These uploads do not expose a controllable asynchronous video-processing job, and access tokens have no configured expiration endpoint; bounded pending-processing behavior and natural token expiry are not verified, while explicit token revocation is checked.

Federation, remote emoji, other releases, Meilisearch, object storage, browser-based MiAuth and chat conversations are not verified by this single-server catalog; source documentation alone is not counted as a passing test.

### What the official sources specify

Provisioning follows the release-specific [Misskey Compose example](https://github.com/misskey-dev/misskey/blob/2025.12.0/compose_example.yml) and [Sharkey Compose example](https://activitypub.software/TransFem-org/Sharkey/-/blob/2025.4.3/compose_example.yml), with the officially supported `sqlLike` search provider from their example configurations.

The published schemas and implementations define administrator bootstrap through [Misskey admin/accounts/create](https://github.com/misskey-dev/misskey/blob/2025.12.0/packages/backend/src/server/api/endpoints/admin/accounts/create.ts) and [Sharkey admin/accounts/create](https://activitypub.software/TransFem-org/Sharkey/-/blob/2025.4.3/packages/backend/src/server/api/endpoints/admin/accounts/create.ts), and tokens through [Misskey miauth/gen-token](https://github.com/misskey-dev/misskey/blob/2025.12.0/packages/backend/src/server/api/endpoints/miauth/gen-token.ts) and [Sharkey miauth/gen-token](https://activitypub.software/TransFem-org/Sharkey/-/blob/2025.4.3/packages/backend/src/server/api/endpoints/miauth/gen-token.ts).

Sharkey publishes its configured limits through [NodeInfo metadata](https://activitypub.software/TransFem-org/Sharkey/-/blob/2025.4.3/packages/backend/src/server/NodeinfoServerService.ts) and checks alt length in [drive/files/create](https://activitypub.software/TransFem-org/Sharkey/-/blob/2025.4.3/packages/backend/src/server/api/endpoints/drive/files/create.ts); its [admin/emoji/add](https://activitypub.software/TransFem-org/Sharkey/-/blob/2025.4.3/packages/backend/src/server/api/endpoints/admin/emoji/add.ts) removes account ownership of the emoji file, so seeding uploads a separate image for the emoji.

### Workspace validation

The existing msky `e2e/run.sh` passed all 79 scenarios with its normal environment, including inherited `NO_COLOR`; the scenario starting `s opens the settings, T picks a theme` explicitly clears `NO_COLOR` inside its PTY because it asserts the Terminal color theme, while the application continues to honor `NO_COLOR` everywhere else.

The duplicate rule passes by sharing implementation first and retaining only the incidental msky/mstdn help-text fingerprint with an explanation in `scripts/rules/duplicates.py`; thresholds are unchanged, and the installed jscpd 5.4.3 was used offline to avoid an npm registry request outside this run's permitted network scope.

`cargo build --workspace`, `cargo test --workspace` (2068 passed, 13 ignored), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo doc --no-deps --workspace` and `just rules` passed after the client changes; msky accounts for 590 passing unit tests and two ignored tests.

## nstr results

### Relay behavior

NIP-01 supplies event signatures, filters, kind-0 profiles, kind-1 notes, acknowledgements and subscription messages; NIP-02 supplies kind-3 contacts. NIP-10 identifies replies, NIP-18 and NIP-25 supply reposts and reactions, and NIP-19 supplies encoded identities and event references. Missing profiles produce null metadata, missing contact lists limit Home to the account's own notes, and rejected reposts or reactions produce relay warnings. NIP-36 content warnings and NIP-92 image metadata are read from tags. Hashtag queries use the NIP-01 `#t` filter and match the supplied tag exactly. Text search uses only `searchRelays` and requires advertised NIP-50 support; the relay controls text matching and ranking, and nstr orders returned events newest first. An unadvertised search is reported as unsupported before sending a subscription. NIP-44 v2 encrypts NIP-17 messages and NIP-46 remote signer requests; an incompatible signer fails the write without publishing an unverified event.

NIP-11 information is fetched once per configured relay URL per process, including across full-screen refreshes, with `Accept: application/nostr+json` over HTTP or HTTPS. Redirects and proxies are disabled. Missing, malformed or unavailable information falls back to ordinary NIP-01 behavior without a warning. Fetching uses at most a quarter of the relay operation timeout, capped at two seconds, and shares that operation's deadline. The `limitation` fields constrain operations, and `supported_nips` gates NIP-50 text search; other implemented NIPs need not be advertised.

Reads use one subscription at a time and respect `max_limit`; zero `max_subscriptions` or `max_limit` makes reads unavailable. `max_message_length` bounds outgoing UTF-8 message bytes. `payment_required` makes that relay unavailable because nstr has no payment flow. `auth_required` makes public-key-only reads unavailable. Unavailable read subscriptions, payment flows and authenticated reads use exit 5 when no relay can serve the request. Explicit `unsupported:` NOTICE, CLOSED and negative OK messages also use exit 5. Oversized messages and ordinary relay rejections remain exit 4. Reasons are retained per relay, and any relay supplying usable results allows the operation to succeed with warnings for the others.

NIP-42 challenges received while posting are answered with a kind-22242 event signed by the configured account signer (key file or NIP-46 bunker), with the relay URL, challenge and current timestamp. The client waits for a matching authentication OK and retries an explicitly auth-rejected publication at most once. Repeated challenges and rejected authentication fail that relay. Reads never open a key file and report that the relay requires auth. All relay publications use this handshake, including gift wraps authenticated with the sending account rather than the ephemeral wrapper key. A bunker must permit signing kind 22242 as well as the application event kind.

CLOSED reasons and NOTICE text are retained per relay. Negative OK reasons keep their machine-readable `blocked:`, `rate-limited:`, `invalid:`, `pow:`, `restricted:` or `auth-required:` prefix; nstr does not pay, mine proof of work or automatically retry rate limits. A `duplicate:` response means the relay already has the event and counts as acceptance. Unknown rejection prefixes remain visible as well.

Short pages are followed until exhaustion, the requested count or the deadline, including when the relay caps results without advertising a limit. A cap inferred from subsequent older events produces a warning. If an advertised limit prevents exhausting a shared timestamp, nstr warns that the boundary may be incomplete and continues with older timestamps. No client can prove completeness when a relay silently omits events. If EOSE never arrives, verified events already received are returned with an incompleteness warning; a timeout with no events fails that relay. Malformed messages, invalid signatures and events outside the filter remain errors.

`post` accepts `--reply ID`, `--quote ID`, `--cw TEXT`, repeatable `--image PATH`, `--video PATH` and ordered `--alt TEXT` arguments. `--dry-run` prints the draft plan without loading an account, signing, uploading or contacting relays. JPEG, PNG, GIF, WebP, MP4 and WebM uploads lose metadata through terminal-kit; uploads are limited to 100 MiB. Blossom servers are tried in configured order until one succeeds. A later publication failure can leave an uploaded blob on that server; retrying the draft does not delete that blob. Downloads verify an imeta SHA-256 when supplied.

`searchRelays` is optional; free-text search without it, or without a named relay advertising NIP-50, exits 5. Hashtag search uses `relays` and works without `searchRelays`. `dmRelays` is the list to publish with `dm-relays`; actual NIP-17 delivery uses each recipient’s published kind 10050, including a copy to the sender. A missing list exits 5 before sending. Messages are unsigned kind-14 rumors, signed kind-13 seals and kind-1059 gift wraps, encrypted with NIP-44 v2. A partially successful delivery is reported as a possible partial delivery. NIP-04 messages are outside this interface.

Delete and cancellation are NIP-09 requests, mute and block share the public NIP-51 mute list, and reports are public kind-1984 events. Relays decide what to honor. These operations do not guarantee erasure or prevent an account from reading public notes. Report reasons are `nudity`, `malware`, `profanity`, `illegal`, `spam`, `impersonation` and `other`. `--yes` skips CLI confirmation for delete, report and logout; declining sends nothing. `notifications --seen` marks the returned IDs read locally. Relationship lists and counts reflect configured relays, not the entire network.


### Local implementation suite

`scripts/compat/nostr-relays.sh` builds official release sources for strfry 1.0.1 and nostr-rs-relay 0.10.0 in disposable Docker images. It needs Docker with buildx, git, cargo, python3, ffmpeg and atago; downloading source and build dependencies requires network access. Running relays use a dedicated Docker bridge with only dynamically assigned `127.0.0.1` host ports. They have no configured upstream relays, verification services or outbound synchronization. The script waits for each NIP-11 document, generates fresh keys and a mode-0600 key file for each configuration, and runs `crates/nostr-terminal-client/e2e/compat/` against each relay alone and then both. It removes its containers, volumes, network, builder cache, images and temporary files on exit.

The suite uses three freshly generated accounts per mode with mutual kind-3 contact lists. Its public WebSocket seed includes 1,056 kind-1 notes, 101 video entries, examples with one to three pictures, 802 notes with four pictures, a root with 50 nested replies, a root with 200 direct replies, 802 mentions, two contact lists tagging the reader, a reaction and a repost. The seed prints its counts and stores signed public events so replaying the same seed is idempotent. ffmpeg creates a short H.264 pattern, a VP9 WebM and four PNGs; a Python HTTP server serves them only on 127.0.0.1. Every mode has fresh relay databases, so global hashtag searches do not include earlier modes' events.

Scenario names carry `usable:`, `heavy:`, `speed:` or `behavior:` for atago's JSON reports. Native atago assertions check identity, sample ordering, multi-page counts, byte-identical repeated reads, contact-filtered timelines, notification types, hashtag search, content-warning and alt tags, signed publication and readback, full-screen threads, pictures, reactions, reposts, replies, composition and navigation. The heavy scenarios check stored volumes and representative navigation; they do not establish full top-to-bottom traversal or rendering of every attachment. `NSTR_COMPAT_RELAYS` contains loopback WebSocket URLs, `NSTR_COMPAT_CONFIG` selects the generated account, and the launcher supplies separate public-key-only picture and video profiles. Setting `NSTR_COMPAT_REPORT_DIR` retains atago JSON reports outside the temporary run directory; otherwise the script prints verdicts and timings before deleting its files.

On 2026-11-28, all 15 implemented scenarios passed against strfry 1.0.1, nostr-rs-relay 0.10.0, and both together on Linux x86-64 with Docker 29.2.1 and a debug nstr build. Both default configurations omitted NIP-50, and text search returned exit 5 while hashtag search remained available. strfry returned NIP-11 as `application/json` and advertised `max_limit=500`; the readiness check accepts that MIME type. The local strfry configuration sets `nofiles=0` because its default request for 1,000,000 file descriptors exceeded Docker's hard limit. nostr-rs-relay 0.8.12 failed to build with Rust 1.85.1 because its locked `time` 0.3.25 dependency encounters the Rust 1.80 type-inference change; release 0.10.0 built with Rust 1.93.1.

The following values are one run's atago scenario durations in milliseconds, including assertions, rather than isolated command latency. CLI run steps must finish in less than 10 seconds. The final row includes startup, the timeline and image assertions, and quitting; each screen assertion has a 10-second timeout and the complete session has a 20-second bound. Heavy navigation assertions use 2-second bounds, with 10 seconds for loading pages or pictures. These measurements are observations on this host, not comparative benchmarks.

| Speed scenario | strfry 1.0.1 | nostr-rs-relay 0.10.0 | Both |
|---|---:|---:|---:|
| Timeline, 100 notes | 116 | 132 | 125 |
| Notifications, 100 items | 95 | 95 | 97 |
| Hashtag search, 100 notes | 86 | 82 | 76 |
| Post with picture URL | 12 | 9 | 9 |
| First timeline and picture session | 148 | 226 | 200 |

The default relay configurations do not exercise enforced AUTH, payment gates, negative acknowledgements, lowered rate limits or missing EOSE; unit and TCP fake-relay tests cover these protocol responses. Nostr WebSocket acknowledgements have no HTTP `Retry-After` header, and nstr never resends a rate-limited write. The real-relay catalog does not verify policy rejection, maximum payloads, relay restart recovery or exact write counts.

The complete shared Usable and Heavy tiers are not claimed. NIP-01 has public events rather than local or federated server timelines, user passwords or expiring access tokens, blob uploads, upload processing, or server-managed media limits and visibility settings. These items do not apply to these relay tests. These live runs did not exercise deletion requests, custom emoji, external media uploads, private messages, undo for reactions and reposts, aggregate reaction counts or CW composition; the mock suites and relay behavior section describe the current client separately. Federation, private events and third-party media services are not verified.

Atago can count JSON-line output and compare repeated output, but has no JSON-lines assertion for unique IDs, sorted keys or equality against a seeded ID set; those exact no-duplicate/no-gap checks remain unverified by the live specs. Its PTY key repetition does not wait for page loading or assert each intermediate screen, so the heavy specs cover representative pages rather than all moves. Per-note attachment placement, border/cell invariants, distinct video frames, frame rate and memory are not verified. JSON reports expose scenario duration but not each successful run or `expect_screen` duration; isolated first-paint, first-picture and move measurements are unavailable.


| Tier | strfry 1.0.1 | nostr-rs-relay 0.10.0 | Both |
| --- | --- | --- | --- |
| usable | 4 passed | 4 passed | 4 passed |
| heavy | 4 passed | 4 passed | 4 passed |
| speed | 5 passed | 5 passed | 5 passed |
| behavior | 2 passed | 2 passed | 2 passed |

With prebuilt local images, set `NSTR_COMPAT_OFFLINE=1`, `NSTR_COMPAT_STRFRY_IMAGE`, `NSTR_COMPAT_NOSTR_RS_IMAGE` and `NSTR_COMPAT_STRFRY_CONFIG` to run the compatibility script without source or image downloads. The images must contain strfry 1.0.1 and nostr-rs-relay 0.10.0 with the entrypoints defined in the script; the configuration must be the upstream strfry configuration. Containers use a dedicated Docker bridge network and expose ports only on 127.0.0.1; no external relay peers are configured. Supplied images are retained after the run.

## CI selection and report generation

The [Live compatibility workflow](../.github/workflows/compat.yml) runs six client/server combinations on the nightly schedule and through `workflow_dispatch`: mstdn with GoToSocial or Mastodon, msky with Misskey or Sharkey, and nstr with strfry or nostr-rs-relay. Each combination has a core job selecting `usable:`, `behavior:` and `speed:`, and a separate job selecting `heavy:`. Each of the twelve jobs provisions and seeds its own disposable deployment; matrix fail-fast is disabled. Akkoma, Pleroma and the combined-relay mode remain available through the local scripts.

Pull requests select only `usable:` on GoToSocial, strfry and nostr-rs-relay, filtered by the affected client paths. Changes to terminal-kit, Cargo inputs, shared compatibility scripts or the workflow select all three fast-server jobs. Misskey-only changes do not select a PR deployment because neither Misskey nor Sharkey is in the fast-server set. Speed checks run only in nightly or manual core jobs, with the existing generous bounds above. Compatibility jobs are optional and do not replace required checks.

`python3 scripts/compat/ci.py CLIENT SERVER GROUP` runs one matrix entry, where `GROUP` is `usable`, `core` or `heavy`. It retains public version metadata, revision, UTC timestamps, runner exit status and the atago JSON report under `target/compat/CLIENT-SERVER-GROUP/`. The mstdn and msky core seeds omit media volumes or deep threads used only by Heavy; the pagination data needed by Usable remains. nstr retains its full seed because Usable checks the complete catalog count. `NSTR_COMPAT_MODE=strfry` or `NSTR_COMPAT_MODE=nostr-rs-relay` builds and starts only that implementation; an unset value runs both implementations separately and together.

The nightly and manual workflow uploads each job's JSON files, then generates this document from the downloaded reports and uploads it as the `compatibility-record` artifact. CI has read-only repository permissions and does not open a pull request or push changes. After downloading the `compat-*` JSON artifacts into separate directories under `target/compat`, this command updates only the generated section below and retains the historical observations:

```sh
python3 scripts/compat/results.py target/compat
```

<!-- compat-reports:start -->

No CI reports have been imported into this checkout. The dated local results above are the recorded evidence.

<!-- compat-reports:end -->
