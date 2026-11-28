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

See the [compatibility record](../../doc/compat.md#nstr-results) for relay behavior, tested versions, tier outcomes, timings and coverage limits.

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

See the [E2E scenarios](../../doc/e2e/), [compatibility results](../../doc/compat.md) and [nightly compatibility workflow](../../.github/workflows/compat.yml) for test coverage and recorded runs.

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

See the [compatibility record](../../doc/compat.md#nstr-results) for the local relay scripts, tested versions, tier outcomes and timings.

## License

[MIT](LICENSE).
