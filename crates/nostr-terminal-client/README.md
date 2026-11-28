[![Build](https://github.com/nao1215/social-terminal-clients/actions/workflows/rust.yml/badge.svg)](https://github.com/nao1215/social-terminal-clients/actions/workflows/rust.yml)
[![E2E](https://github.com/nao1215/social-terminal-clients/actions/workflows/e2e.yml/badge.svg)](https://github.com/nao1215/social-terminal-clients/actions/workflows/e2e.yml)
[![measured with himorime](https://img.shields.io/badge/measured%20with-himorime-d9480f?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZmZmIiBzdHJva2Utd2lkdGg9IjIuNCIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBkPSJNNC4yIDE4LjVBOSA5IDAgMSAxIDE5LjggMTguNSIvPjxwYXRoIGZpbGw9Im5vbmUiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIyLjQiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIgZD0iTTEyIDE0LjUgMTYuNSA5Ii8%2BPGNpcmNsZSBmaWxsPSIjZmZmIiBjeD0iMTIiIGN5PSIxNC41IiByPSIyLjIiLz48L3N2Zz4=&logoColor=white)](https://github.com/nao1215/himorime)

# nstr

A full-screen Nostr client and commands for reading profile metadata, a home timeline and notifications, and publishing signed public text notes.

## Build

```sh
cargo build -p nostr-terminal-client
cargo run -p nostr-terminal-client -- --help
```

## Configuration

Place a JSON file at `nstr/config.json` under the platform config directory. To use another directory, set `NSTR_CONFIG_DIR`; the client reads `<NSTR_CONFIG_DIR>/config.json` when the value is non-empty. An unset or empty value uses the platform default. Relative key paths are resolved beside that config file. The client reads configuration and keys without creating or changing them.

```json
{
  "publicKey": "YOUR_HEX_PUBLIC_KEY_OR_NPUB_OR_NPROFILE",
  "keyFile": "signing-key",
  "relays": ["wss://YOUR_RELAY_HOST"],
  "timeoutSeconds": 10
}
```

`publicKey` and one to sixteen relay URLs are required; `keyFile` and `bunker` are optional for reading and cannot be configured together. Choose your own relay URLs: the client has no default relay and never follows relay hints from events or NIP-19 identifiers. `ws://` supports local relays and `wss://` uses TLS with native certificate roots. URLs with credentials or fragments are rejected. The timeout is one to 300 seconds per relay operation, including DNS resolution, connection, handshake and all its pages.

For posting, place one lowercase hex or `nsec` secret key in `keyFile` yourself. On Unix, restrict access to its owner with `chmod 600`; files accessible to group or others are rejected. Symlinks and nonregular key files are rejected. On Windows, restrict access with the file's ACL. The key must match `publicKey`. Only a write (`post`, a composer sent with Ctrl+S, a reaction or a repost) opens it, and neither output nor errors contain it. Reading works with a public key and no signing configuration.

For remote signing, replace `keyFile` with a signer-provided NIP-46 `bunker://` URI. Its hex public key identifies the signer transport and can differ from your account's `publicKey`; its one to sixteen `relay` parameters carry signing requests, while the top-level `relays` receive the published note. The optional `secret` is sent in the encrypted `connect` request. Percent-encode query values, especially secrets containing `+` or `&`, and protect the config when it contains a secret.

```json
{
  "publicKey": "YOUR_ACCOUNT_HEX_PUBLIC_KEY_OR_NPUB_OR_NPROFILE",
  "bunker": "bunker://SIGNER_HEX_PUBLIC_KEY?relay=wss%3A%2F%2FYOUR_SIGNER_RELAY_HOST&secret=YOUR_SECRET",
  "relays": ["wss://YOUR_PUBLICATION_RELAY_HOST"],
  "timeoutSeconds": 30
}
```

`nstr post "hello"` creates an ephemeral client key in memory, connects to the bunker, checks `get_public_key` against `publicKey`, and requests a signature for the kind-1 note with `sign_event`. Requests and responses use signed kind-24133 events with NIP-44 v2 encryption and matched request IDs. The client verifies the returned signature and every requested event field before publishing. It writes no client key or session state to disk; each run needs a new connection, so a single-use bunker secret must be replaced after use. Reading commands contact only the top-level relays.

Signing requests are sent to the bunker URI's relays concurrently, using the same request ID and encrypted event on each relay. `timeoutSeconds` bounds the whole signing session, including connection and user authorization; publication then uses its own relay deadlines. An `auth_url` response shows an instruction and an HTTP or HTTPS URL in the full-screen client, or on stderr for commands, including with `--json`; open it yourself to authorize before the timeout. The client keeps waiting for the matching response and never opens the URL automatically. NIP-44 plaintext is limited to 65,535 UTF-8 bytes per request or response, including JSON overhead. Only signer-initiated `bunker://` connections are accepted.

## Full-screen client

Run `nstr` without a command in an interactive terminal. Home reads the account's kind-1 notes and those of its latest kind-3 contacts; Notifications reads notes, reactions, reposts and contact lists tagging the account; Profile shows the account's kind-0 metadata and notes. Lists load more near the end using inclusive `until` boundaries and deduplicate repeated events. The status row shows how many configured relays completed the latest operation and displays relay warnings.

| Key | Action |
|---|---|
| `1` `2` `3` | Home, Notifications, Profile |
| `tab` `shift+tab` | Next or previous tab |
| `j` `k` `↓` `↑` | Move the selection |
| `g` `G` `home` `end` | First or last loaded note |
| `pgdn` `pgup` | Move five notes |
| `R` `F5` | Refresh the current view |
| `v` `esc` | Open a note's thread or go back |
| `n` `r` | New note or reply to the selected note |
| `ctrl+s` | Send from the composer |
| `enter` `ctrl+u` | Insert a newline or clear to the start of the line in the composer |
| `esc` | Cancel the composer without sending |
| `l` | Send a kind-7 `+` reaction |
| `b` | Send a kind-6 repost |
| `c` | Copy the note's `nostr:nevent1...` identifier using the terminal clipboard |
| `space` | Open the note's pictures |
| `h` `l` `←` `→` | Previous or next picture in the viewer |
| `s` | Settings: theme and language |
| `T` | Preview themes; Enter applies, Esc cancels |
| `j` `k` `enter` `x` | Move, choose, or reset the selected setting |
| `?` | Help; `j` `k` scroll and Esc closes |
| `q` `ctrl+c` | Quit; `q` closes the picture viewer or help first |

A thread shows the selected note, its NIP-10 parent when available, and replies returned by the configured relays. Replies preserve root and reply references and tag the author and conversation participants. The composer sends only on Ctrl+S; Enter inserts a newline. Writes use the configured key file or bunker, and a failed send keeps the draft. Reactions and reposts appear immediately and are taken back locally if no relay accepts them; repeated presses do not duplicate an action already sent during the session. A timeout can still leave a relay's acceptance unknown.

Video URLs and `imeta` entries with a video MIME type are identified as videos and excluded from the picture decoder. The full-screen client reports `unsupported: video playback in nstr` when Space is pressed on a video-only note; it continues running. It has no video playback, thumbnails, inline playback or video saving.

Image URLs in content with PNG, JPEG, GIF, WebP, BMP or AVIF extensions, and URLs in `imeta` tags, are drawn through terminal-kit's media support when the terminal supports graphics. Picture fetching uses those HTTP or HTTPS URLs. Text remains available in terminals without graphics.

Theme and language choices are kept in `settings.json` beside `config.json`. The language is the saved choice, else the first non-empty `LC_ALL`, `LC_MESSAGES` or `LANG`, else English; English and Japanese are supported. Unknown locales fall back to English. `NO_COLOR` selects monochrome rendering. A settings file that cannot be read is left untouched. Command output remains English. Starting the full-screen client without a terminal exits with status 2 before loading configuration.

## Commands

| Command | Use |
|---|---|
| `nstr whoami` | Read the configured public key's latest kind-0 metadata |
| `nstr timeline [-n LIMIT]` | Read kind-1 notes from the account and its latest kind-3 contact list |
| `nstr notifications [-n LIMIT]` | Read notes, reactions, reposts and contact lists with a `p` tag naming the account |
| `nstr search QUERY [-n LIMIT]` | Search an exact hashtag with NIP-01, or text with advertised NIP-50 support |
| `nstr post TEXT` | Sign a public kind-1 note and send it once to each configured relay |

`-n` and `--limit` count unique items to print, default to 20, and require at least one. Reads combine configured relays, verify IDs and signatures, deduplicate by event ID and sort newest first with event ID as the tie breaker. An absent contact list produces a timeline of the account's own notes. Reading metadata with no matching event still prints the configured identity, with unknown metadata as null.

Lists read pages using inclusive `until` boundaries, retaining events that share a timestamp. A boundary request grows as needed, up to 10,000 events; exceeding that bound is an error. Relays can retain only part of the history or cap results, so a list reflects the events those relays return. Incoming WebSocket messages are limited to 2 MiB.

A read completes at EOSE and sends CLOSE. A post succeeds when at least one relay returns an affirmative OK or a `duplicate:` acknowledgement for its ID. Failed relays and NOTICE messages appear on stderr; all relays reporting an unsupported feature gives exit 5; any transport or other relay failure among the failed relays gives exit 4. Each relay gets the same signed event. The only automatic write retry is one resend after a NIP-42 authentication succeeds and the original event was rejected with `auth-required:`. A timeout after a write leaves acceptance unknown; the relay may have stored it.

Notifications include notes tagging the account anywhere in a conversation, kind-7 reactions, kind-6 reposts and kind-3 contact lists. Contact lists are reported as `follow` observations; nstr does not compare contact-list history to distinguish a new follow from a retained contact. NIP-10 reply or root markers, and legacy positional `e` tags, identify replies; a note counted as a reply may answer someone else in a thread the account is tagged in. Each command reads once and exits.

## Relay compatibility

NIP-01 supplies event signatures, filters, kind-0 profiles, kind-1 notes, acknowledgements and subscription messages; NIP-02 supplies kind-3 contacts. NIP-10 identifies replies, NIP-18 and NIP-25 supply reposts and reactions, and NIP-19 supplies encoded identities and event references. Missing profiles produce null metadata, missing contact lists limit Home to the account's own notes, and rejected reposts or reactions produce relay warnings. NIP-36 content warnings and NIP-92 image metadata are read from tags. Hashtag queries use the NIP-01 `#t` filter and match the supplied tag exactly. Text search requires advertised NIP-50 support; the relay controls text matching and ranking, and nstr orders returned events newest first. An unadvertised search is reported as unsupported before sending a subscription. NIP-44 v2 and NIP-46 are used only for a configured remote signer; an incompatible signer fails the write without publishing an unverified event.

NIP-11 information is fetched once per configured publication relay URL per process, including across full-screen refreshes, with `Accept: application/nostr+json` over HTTP or HTTPS. Redirects and proxies are disabled. Missing, malformed or unavailable information falls back to ordinary NIP-01 behavior without a warning. Fetching uses at most a quarter of the relay operation timeout, capped at two seconds, and shares that operation's deadline. The `limitation` fields constrain operations, and `supported_nips` gates NIP-50 text search; other implemented NIPs need not be advertised.

Reads use one subscription at a time and respect `max_limit`; zero `max_subscriptions` or `max_limit` makes reads unavailable. `max_message_length` bounds outgoing UTF-8 message bytes. `payment_required` makes that relay unavailable because nstr has no payment flow. `auth_required` makes public-key-only reads unavailable. Unavailable read subscriptions, payment flows and authenticated reads use exit 5 when no relay can serve the request. Explicit `unsupported:` NOTICE, CLOSED and negative OK messages also use exit 5. Oversized messages and ordinary relay rejections remain exit 4. Reasons are retained per relay, and any relay supplying usable results allows the operation to succeed with warnings for the others.

NIP-42 challenges received while posting are answered with a kind-22242 event signed by the local posting key, with the relay URL, challenge and current timestamp. The client waits for a matching authentication OK and retries an explicitly auth-rejected publication at most once. Repeated challenges and rejected authentication fail that relay. Reads never open a key file and report that the relay requires auth. Publication with a bunker has no local account key for relay AUTH and reports the same requirement; NIP-42 authentication through a bunker is not implemented.

CLOSED reasons and NOTICE text are retained per relay. Negative OK reasons keep their machine-readable `blocked:`, `rate-limited:`, `invalid:`, `pow:`, `restricted:` or `auth-required:` prefix; nstr does not pay, mine proof of work or automatically retry rate limits. A `duplicate:` response means the relay already has the event and counts as acceptance. Unknown rejection prefixes remain visible as well.

Short pages are followed until exhaustion, the requested count or the deadline, including when the relay caps results without advertising a limit. A cap inferred from subsequent older events produces a warning. If an advertised limit prevents exhausting a shared timestamp, nstr warns that the boundary may be incomplete and continues with older timestamps. No client can prove completeness when a relay silently omits events. If EOSE never arrives, verified events already received are returned with an incompleteness warning; a timeout with no events fails that relay. Malformed messages, invalid signatures and events outside the filter remain errors.

## Output

`--json` prints one object per line using [the shared JSON contract](../../doc/json.md). Accounts include `npub`, `nprofile` and `picture`. Posts include `kind`, `tags`, `sig`, `note` and `nevent`; `uri` is a `nostr:note...` identifier, `url` and unqueried counts are null, and `files` is empty. Embedded users identify authors by npub; profile metadata is read by `whoami`. A write includes `uri` and `acceptedRelays` beside the shared action keys.

Text output removes terminal control characters and formats timestamps with terminal-kit's helpers. A pipe closed by its reader, such as `nstr timeline | head`, exits successfully. Errors print `error: ...` and an optional `hint: ...` on stderr, and with `--json` also print `{"error", "hint", "status"}` on stdout.

| Exit status | Meaning |
|---|---|
| 0 | Success |
| 1 | Invalid command, option or argument, or no signing key configured for posting |
| 2 | Full-screen client needs a terminal, or terminal I/O failed |
| 3 | Config, key file or output failure |
| 4 | Network, relay protocol, remote signer or event verification failure |
| 5 | Unsupported feature |

## Testing

| Technique | Coverage |
|---|---|
| Unit and property tests | Events, signatures, identifiers, encryption and signer responses; nightly properties use 20,000 cases |
| Fuzzing | Relay messages, events and tags, NIP-19, NIP-44, NIP-46 bunker URIs and responses |
| Mutation tests | Protocol parsing, verification, attachment discovery and draft tags |
| Benchmarks | Start-up, 100-event commands and the first post on screen |

The fuzz harnesses include the client's source modules. The NIP-46 harness builds the URI and response parsers from `src/api/nip46.rs`, exposing the private response function only in the fuzz build; it encrypts and signs generated plaintext so malformed responses reach the parser after authentication. NIP-19 mutations also receive fresh Bech32 checksums to exercise the TLV reader. No fuzz target opens a connection or loads user configuration.

From this crate's directory:

```sh
just bench                                  # himorime, Python 3
just fuzz relay_message -max_total_time=60
just properties                             # 20,000 cases per property
just mutants                                # changes since main
```

[Benchmarks](bench/README.md) run against local stand-ins. Fuzz targets run over their committed seeds on pull requests and explore nightly for 300 seconds each; local exploration writes new inputs to `fuzz/work/`. Mutation testing reports changed lines on pull requests and the selected source files weekly, without a score gate.

```sh
cargo test -p nostr-terminal-client
just e2e-nstr    # end-to-end tests (atago, needs python3)
```

Unit and property tests check event hashing and signatures against the published BIP-340 and NIP-19 vectors, and cover message parsing, config validation and output. NIP-44 tests embed selected [paulmillr/nip44 vectors](tests/testdata/nip44-v2.json) and check key derivation, padding, encryption, decryption and tampering. The integration tests run the binary against fake relays and a fake remote signer started in the test process on loopback, with keys the tests generate; signer cases cover success, key mismatch, rejection, authorization URLs, timeout, request matching, altered events and config conflicts. The end-to-end specs in `e2e/atago/` run the built binary against `e2e/atago/testdata/relay.py`, a relay stand-in on 127.0.0.1 whose events are signed with throwaway keys; no test connects to a public relay. Remote signing is covered by the Rust integration suite; the Python relay uses only the standard library, which provides HMAC-SHA256 but no ChaCha20 implementation.

### Local implementation suite

`scripts/compat/nostr-relays.sh` builds official release sources for strfry 1.0.1 and nostr-rs-relay 0.10.0 in disposable Docker images. It needs Docker with buildx, git, cargo, python3, ffmpeg and atago; downloading source and build dependencies requires network access. Running relays use a dedicated Docker bridge with only dynamically assigned `127.0.0.1` host ports. They have no configured upstream relays, verification services or outbound synchronization. The script waits for each NIP-11 document, generates fresh keys and a mode-0600 key file for each configuration, and runs `e2e/compat/` against each relay alone and then both. It removes its containers, volumes, network, builder cache, images and temporary files on exit.

The suite uses three freshly generated accounts per mode with mutual kind-3 contact lists. Its public WebSocket seed includes 1,056 kind-1 notes, 101 video entries, examples with one to three pictures, 802 notes with four pictures, a root with 50 nested replies, a root with 200 direct replies, 802 mentions, two contact lists tagging the reader, a reaction and a repost. The seed prints its counts and stores signed public events so replaying the same seed is idempotent. ffmpeg creates a short H.264 pattern, a VP9 WebM and four PNGs; a Python HTTP server serves them only on 127.0.0.1. Every mode has fresh relay databases, so global hashtag searches do not include earlier modes' events.

Scenario names carry `usable:`, `heavy:`, `speed:` or `behavior:` for atago's JSON reports. Native atago assertions check identity, sample ordering, multi-page counts, byte-identical repeated reads, contact-filtered timelines, notification types, hashtag search, content-warning and alt tags, signed publication and readback, full-screen threads, pictures, reactions, reposts, replies, composition and navigation. The heavy scenarios check stored volumes and representative navigation; they do not establish full top-to-bottom traversal or rendering of every attachment. `NSTR_COMPAT_RELAYS` contains loopback WebSocket URLs, `NSTR_COMPAT_CONFIG` selects the generated account, and the launcher supplies separate public-key-only picture and video profiles. Setting `NSTR_COMPAT_REPORT_DIR` retains atago JSON reports outside the temporary run directory; otherwise the script prints verdicts and timings before deleting its files.

On 2026-11-28, all 15 implemented scenarios passed against strfry 1.0.1, nostr-rs-relay 0.10.0, and both together on Linux x86-64 with Docker 29.2.1 and a debug nstr build. Both default configurations omitted NIP-50, and text search returned exit 5 while hashtag search remained available. strfry returned NIP-11 as `application/json` and advertised `max_limit=500`; the readiness check accepts that MIME type. The local strfry configuration sets `nofiles=0` because its default request for 1,000,000 file descriptors exceeded Docker's hard limit. nostr-rs-relay 0.8.12 failed to build with Rust 1.85.1 because its locked `time` 0.3.25 dependency encounters the Rust 1.80 type-inference change; the newest upstream release tag, 0.10.0, built with Rust 1.93.1.

The following values are one run's atago scenario durations in milliseconds, including assertions, rather than isolated command latency. CLI run steps must finish in less than 10 seconds. The final row includes startup, the timeline and image assertions, and quitting; each screen assertion has a 10-second timeout and the complete session has a 20-second bound. Heavy navigation assertions use 2-second bounds, with 10 seconds for loading pages or pictures. These measurements are observations on this host, not comparative benchmarks.

| Speed scenario | strfry 1.0.1 | nostr-rs-relay 0.10.0 | Both |
|---|---:|---:|---:|
| Timeline, 100 notes | 116 | 132 | 125 |
| Notifications, 100 items | 95 | 95 | 97 |
| Hashtag search, 100 notes | 86 | 82 | 76 |
| Post with picture URL | 12 | 9 | 9 |
| First timeline and picture session | 148 | 226 | 200 |

The default relay configurations do not exercise enforced AUTH, payment gates, negative acknowledgements, lowered rate limits or missing EOSE; unit and TCP fake-relay tests cover these protocol responses. Nostr WebSocket acknowledgements have no HTTP `Retry-After` header, and nstr never resends a rate-limited write. The real-relay catalog does not verify policy rejection, maximum payloads, relay restart recovery or exact write counts.

The complete shared Usable and Heavy tiers are not claimed. NIP-01 has public events rather than local or federated server timelines, user passwords or expiring access tokens, blob uploads, upload processing, or server-managed media limits and visibility settings. These items do not apply to these relay tests. Nostr does have deletion requests (NIP-09), custom emoji (NIP-30), and optional media and private-message protocols, but nstr does not implement those operations. It also lacks undo for reactions and reposts, aggregate reaction counts, CW composition, video playback and saving; those are client gaps rather than absent Nostr features. Federation, private events and third-party media services are not verified.

Atago can count JSON-line output and compare repeated output, but has no JSON-lines assertion for unique IDs, sorted keys or equality against a seeded ID set; those exact no-duplicate/no-gap checks remain unverified by the live specs. Its PTY key repetition does not wait for page loading or assert each intermediate screen, so the heavy specs cover representative pages rather than all moves. Per-note attachment placement, border/cell invariants, distinct video frames, frame rate and memory are not verified. JSON reports expose scenario duration but not each successful run or `expect_screen` duration; isolated first-paint, first-picture and move measurements are unavailable.

## License

[MIT](LICENSE).
