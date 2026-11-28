# Local response fixtures

Captured from disposable accounts on Misskey 2025.12.0 and Sharkey 2025.4.3 on 2026-11-28 using `scripts/compat/misskey.sh` and `scripts/compat/sharkey.sh`; these accounts and all server data were removed after each run.

The main JSON files contain note, attachment, reaction, emoji and API error responses; local ports, timestamps and the generated search marker are normalized, while response fields and structured error codes are preserved.

The `*-limits.json` files project `maxNoteTextLength` and optional `maxAltTextLength` from public `/nodeinfo/2.1` metadata; Misskey omits the alt limit, while Sharkey advertises 20000 UTF-16 code units.

The `*-upload.json` files contain drive responses for the same PNG uploaded with a new comment both without and with `force`; the first reuses the previous comment and the second keeps the supplied alt text.

The `*-blocked.json` files contain the status and body when the blocked account requests a followers-only note; Misskey returns a hidden note with HTTP 200, while Sharkey returns `NO_SUCH_NOTE` with HTTP 400.

The `*-thread.json` files project `id`, `text`, `replyId` and `repliesCount` from a root note and two actual 100-reply pages; pagination boundaries and IDs are preserved, and unrelated profile and media fields are omitted.

Set `MSKY_CAPTURE_DIR` to retain setup responses under a directory outside the checkout; the runner never captures account creation or token creation responses.
