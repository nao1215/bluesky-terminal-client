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

Run `nstr` without a command in an interactive terminal. Home reads the account's kind-1 notes and those of its latest kind-3 contacts; Notifications reads mentions and replies tagging the account; Profile shows the account's kind-0 metadata and notes. Lists load more near the end using inclusive `until` boundaries and deduplicate repeated events. The status row shows how many configured relays completed the latest operation and displays relay warnings.

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

Image URLs in content with PNG, JPEG, GIF, WebP, BMP or AVIF extensions, and URLs in `imeta` tags, are drawn through terminal-kit's media support when the terminal supports graphics. Picture fetching uses those HTTP or HTTPS URLs. Text remains available in terminals without graphics.

Theme and language choices are kept in `settings.json` beside `config.json`. The language is the saved choice, else the first non-empty `LC_ALL`, `LC_MESSAGES` or `LANG`, else English; English and Japanese are supported. Unknown locales fall back to English. `NO_COLOR` selects monochrome rendering. A settings file that cannot be read is left untouched. Command output remains English. Starting the full-screen client without a terminal exits with status 2 before loading configuration.

## Commands

| Command | Use |
|---|---|
| `nstr whoami` | Read the configured public key's latest kind-0 metadata |
| `nstr timeline [-n LIMIT]` | Read kind-1 notes from the account and its latest kind-3 contact list |
| `nstr notifications [-n LIMIT]` | Read kind-1 notes with a `p` tag naming the account, classified as mentions or replies |
| `nstr post TEXT` | Sign a public kind-1 note and send it once to each configured relay |

`-n` and `--limit` count unique items to print, default to 20, and require at least one. Reads combine configured relays, verify IDs and signatures, deduplicate by event ID and sort newest first with event ID as the tie breaker. An absent contact list produces a timeline of the account's own notes. Reading metadata with no matching event still prints the configured identity, with unknown metadata as null.

Lists read pages using inclusive `until` boundaries, retaining events that share a timestamp. A boundary request grows as needed, up to 10,000 events; exceeding that bound is an error. Relays can retain only part of the history or cap results, so a list reflects the events those relays return. Each WebSocket message is limited to 2 MiB.

A read completes at EOSE and sends CLOSE. A post succeeds when at least one relay returns an affirmative OK for its ID. Failed relays and NOTICE messages appear on stderr; all relays failing gives exit 4. Each relay gets the same signed event, without automatic write retries. A timeout after a write leaves acceptance unknown; the relay may have stored it.

Notifications include notes tagging the account anywhere in a conversation. NIP-10 reply or root markers, and legacy positional `e` tags, identify replies; a note counted as a reply may answer someone else in a thread the account is tagged in. Each command reads once and exits.

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

## License

[MIT](LICENSE).
