# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `msky author ACTOR`: an account's notes, newest first, read a page of up to 100 at a time with `untilId` until `-n` notes came (20 by default), each note once. ACTOR is `name`, `@name`, `@name@host`, or the account's address. The options are those of bsky's `author`: `--filter with-replies` (default), `no-replies`, `media` (notes with files), `video` and `threads` (both told by msky from the notes, as Misskey cannot filter by them), and `--reposts` to keep renotes, each under a `⟳ renoted by @name` line.
- `msky note NOTE`: one note, by its ID or its address.
- `msky search QUERY`: the notes that match, newest first, paged like `author`.
- `msky download NOTE`: saves the files attached to a note and to the note it renotes or quotes, as `<note id>-<n>.<ext>`, in `-o DIR`, `MSKY_DOWNLOAD_DIR`, or `msky` in the Downloads folder, and prints each path (with `--json`, a line of `note`, `kind`, `url` and `path` each). A note without files is a usage error. A file is never written over (the next is saved as `name (1).ext`), files over 200 MB and addresses other than http and https are refused, and a file that could not be finished is removed. `msky author ACTOR --download` (or `-o DIR`) saves the files of the notes it read instead of printing them.
- `--host` and `MSKY_HOST` choose the server, misskey.io by default; an address given for a note or an account is read on its own server.
- `--json` prints one note per line in the same shape for every command: the time, the text and the content warning, the address, the reactions with their total, the renote and reply counts, the files with their address, type and size, and the account with its host.
- `msky login`: logs in with MiAuth, with read-only access (`read:account`, `read:notifications`, `read:following`, `read:reactions`). It prints the address to open on the server, opens it in the web browser when stderr is a terminal (`MSKY_BROWSER` names the program), and asks `miauth/<session>/check` every two seconds until the access is allowed. Access not allowed within `--timeout` seconds (300 by default), or denied, is exit 1 and nothing is saved. msky never handles a password.
- The token is saved per server, as bsky saves its sessions: `accounts/<host>.json` in `MSKY_CONFIG_DIR` or `msky` in the platform's config folder, owner-only (0600) on Unix and written atomically, with `accounts.json` naming the account in use. Without `--host` or `MSKY_HOST`, msky asks the server of the account in use.
- `msky logout` removes the token of the account in use or of the one on `--host`, and `--all` every one, a broken login file included.
- `msky whoami`: the account logged in on the server (`i`), its address, its counts and its description; `--json` prints a note's `user` with them.
- `msky timeline [home|local|social|global]`: a timeline, paged as `author` pages, each note once, renotes left out unless `--reposts`. `home` and `social` need a login; `local` and `global` are read as a visitor without one.
- `msky notifications`: the account's notifications, newest first, paged the same way and left unread on the server (`markAsRead: false`); `--json` prints `id`, `createdAt`, `type`, `user`, `reaction`, and `note` in the shape every command prints.
- `author`, `note`, `search` and `download` send the token saved for the server they ask, and only to that server; without one they read as a visitor as before.
- A token the server no longer takes, a permission it lacks, and a timeline the server keeps from you each say what to do.
- A call the server turns away for being asked too often (HTTP 429, `RATE_LIMIT_EXCEEDED`) or that a busy gateway drops is tried again up to three times, after the wait `Retry-After` names (a minute at most) or 1, 2 and 4 seconds.
