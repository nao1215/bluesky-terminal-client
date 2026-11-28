# Live-server test catalog (shared by bsky, msky, mstdn and nstr)

Every compatible system a client claims is checked with the same catalog, written as atago specs against a server or relay started locally by `scripts/compat/<system>.sh`. A system counts as usable with a client when the Usable tier passes; the other tiers are reported in `doc/compat.md` per system. Each spec names its tier in the scenario name (`usable: ...`, `heavy: ...`, `speed: ...`, `behavior: ...`) so the matrix can be generated from atago's JSON report. A missing check remains not verified, even when every implemented scenario passes.

## Seeding

The script seeds the server through its public API, as two or three throwaway accounts that follow each other, before the specs run: text posts, posts with 1 to 4 pictures, posts with a video (a short H.264 test pattern made with ffmpeg in the script, plus one in a codec the clients cannot play), long threads, posts with content warnings, every visibility, mentions, hashtags, custom emoji where the system exposes a public creation API, and notifications produced by the second account. Seeding is idempotent per run and its counts are printed, so a failing spec can name what it expected. A server that transcodes the second video into a supported codec cannot verify the unsupported-codec viewer case with that upload.

## Usable (must pass to call the system usable)

- Log in or use a token made by the script; whoami matches the account.
- Read the home timeline and, where the system has them, local and federated or global; posts from the seeded accounts appear in order, newest first.
- Page through at least three pages: no duplicates (compare IDs with `--json`), no gaps against the seeded count, the end is reached.
- Post text, reply, open the thread with the reply in it, delete your post where the client has delete, including a TUI-only action.
- Favourite or react, and boost or repost or renote, and take each back; the counts on the post change and change back.
- Upload a picture with alt text and read the post back with the alt text; upload the test video and read it back as a video.
- Content warning and visibility are sent and read back as sent.
- Notifications show the mention, reply, favourite or reaction, boost, and follow the second account made. Mastodon-family replies addressed to an account are mention notifications, not a separate reply type.
- Search a hashtag (and words or accounts where the system has them).
- The full-screen client, in a pty with `graphics: kitty`, opens on the seeded timeline, moves, opens a thread, opens the pictures and plays the video (`images: { min_count: 1 }`), composes and sends a post, quits with status 0.
- Anything the system does not have is reported by the client as unsupported with a message naming the feature and server, never as a network error, and the rest keeps working. An ambiguous resource 404 or permission refusal is not evidence of an absent feature.

## Heavy

- A timeline of only video posts (at least 100): the full-screen client scrolls through it from top to bottom and back, every visible video post shows its thumbnail, the selected video plays in the viewer, and the client stays responsive (each move answered within the bound below). Repeat with the inline timeline playback when the client has it.
- A timeline where every post has four pictures (at least 100).
- A thread 50 replies deep, and a post with 200 replies.
- Posts at the system's own limits: the longest text, the most pictures, the largest file, the longest alt text; one over each published, locally enforceable limit is refused before upload with the system's limit in the message. Treat an exclusive byte ceiling as accepting one byte below the advertised value and rejecting the value itself. Report a lower client limit separately, and do not claim the server limit was tested.
- Text with ZWJ emoji, skin tones, flags, VS16, CJK, RTL, combining marks and very long words, in posts, names and alt text; the screen never breaks its layout (no cell spills over the border, the hint row stays intact).
- A notification list of at least 500, paged to the end.
- A video in a codec the client cannot play: the viewer says why and offers saving, and nothing hangs.

## Speed (bounds are orders of magnitude, generous enough for shared CI runners)

- CLI: `timeline -n 100 --json`, `notifications -n 100 --json`, `search`, `post` with a picture, each under a stated bound (atago duration on the run step); heavier comparisons and regressions go to the existing himorime benchmarks.
- Full-screen client: first timeline paint, first picture, first video frame after space, and the time a move takes on the heavy timelines, each as an `expect_screen` with its own timeout.
- The bounds and the measured values per system go to `doc/compat.md`, so a slow system shows as slow rather than failing silently. A scenario duration is not an individual frame or key-response duration.

## Behavior

- Ordering and deduplication across pages and after refresh.
- A write is sent once (idempotency keys where the system has them); a retried read does not duplicate anything.
- Rate limits: with the server's rate limit lowered by the script where the system allows it, the client waits as Retry-After or the reset header says and reports it; a write is never resent.
- Errors: a wrong token, an expired token where tokens expire, a deleted post, a blocked account and a server restart mid-session each give the client's documented message and exit status.
- Media processing: a video the server is still processing is waited for, within a bound, and reported if it never finishes.
- Federation-only features that a single local server cannot show are listed as not verified, not as passed.

## Where atago falls short

If a check here cannot be written declaratively in atago (for example frame rate, memory, or counting how many distinct video frames reached the screen), do not work around it with shell or a Python assertion driver: describe the missing atago feature (what spec key, how it behaves, which spec needs it) in the report, so it can be added to atago itself.

## Mastodon-family clarifications

The catalog distinguishes a server's public emoji-creation API from its administrative web UI, checks the codec that survived server transcoding, treats addressed replies as mention notifications, includes TUI-only deletion, distinguishes hidden resources from unsupported endpoints, separates lower client limits from published server limits and exclusive byte ceilings, conditions expiry checks on expiring tokens, and distinguishes scenario timings from frame timings. These clarifications change no successful check into an assumed pass.
