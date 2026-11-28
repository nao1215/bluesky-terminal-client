# atago Behavior Specs
## Summary
1 suite · 5 scenarios
## Contents
- [msky download](#msky-download): 5 scenarios
  - [download saves each file of a note under the note's name, and prints where](#scenario-download-saves-each-file-of-a-note-under-the-notes-name-and-prints-where)
  - [download writes over nothing, and --json says what each saved file is](#scenario-download-writes-over-nothing-and---json-says-what-each-saved-file-is)
  - [download saves the files of the note a renote or a quote carries](#scenario-download-saves-the-files-of-the-note-a-renote-or-a-quote-carries)
  - [a file not at a web address, too large, or missing is an error and leaves nothing](#scenario-a-file-not-at-a-web-address-too-large-or-missing-is-an-error-and-leaves-nothing)
  - [a note without files, or not written as a note, is a usage error](#scenario-a-note-without-files-or-not-written-as-a-note-is-a-usage-error)

## msky download
Source: `crates/misskey-terminal-client/e2e/atago/download.atago.yaml`
### Scenario: download saves each file of a note under the note's name, and prints where
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `expected/photo.png` is created.
- Fixture file `expected/clip.mp4` is created.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky download ${api.url}/notes/9abc -o 写真👨‍👩‍👧 --host 127.0.0.1:9
```

#### Then
- exit code is `0`
- stdout matches `/^[^\n]*9abc-1\.png\n[^\n]*9abc-2\.mp4\n[^\n]*9abc-3\.bin\n$/`
- stderr is empty
- dir `写真👨‍👩‍👧` contains `9abc-1.png`, contains `9abc-2.mp4`, contains `9abc-3.bin`, has 3 entries
- file `写真👨‍👩‍👧/9abc-1.png` is byte-identical to `expected/photo.png`
- file `写真👨‍👩‍👧/9abc-2.mp4` is byte-identical to `expected/clip.mp4`
- mock `api` received `/api/notes/show`

### Scenario: download writes over nothing, and --json says what each saved file is
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `out/9abc-1.png` is created.
- Environment variables are set: MSKY_DOWNLOAD_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### Inputs
_Fixture `out/9abc-1.png`:_

```text
already here
```

#### When

```shell
msky download 9abc --json --host ${api.url}
```

#### Then
- exit code is `0`
- stdout at `$.postId` equals `9abc`; at `$.type` equals `image`; at `$.url` equals `${api.url}/files/f1`; at `$.path` matches `/9abc-1 \(1\)\.png$/`
- file `out/9abc-1.png` equals exact bytes
- file `out/9abc-1 (1).png` is exactly 242 bytes

### Scenario: download saves the files of the note a renote or a quote carries
#### Given
- Stub HTTP server `api` serves 4 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky download qt1 -o out --json --host ${api.url}
```

#### Then
- exit code is `0`
- stdout matches `/"path":"[^"]*qt1-1\.png","postId":"qt1","type":"image"[^\n]*\n[^\n]*"path":"[^"]*orig-1\.png","postId":"orig","type":"image"[^\n]*\n[^\n]*orig-2\.mp4","postId":"orig","type":"video"/`
- dir `out` contains `qt1-1.png`, contains `orig-1.png`, contains `orig-2.mp4`, has 3 entries

### Scenario: a file not at a web address, too large, or missing is an error and leaves nothing
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Stub HTTP server `big` serves 1 canned route(s) at `${big.url}` and records every request (#24).
- Stub HTTP server `gone` serves 1 canned route(s) at `${gone.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky download bad -o out --host ${api.url}
msky download big -o out --host ${big.url}
msky download gone -o out --host ${gone.url}
```

#### Then
- after `msky download bad -o out --host ${api.url}`:
  - exit code is `4`
  - stderr equals an exact value
- after `msky download big -o out --host ${big.url}`:
  - exit code is `4`
  - stderr contains `is larger than 200 MB, the most msky saves; nothing was written`
  - mock `big` received `/files/v` exactly 0 time(s)
- after `msky download gone -o out --host ${gone.url}`:
  - exit code is `4`
  - stderr contains `/files/missing: HTTP 404`
  - dir `out` does not exist

#### Expected output
_expected stderr:_

```text
error: file:///etc/passwd is not an http(s) address; nothing was written
```

### Scenario: a note without files, or not written as a note, is a usage error
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
msky download plain -o out --host ${api.url}
msky download ${api.url}/@alice --host ${api.url}
```

#### Then
- after `msky download plain -o out --host ${api.url}`:
  - exit code is `1`
  - stderr equals an exact value
- after `msky download ${api.url}/@alice --host ${api.url}`:
  - exit code is `1`
  - stderr contains `is not a note`
  - mock `api` received `/api/notes/show` exactly 1 time(s)

#### Expected output
_expected stderr:_

```text
error: ${api.url}/notes/plain has no files
```

