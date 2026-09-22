# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `bs`, a Bluesky client for the terminal. The timeline shows posts written by accounts you follow and by you, and nothing else: reposts and posts by accounts you do not follow are left out, and a post you send appears on it right away. Avatars and attached photos are drawn inline with kitty graphics, sixel, or iTerm2 inline images, each at its own aspect ratio.
- Posting, replying (the reply keeps the thread's root and names the post it answers), and liking. New posts carry link, mention, and hashtag facets at UTF-8 byte offsets, and the composer enforces Bluesky's 300-character limit before sending.
- Search for posts and for accounts, following and unfollowing from the results, from a post's author, or from a profile, and opening any account's profile with its recent posts.
- A profile editor for the display name, description, and avatar. Saving keeps every other field of the profile record and is guarded with `swapRecord` against the version the editor loaded, so an edit made elsewhere in the meantime is refused rather than overwritten.
- Login with an app password. Only the session tokens are stored, in `session.json` with owner-only permissions; an expired access token is refreshed once and the new tokens are saved. `bs logout` removes the session.
- A terminal without an image protocol is refused at startup with exit status 2; `BS_GRAPHICS` forces a protocol for terminals that support one but do not answer the query. Usage errors exit 1 and a session file that cannot be read or parsed exits 3, each with an `error:` line and, where there is a next step, a `hint:`.
- Replies on the timeline are shown under the posts they answer: the parent, and the thread's first post with a gap marker when the reply sits deeper. `v` opens a post's thread: the posts above it, the post, and every reply expanded with indentation, with placeholders for deleted or blocked posts. Like, repost, reply, and follow work on any post in it.
- A Notifications tab (`3`) shows likes, reposts, follows, mentions, replies, and quotes with the posts they are about, counts the unread ones on the tab, and marks them seen up to the newest one shown, in the server's time rather than the local clock. Replies, mentions, and quotes can be answered and liked from it.
- Repost and remove a repost with `b`.
- Lists load their next page as the selection nears the end: the timeline, search results, a profile's posts, and notifications. A page is never loaded twice, repeated posts are skipped, and a page for a list that has since been refreshed is dropped.
- 42 color themes in a picker opened with `T`, previewed live and saved in `settings.json`: Bluesky's own looks (`bluesky`, the default, `bluesky-dark`, `bluesky-light`), well-known editor themes such as Dracula, Nord, Gruvbox, Solarized, the Catppuccin, Rosé Pine, and Tokyo Night families, One Dark, GitHub, Monokai, Kanagawa, and Everforest, the terminal's own palette (`terminal`), and `monochrome`. A test keeps every theme's text readable against its background. A `settings.json` that cannot be read is reported and never overwritten. 24-bit themes are mapped to 256 colors on terminals without 24-bit color, and `NO_COLOR` turns color off.
- Errors are shown in a box in the middle of the screen, over everything, and close with the next key (which still does what it does) or by themselves after ten seconds; passing messages stay on the status row. The theme picker shows ten themes at a time and scrolls, with arrows for what is above and below.
- Links open in the web browser: `Space` on a post without pictures or video opens its link, and `o` opens the link of any post (its link card, else the first link in its text). Only http(s) links are opened, with the system's own opener or `BS_BROWSER`.
- The error box shows the hint that says what to do, not only what went wrong.
- Post results of a search, and the posts of a thread, say whether each author is followed (`✓ following` / `not following`), and `f` follows or unfollows the author from there too.
- The key hints wrap onto up to three rows on a narrow screen instead of being cut off; a hint is never split.
- A hint row under every view with the keys that work there, and a `?` help grouped by view that scrolls and closes only with Esc, `q`, or `?`.
- Up to four pictures on a post, with alt text and their aspect ratio. `Ctrl+O` in the composer opens a folder browser that lists folders and pictures and previews the selected one; `Space` marks pictures across folders. Each picture is turned upright from its EXIF orientation, scaled to at most 2000 pixels, re-encoded under 1 MB without its metadata, and all are read before any is uploaded. The profile editor chooses the avatar with the same browser.
- One video (MP4, MOV, WebM, MPEG; up to 100 MB and 3 minutes) or animated GIF on a post, with alt text and its shape read from the file's header. It is uploaded through Bluesky's video service with a token the PDS issues for it, and bs waits for the service to process it; the service turns a GIF into a video, so bs needs no codec. The browser and the composer describe a video on disk by its length, shape, and size, and a post has up to four pictures or one video, not both. Before uploading, bs asks the service's upload limits, so a refusal (an unconfirmed email address, the daily allowance) is reported with its reason before the file is sent.
- A viewer on `Space`: a post's pictures full screen at their own shape, with their alt text, or its video played without sound, from Bluesky's HLS stream and decoded by OpenH264 built into bs (no external program). A video bs cannot play shows its thumbnail with a warning. `r` plays a video again, `d` saves the picture or video in `Downloads/bs`, `Esc` goes back.
- Faster scrolling: pictures are scaled and encoded for the terminal on background threads instead of while drawing, the pictures of the next posts are prepared before they scroll into view and those of the 20 after them downloaded, lists draw Bluesky's small avatar thumbnails instead of the full-size avatars (about a tenth of the bytes), the next page is asked for 10 posts before the end, and keys pressed faster than a frame draws are applied together.
- Notifications load in the background at the start, so their tab shows the unread count at once and opens without waiting; they are marked seen only when the tab is visited, and a failure there waits on the tab instead of interrupting the timeline. Downloaded pictures are kept on disk (`BS_CACHE_DIR`, 256 MB, least recently used removed first), so the next start draws them without downloading.
- Esc on a profile opened from the search results, the timeline, or the notifications goes back there, with the results and the selection as they were.

### Fixed

- Typing on the Search tab ran the letters as commands (`q` quit, `l` liked a result) unless `/` was pressed first. Arriving at an empty Search tab now puts the cursor in the box, and `Tab` leaves it.
- A status message such as "liked" replaced the key hints until the next message. Messages now have their own row and clear after a few seconds.
