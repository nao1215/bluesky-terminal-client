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

`publicKey` and one to sixteen relay URLs are required; `keyFile` and `bunker` are optional for reading and cannot be configured together. Choose your own relay URLs: the client has no default relay and never follows relay hints from events or NIP-19 identifiers. `ws://` supports local relays and `wss://` uses TLS with native certificate roots. URLs with credentials or fragments are rejected. The timeout is one to 300 seconds per relay operation, including DNS resolution, connection, handshake and all its pages.

For posting, place one lowercase hex or `nsec` secret key in `keyFile` yourself. On Unix, restrict access to its owner with `chmod 600`; files accessible to group or others are rejected. Symlinks and nonregular key files are rejected. On Windows, restrict access with the file's ACL. The key must match `publicKey`. Only `post` opens it, and neither output nor errors contain it. Reading works with a public key and no signing configuration.

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

Signing requests are sent to the bunker URI's relays concurrently, using the same request ID and encrypted event on each relay. `timeoutSeconds` bounds the whole signing session, including connection and user authorization; publication then uses its own relay deadlines. An `auth_url` response prints an instruction and an HTTP or HTTPS URL on stderr, including with `--json`; open it yourself to authorize before the timeout. The client keeps waiting for the matching response and never opens the URL automatically. NIP-44 plaintext is limited to 65,535 UTF-8 bytes per request or response, including JSON overhead. Only signer-initiated `bunker://` connections are accepted.

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
| 4 | Network, relay protocol, remote signer or event verification failure |

## Tests

```sh
cargo test -p nostr-terminal-client
just e2e-nstr    # end-to-end tests (atago, needs python3)
```

Unit and property tests check event hashing and signatures against the published BIP-340 and NIP-19 vectors, and cover message parsing, config validation and output. NIP-44 tests embed selected [paulmillr/nip44 vectors](tests/testdata/nip44-v2.json) and check key derivation, padding, encryption, decryption and tampering. The integration tests run the binary against fake relays and a fake remote signer started in the test process on loopback, with keys the tests generate; signer cases cover success, key mismatch, rejection, authorization URLs, timeout, request matching, altered events and config conflicts. The end-to-end specs in `e2e/atago/` run the built binary against `e2e/atago/testdata/relay.py`, a relay stand-in on 127.0.0.1 whose events are signed with throwaway keys; no test connects to a public relay. Remote signing is covered by the Rust integration suite; the Python relay uses only the standard library, which provides HMAC-SHA256 but no ChaCha20 implementation.

## License

[MIT](LICENSE).
