# Changelog

All notable changes to nstr (the nostr-terminal-client crate) are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Private NIP-01 event hashing, BIP-340 signing and verification, and NIP-19 npub, nsec, note, nprofile and nevent encoding and decoding.
- Synchronous WebSocket relay operations with concurrent configured relays, deadlines, verified events, deduplication, pagination, subscription closure and matching publication acknowledgements.
- `nstr whoami`, `timeline`, `notifications` and `post`, with positive `-n` limits, shared `--json` output, terminal-safe text and broken-pipe handling.
- Public-key reading and user-provided signing key files, checked for Unix permissions and agreement with the configured public key.
- Configuration from `<NSTR_CONFIG_DIR>/config.json` when the environment variable is non-empty, otherwise from `<platform config dir>/nstr/config.json`, following the other clients' config directory convention.
- Unit and property tests, published protocol vectors, and CLI integration tests using local fake relays and generated test keys.
- NIP-46 remote signing for `post` through a `bunker` URI, with ephemeral client keys, NIP-44 v2 encryption, signer identity and signed-note checks, and authorization URL instructions on stderr.
