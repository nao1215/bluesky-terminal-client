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
- Full-screen client when `nstr` runs without a command, with Home, Notifications and Profile tabs, automatic paging, threads, English and Japanese help, and saved theme and language choices.
- Full-screen posting and NIP-10 replies with Ctrl+S, kind-7 reactions and kind-6 reposts, using key files or NIP-46 bunkers with authorization URLs shown in the client.
- Immediate reaction and repost feedback with rollback when no relay accepts, relay response counts and warning messages, clipboard copying of nevent identifiers, and pictures from content URLs and imeta tags.
- Exit status 2 when the full-screen client cannot use an interactive terminal.

- Commands for individual notes, threads, author profiles, search, reactions, reposts, deletion requests, relationships, public reports and profile updates.
- Saved account configurations with per-command selection, account switching and local read state.
- Stdin posts, side-effect-free draft previews, replies, quotes, content warnings, Blossom attachments with metadata removed, downloads and media fields in JSON output.
- NIP-17 private messages using NIP-44 seals and gift wraps, published DM relay lists, and remote-signer encryption and decryption.
- Full-screen search, author profiles, chat, actions, account management, pinned columns, profile editing with avatar/banner uploads, attachment selection, video playback, downloads, custom emoji and content-warning folding.
- A bounded picture cache, author metadata in lists, unread notification counts and thread read-ahead.
- Exit status 5 for unavailable search configuration, media configuration, DM inboxes and missing events.
- Hashtag searches on normal relays and text searches restricted to configured relays advertising NIP-50, with shared pagination and relay limits.
- NIP-42 relay authentication through key files and NIP-46 bunkers for all publications, including private-message gift wraps.
- Follow notifications alongside mentions, replies, quotes, reactions and reposts.
- Partial read results remain available when the optional mute-list fetch times out.
- Thread read-ahead is invalidated after accepted writes so newly sent replies appear when reopening a thread.
