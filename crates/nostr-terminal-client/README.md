# nstr

Nostr commands for reading profile metadata, a home timeline and notifications, and publishing signed public text notes.

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

`publicKey` and one to sixteen relay URLs are required; `keyFile` is optional for reading. Choose your own relay URLs: the client has no default relay and never follows relay hints from events or NIP-19 identifiers. `ws://` supports local relays and `wss://` uses TLS with native certificate roots. URLs with credentials or fragments are rejected. The timeout is one to 300 seconds per relay operation, including DNS resolution, connection, handshake and all its pages.

For posting, place one lowercase hex or `nsec` secret key in `keyFile` yourself. On Unix, restrict access to its owner with `chmod 600`; files accessible to group or others are rejected. Symlinks and nonregular key files are rejected. On Windows, restrict access with the file's ACL. The key must match `publicKey`. Only `post` opens it, and neither output nor errors contain it. Reading works with a public key and no secret key file.

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
| 3 | Config, key file or output failure |
| 4 | Network, relay protocol or event verification failure |

## Tests

```sh
cargo test -p nostr-terminal-client
```

Unit and property tests check event hashing and signatures against the published BIP-340 and NIP-19 vectors, and cover message parsing, config validation and output. The integration tests run the binary against fake relays started in the test process on loopback, with keys the tests generate; no test connects to a public relay.

## License

[MIT](LICENSE).
