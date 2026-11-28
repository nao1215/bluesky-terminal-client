[![Build](https://github.com/nao1215/social-terminal-clients/actions/workflows/rust.yml/badge.svg)](https://github.com/nao1215/social-terminal-clients/actions/workflows/rust.yml)
[![E2E](https://github.com/nao1215/social-terminal-clients/actions/workflows/e2e.yml/badge.svg)](https://github.com/nao1215/social-terminal-clients/actions/workflows/e2e.yml)
[![measured with himorime](https://img.shields.io/badge/measured%20with-himorime-d9480f?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZmZmIiBzdHJva2Utd2lkdGg9IjIuNCIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBkPSJNNC4yIDE4LjVBOSA5IDAgMSAxIDE5LjggMTguNSIvPjxwYXRoIGZpbGw9Im5vbmUiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIyLjQiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIgZD0iTTEyIDE0LjUgMTYuNSA5Ii8%2BPGNpcmNsZSBmaWxsPSIjZmZmIiBjeD0iMTIiIGN5PSIxNC41IiByPSIyLjIiLz48L3N2Zz4=&logoColor=white)](https://github.com/nao1215/himorime)

# nstr

A full-screen Nostr client and command-line tools for public notes, profiles, search, attachments, account lists and private messages. Writes use a key file or a NIP-46 bunker. The interface supports English and Japanese.

## Build

```sh
cargo build -p nostr-terminal-client
cargo run -p nostr-terminal-client -- --help
```

## Configuration

Place a JSON file at `nstr/config.json` under the platform config directory. To use another directory, set `NSTR_CONFIG_DIR`; the client reads `<NSTR_CONFIG_DIR>/config.json` when the value is non-empty. An unset or empty value uses the platform default. Relative key paths are resolved beside that config file. Saved accounts live in `accounts/NAME.json`; The name `default` refers to `config.json`. `--account NAME` selects one for a command or the full-screen client. `accounts --use NAME` records the default in `active-account`. Key files are never changed.

```json
{
  "publicKey": "YOUR_HEX_PUBLIC_KEY_OR_NPUB_OR_NPROFILE",
  "keyFile": "signing-key",
  "relays": ["wss://YOUR_RELAY_HOST"],
  "searchRelays": ["wss://YOUR_SEARCH_RELAY_HOST"],
  "dmRelays": ["wss://YOUR_DM_RELAY_HOST"],
  "blossomServers": ["https://YOUR_BLOSSOM_HOST"],
  "timeoutSeconds": 10
}
```

`publicKey` and one to sixteen relay URLs are required; `keyFile` and `bunker` are optional for reading and cannot be configured together. Choose your own relay URLs: the client has no default relay and never follows relay hints from events or NIP-19 identifiers. `ws://` supports local relays and `wss://` uses TLS with native certificate roots. URLs with credentials or fragments are rejected. The timeout is one to 300 seconds per relay operation, including DNS resolution, connection, handshake and all its pages.

For posting, place one lowercase hex or `nsec` secret key in `keyFile` yourself. On Unix, restrict access to its owner with `chmod 600`; files accessible to group or others are rejected. Symlinks and nonregular key files are rejected. On Windows, restrict access with the file's ACL. The key must match `publicKey`. Signing and private-message encryption/decryption open it, and neither output nor errors contain it. Reading works with a public key and no signing configuration.

For remote signing, replace `keyFile` with a signer-provided NIP-46 `bunker://` URI. Its hex public key identifies the signer transport and can differ from your account's `publicKey`; its one to sixteen `relay` parameters carry signing requests, while the top-level `relays` receive the published note. The optional `secret` is sent in the encrypted `connect` request. Percent-encode query values, especially secrets containing `+` or `&`, and protect the config when it contains a secret.

```json
{
  "publicKey": "YOUR_ACCOUNT_HEX_PUBLIC_KEY_OR_NPUB_OR_NPROFILE",
  "bunker": "bunker://SIGNER_HEX_PUBLIC_KEY?relay=wss%3A%2F%2FYOUR_SIGNER_RELAY_HOST&secret=YOUR_SECRET",
  "relays": ["wss://YOUR_PUBLICATION_RELAY_HOST"],
  "timeoutSeconds": 30
}
```

`nstr post "hello"` creates an ephemeral client key in memory, connects to the bunker, checks `get_public_key` against `publicKey`, and requests a signature for the kind-1 note with `sign_event`. Requests and responses use signed kind-24133 events with NIP-44 v2 encryption and matched request IDs. The client verifies the returned signature and every requested event field before publishing. The client identity and connection authorization are reused in memory for all signing and encryption calls on that loaded account, including both DM layers. Nothing is persisted; a new process or account reload needs a new connection, so a single-use bunker secret must be replaced then. The connect request asks for the supported event kinds and NIP-44 encrypt/decrypt permissions. Public reading and hashtag search use the top-level relays, text search uses `searchRelays`, and DM delivery and reading use the recipient’s kind-10050 relay list. Private-message reading also requires NIP-44 decryption by the configured signer.

Signing requests are sent to the bunker URI's relays concurrently, using the same request ID and encrypted event on each relay. `timeoutSeconds` bounds the whole signing session, including connection and user authorization; publication then uses its own relay deadlines. An `auth_url` response shows an instruction and an HTTP or HTTPS URL in the full-screen client, or on stderr for commands, including with `--json`; open it yourself to authorize before the timeout. The client keeps waiting for the matching response and never opens the URL automatically. NIP-44 plaintext is limited to 65,535 UTF-8 bytes per request or response, including JSON overhead. Only signer-initiated `bunker://` connections are accepted.

## Full-screen client

Run `nstr` without a command in an interactive terminal. With no configuration file, a JSON editor opens; Ctrl+S validates and saves it, and Esc exits without saving. Use a public key and a key-file path or bunker URI, not private key material. Home reads the account's kind-1 notes and those of its latest kind-3 contacts; Notifications reads mentions, replies, quotes, reactions, reposts and follows tagging the account; Profile shows the account's kind-0 metadata and notes. Lists load more near the end using inclusive `until` boundaries and deduplicate repeated events. The status row shows how many configured relays completed the latest operation and displays relay warnings.

| Key | Action |
|---|---|
| `1` … `4` | Home, Notifications, Profile, Chat |
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
| `l` | Toggle a kind-7 `+` reaction using a deletion request for cancellation |
| `b` | Toggle a kind-6 repost using a deletion request for cancellation |
| `c` | Copy the note's `nostr:nevent1...` identifier using the terminal clipboard |
| `space` | Open pictures or MP4/H.264 video; `r` replays video |
| `h` `l` `←` `→` | Previous or next picture in the viewer |
| `s` | Settings: theme and language |
| `T` | Preview themes; Enter applies, Esc cancels |
| `j` `k` `enter` `x` | Move, choose, or reset the selected setting |
| `?` | Help; `j` `k` scroll and Esc closes |
| `q` `ctrl+c` | Quit; `q` closes the picture viewer or help first |
| `enter` | Open the selected author’s profile |
| `/` | Open the search box; Enter searches; Esc cancels |
| `Esc` in search results | Return to the previous timeline and selection |
| `m` / `i` | Choose a chat recipient / compose a message; Ctrl+S sends (Ctrl+Enter also works) |
| `I` | Publish the configured DM relay list, with confirmation |
| `V` | Open the quoted note; quoted content warnings remain folded |
| `Q` / `w` | Quote with a NIP-18 q tag / reveal a content warning |
| `ctrl+w` | Edit the composer’s NIP-36 content warning |
| `ctrl+o` / `tab` / `ctrl+x` | Browse attachments / edit alt text (Tab/Shift+Tab selects another attachment) / remove the selected attachment |
| `e` | Edit your profile fields (Tab selects; Ctrl+S saves; Ctrl+O attaches an avatar/banner), or enter a reaction on a note |
| `.` | Action menu |
| `D` / `f` / `M` / `B` / `!` | Delete, follow, mute/block or report; confirm with `y` |
| `A` | Saved accounts; Enter switches, `a` adds `NAME JSON`, `x` requests removal |
| `+` / `H` `L` / `<` `>` / `x` | Pin the view, focus a column, reorder columns, remove with confirmation |
| `d` | Save attachments under the configuration directory’s `downloads/` folder |

Press `/` on a list to open a bordered Search box above it. Enter searches for posts by text or hashtag and shows the results in the timeline area with the title `Search: <query>`. Results support the same navigation and post actions as the timeline, including loading more near the end. Esc returns to the previous list and selected note without fetching it again. The box remembers the last query, and Esc in the box cancels without changing the list. An empty search shows `No results for "<query>"`; a failed search keeps the previous content and shows the error in the status line. The Search tab has been removed. `+` still pins a search as a column.

A thread shows the selected note, up to 32 NIP-10 ancestors and up to eight rounds of replies returned by the configured relays. Replies preserve root and reply references and tag the author and conversation participants. The composer sends only on Ctrl+S; Enter inserts a newline. Writes use the configured key file or bunker, and a failed send keeps the draft. Reactions and reposts appear immediately and are taken back locally if no relay accepts them; the client also reads existing reactions and reposts from relays. Cancellation publishes kind 5. A timeout can still leave a relay's acceptance unknown.

Image URLs in content with PNG, JPEG, GIF, WebP, BMP or AVIF extensions, and URLs in `imeta` tags, are drawn through terminal-kit's media support when the terminal supports graphics. Picture fetching uses those HTTP or HTTPS URLs. Text remains available in terminals without graphics. A content warning hides its text and pictures until `w` opens it. NIP-30 emoji tags provide inline custom emoji. The picture cache holds up to 256 MiB under `picture-cache/`. Thread read-ahead starts after the selection rests for 400 ms. Read notification IDs and pinned columns are stored per public key in `state-PUBLIC_KEY.json`.

Theme and language choices are kept in `settings.json` beside `config.json`. The language is the saved choice, else the first non-empty `LC_ALL`, `LC_MESSAGES` or `LANG`, else English; English and Japanese are supported. Unknown locales fall back to English. `NO_COLOR` selects monochrome rendering. A settings file that cannot be read is left untouched. Command output remains English. Starting the full-screen client without a terminal exits with status 2 before loading configuration.

## Commands

| Command | Use |
|---|---|
| `nstr whoami` | Read the configured public key's latest kind-0 metadata |
| `nstr timeline [-n LIMIT]` | Read kind-1 notes from the account and its latest kind-3 contact list |
| `nstr notifications [-n LIMIT]` | Read mentions, replies, quotes, reactions, reposts and follows; `--seen` marks them read locally |
| `nstr post TEXT` | Sign a public kind-1 note; `-` reads stdin |
| `nstr profile [ACTOR]` / `author ACTOR [-n LIMIT]` | Read metadata or an author’s notes and reposts |
| `nstr note ID` / `thread ID [-n LIMIT]` | Read one event or its ancestors and replies |
| `nstr search TEXT [-n LIMIT]` | Search #hashtags on normal relays, or text on configured NIP-50 search relays |
| `nstr like ID [--reaction TEXT]` / `unlike ID` | Publish a reaction or request its deletion |
| `nstr repost ID` / `unrepost ID` / `delete ID [--yes]` | Repost or publish NIP-09 deletion requests |
| `nstr likes ID` / `reposts ID` | Read observed reaction/repost events |
| `nstr follow ACTOR` / `unfollow ACTOR` | Update kind 3, preserving unrelated tags and content |
| `nstr follows [ACTOR]` / `followers [ACTOR]` | Read observed relationships |
| `nstr mute ACTOR` / `unmute ACTOR` / `mutes` | Update/read the public portion of kind 10000; block/unblock/blocks are aliases |
| `nstr report ID --reason REASON --comment TEXT [--yes]` | Publish a public NIP-56 report |
| `nstr profile-update JSON` | Merge a JSON object into kind 0, retaining unknown fields; `-` reads stdin |
| `nstr upload PATH [--alt TEXT]` | Strip metadata, upload through Blossom and print the imeta tag |
| `nstr download ID [--output DIR]` | Save attachments without overwriting existing files |
| `nstr chat [ACTOR] [TEXT] [-n LIMIT]` | List conversations, read private messages or send to an account; `TEXT=-` reads stdin |
| `nstr dm-relays` | Publish `dmRelays` as your kind-10050 inbox list |
| `nstr login NAME --public-key KEY --relay URL [--key-file PATH or --bunker URI]` | Save a configuration without copying private key material |
| `nstr accounts [--use NAME]` / `logout NAME [--yes]` | List/select/remove saved configurations |

`-n` and `--limit` count unique items to print, default to 20, and require at least one. Reads combine configured relays, verify IDs and signatures, deduplicate by event ID and sort newest first with event ID as the tie breaker. An absent contact list produces a timeline of the account's own notes. Reading metadata with no matching event still prints the configured identity, with unknown metadata as null.

Lists read pages using inclusive `until` boundaries, retaining events that share a timestamp. A boundary request grows as needed, up to 10,000 events; exceeding that bound is an error. Relays can retain only part of the history or cap results, so a list reflects the events those relays return. Incoming WebSocket messages are limited to 2 MiB.

A read completes at EOSE and sends CLOSE. A post succeeds when at least one relay returns an affirmative OK or a `duplicate:` acknowledgement for its ID. Failed relays and NOTICE messages appear on stderr; all relays reporting an unsupported feature gives exit 5; any transport or other relay failure among the failed relays gives exit 4. Each relay gets the same signed event. The only automatic write retry is one resend after a NIP-42 authentication succeeds and the original event was rejected with `auth-required:`. A timeout after a write leaves acceptance unknown; the relay may have stored it.

Notifications include notes tagging the account anywhere in a conversation, kind-7 reactions, kind-6 reposts and kind-3 contact lists. Contact lists are reported as `follow` observations; nstr does not compare contact-list history to distinguish a new follow from a retained contact. NIP-10 reply or root markers, and legacy positional `e` tags, identify replies; a note counted as a reply may answer someone else in a thread the account is tagged in. Each command reads once and exits.

## Relay compatibility

NIP-01 supplies event signatures, filters, kind-0 profiles, kind-1 notes, acknowledgements and subscription messages; NIP-02 supplies kind-3 contacts. NIP-10 identifies replies, NIP-18 and NIP-25 supply reposts and reactions, and NIP-19 supplies encoded identities and event references. Missing profiles produce null metadata, missing contact lists limit Home to the account's own notes, and rejected reposts or reactions produce relay warnings. NIP-36 content warnings and NIP-92 image metadata are read from tags. Hashtag queries use the NIP-01 `#t` filter and match the supplied tag exactly. Text search uses only `searchRelays` and requires advertised NIP-50 support; the relay controls text matching and ranking, and nstr orders returned events newest first. An unadvertised search is reported as unsupported before sending a subscription. NIP-44 v2 encrypts NIP-17 messages and NIP-46 remote signer requests; an incompatible signer fails the write without publishing an unverified event.

NIP-11 information is fetched once per configured relay URL per process, including across full-screen refreshes, with `Accept: application/nostr+json` over HTTP or HTTPS. Redirects and proxies are disabled. Missing, malformed or unavailable information falls back to ordinary NIP-01 behavior without a warning. Fetching uses at most a quarter of the relay operation timeout, capped at two seconds, and shares that operation's deadline. The `limitation` fields constrain operations, and `supported_nips` gates NIP-50 text search; other implemented NIPs need not be advertised.

Reads use one subscription at a time and respect `max_limit`; zero `max_subscriptions` or `max_limit` makes reads unavailable. `max_message_length` bounds outgoing UTF-8 message bytes. `payment_required` makes that relay unavailable because nstr has no payment flow. `auth_required` makes public-key-only reads unavailable. Unavailable read subscriptions, payment flows and authenticated reads use exit 5 when no relay can serve the request. Explicit `unsupported:` NOTICE, CLOSED and negative OK messages also use exit 5. Oversized messages and ordinary relay rejections remain exit 4. Reasons are retained per relay, and any relay supplying usable results allows the operation to succeed with warnings for the others.

NIP-42 challenges received while posting are answered with a kind-22242 event signed by the configured account signer (key file or NIP-46 bunker), with the relay URL, challenge and current timestamp. The client waits for a matching authentication OK and retries an explicitly auth-rejected publication at most once. Repeated challenges and rejected authentication fail that relay. Reads never open a key file and report that the relay requires auth. All relay publications use this handshake, including gift wraps authenticated with the sending account rather than the ephemeral wrapper key. A bunker must permit signing kind 22242 as well as the application event kind.

CLOSED reasons and NOTICE text are retained per relay. Negative OK reasons keep their machine-readable `blocked:`, `rate-limited:`, `invalid:`, `pow:`, `restricted:` or `auth-required:` prefix; nstr does not pay, mine proof of work or automatically retry rate limits. A `duplicate:` response means the relay already has the event and counts as acceptance. Unknown rejection prefixes remain visible as well.

Short pages are followed until exhaustion, the requested count or the deadline, including when the relay caps results without advertising a limit. A cap inferred from subsequent older events produces a warning. If an advertised limit prevents exhausting a shared timestamp, nstr warns that the boundary may be incomplete and continues with older timestamps. No client can prove completeness when a relay silently omits events. If EOSE never arrives, verified events already received are returned with an incompleteness warning; a timeout with no events fails that relay. Malformed messages, invalid signatures and events outside the filter remain errors.

`post` accepts `--reply ID`, `--quote ID`, `--cw TEXT`, repeatable `--image PATH`, `--video PATH` and ordered `--alt TEXT` arguments. `--dry-run` prints the draft plan without loading an account, signing, uploading or contacting relays. JPEG, PNG, GIF, WebP, MP4 and WebM uploads lose metadata through terminal-kit; uploads are limited to 100 MiB. Blossom servers are tried in configured order until one succeeds. A later publication failure can leave an uploaded blob on that server; retrying the draft does not delete that blob. Downloads verify an imeta SHA-256 when supplied.

`searchRelays` is optional; free-text search without it, or without a named relay advertising NIP-50, exits 5. Hashtag search uses `relays` and works without `searchRelays`. `dmRelays` is the list to publish with `dm-relays`; actual NIP-17 delivery uses each recipient’s published kind 10050, including a copy to the sender. A missing list exits 5 before sending. Messages are unsigned kind-14 rumors, signed kind-13 seals and kind-1059 gift wraps, encrypted with NIP-44 v2. A partially successful delivery is reported as a possible partial delivery. NIP-04 messages are outside this interface.

Delete and cancellation are NIP-09 requests, mute and block share the public NIP-51 mute list, and reports are public kind-1984 events. Relays decide what to honor. These operations do not guarantee erasure or prevent an account from reading public notes. Report reasons are `nudity`, `malware`, `profanity`, `illegal`, `spam`, `impersonation` and `other`. `--yes` skips CLI confirmation for delete, report and logout; declining sends nothing. `notifications --seen` marks the returned IDs read locally. Relationship lists and counts reflect configured relays, not the entire network.

## Environment

| Variable | Use |
|---|---|
| `NSTR_CONFIG_DIR` | Non-empty override of the configuration directory |
| `NSTR_GRAPHICS` | Force `kitty`, `sixel` or `iterm2` graphics |
| `LC_ALL`, `LC_MESSAGES`, `LANG` | UI language when no saved choice exists |
| `NO_COLOR` | Monochrome rendering |
| `TZ` | Local timestamp display |

## Output

`--json` prints one object per line using [the shared JSON contract](../../doc/json.md). Accounts include `npub`, `nprofile` and `picture`. Posts include `kind`, `tags`, `sig`, `note` and `nevent`; `uri` is a `nostr:note...` identifier, `url` and unqueried counts are null, and `files` contains normalized NIP-92 fields and recognized media URLs. Embedded users identify authors by npub; lists batch-fetch kind-0 display names, leaving unknown names null. Full metadata is available through `whoami` and `profile`. A write includes `uri` and `acceptedRelays` beside the shared action keys.

Text output removes terminal control characters and formats timestamps with terminal-kit's helpers. A pipe closed by its reader, such as `nstr timeline | head`, exits successfully. Errors print `error: ...` and an optional `hint: ...` on stderr, and with `--json` also print `{"error", "hint", "status"}` on stdout.

| Exit status | Meaning |
|---|---|
| 0 | Success |
| 1 | Invalid command, option or argument, or no signing key configured for posting |
| 2 | Full-screen client needs a terminal, or terminal I/O failed |
| 3 | Config, key file or output failure |
| 4 | Network, relay protocol, remote signer or event verification failure |
| 5 | Requested event or configured capability is unavailable |

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

Unit and property tests check event hashing and signatures against the published BIP-340 and NIP-19 vectors, and cover message parsing, config validation and output. NIP-44 tests embed selected [paulmillr/nip44 vectors](tests/testdata/nip44-v2.json) and check key derivation, padding, encryption, decryption and tampering. The integration tests run the binary against fake relays and a fake remote signer started in the test process on loopback, with keys the tests generate; signer cases cover success, key mismatch, rejection, authorization URLs, timeout, request matching, altered events and config conflicts. The end-to-end specs in `e2e/atago/` run the built binary against `e2e/atago/testdata/relay.py`, a relay stand-in on 127.0.0.1 whose events are signed with throwaway keys; no test connects to a public relay. Remote signing is covered by the Rust integration suite; the Python fixtures use only the standard library and include an independent ChaCha20/NIP-44 decoder to inspect gift wraps. Blossom tests inspect authorization events and the exact uploaded bytes. NIP-59’s official example is embedded with its source in `tests/testdata/nip59.json`.

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

With prebuilt local images, set `NSTR_COMPAT_OFFLINE=1`, `NSTR_COMPAT_STRFRY_IMAGE`, `NSTR_COMPAT_NOSTR_RS_IMAGE` and `NSTR_COMPAT_STRFRY_CONFIG` to run the compatibility script without source or image downloads. The images must contain strfry 1.0.1 and nostr-rs-relay 0.10.0 with the entrypoints defined in the script; the configuration must be the upstream strfry configuration. Containers use a dedicated Docker bridge network and expose ports only on 127.0.0.1; no external relay peers are configured. Supplied images are retained after the run.
