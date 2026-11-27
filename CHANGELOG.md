# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `msky author USER`: an account's notes, newest first, read a page of up to 100 at a time with `untilId` until `-n` notes came (20 by default). USER is `name`, `@name`, `@name@host`, or the account's address. Replies and renotes are left out unless `--replies` or `--renotes` asks for them, and `--with-files` keeps only the notes with files.
- `msky note NOTE`: one note, by its ID or its address.
- `msky search QUERY`: the notes that match, newest first, paged like `author`.
- `msky download NOTE`: saves the files attached to a note as `<note id>-<n>.<ext>`, in `-o DIR`, `MSKY_DOWNLOAD_DIR`, or `msky` in the Downloads folder. A file is never written over (the next is saved as `name (1).ext`), files over 200 MB and addresses other than http and https are refused, and a file that could not be finished is removed. `msky author USER --download` (or `-o DIR`) saves the files of every note it lists.
- `--host` and `MSKY_HOST` choose the server, misskey.io by default; an address given for a note or an account is read on its own server.
- `--json` prints one note per line in the same shape for every command: the time, the text and the content warning, the address, the reactions with their total, the renote and reply counts, the files with their address, type and size, and the account with its host.
- A call the server turns away for being asked too often (HTTP 429, `RATE_LIMIT_EXCEEDED`) or that a busy gateway drops is tried again up to three times, after the wait `Retry-After` names (a minute at most) or 1, 2 and 4 seconds.
