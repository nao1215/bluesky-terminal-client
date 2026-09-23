# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Several accounts can be logged in. `A` lists them with the one in use marked, switches to another (its timeline, notifications and profile, nothing of the one before), logs in another, and logs one out after a `y`. `bsky -a <handle>` (or `BSKY_ACCOUNT`) starts on an account without changing the one in use, `bsky logout` logs out the one in use or the one `-a` names, and `bsky logout --all` every one. Each account's tokens are in a file of their own under `accounts/` in the config folder; the `session.json` of earlier versions becomes the first account the first time this version starts, and an earlier version started afterwards finds no login.

## [0.3.0] - 2026-09-23

### Added

- `.` lists everything the keys do to the selected post, each saying what it would do now (like it or remove your like, follow or unfollow), and runs the one you choose. The key itself still works, so the list teaches the keys rather than replacing them.
- `Q` quotes the selected post: the composer says whose post it quotes, and the post that goes out carries it. A quote with pictures of its own is sent as one embed carrying both.
- `D` deletes your own post, after a `y` that confirms it: any other key keeps the post. The hint row offers it only on a post of yours, and the deleted post goes from every list it was in.
- `c` copies the selected post's address on bsky.app to the clipboard, through the terminal itself (OSC 52), so it works over ssh and in tmux with `set-clipboard on`.
- A terminal smaller than 24 columns by 8 rows shows what size bsky needs and what size the terminal is, instead of a screen with shreds of the tabs, a post, and the key hints on it. The client comes back as soon as the window is made bigger.
- A settings screen, opened with `s` on your own Profile tab, where two buttons say it and the profile editor exist. The theme can be chosen there, and pictures turned off (posts then say what they carry, as on a terminal that cannot draw them) or back on without a restart; both are kept in `settings.json`. The download folder, picture cache, video service and browser are shown with where their values come from. A setting that a `BSKY_` variable fixes says so and is not changed.
- The download folder, the picture cache, the video service and the browser can be changed on the settings screen: the folders are chosen in a browser that lists folders only, and a folder that cannot be written is refused with the reason; the service and the browser are typed. Each takes effect at once and is kept in `settings.json`, and `x` puts it back to its default. A `BSKY_` variable that is set still wins.

### Changed

- The key hints are one row and keep to the few keys of the view: the keys that act on a post moved behind `.`. The row used to take three rows of a narrow terminal and was cut anyway.
- `o` opens the post itself on bsky.app when it carries no link, instead of saying there is nothing to open. `Space` is unchanged: it shows the pictures or the video, and says when there are none.

### Fixed

- A mention or a link written in a parenthesis straight after an address, as text without spaces puts them (`(https://example.com/a:(@alice.test)`), is sent as its own facet. The handle used to be spelled into the link, so the post carried an address nobody can reach and mentioned nobody.
- The composer on a screen too short for the thumbnails lists the pictures it will send. The rows the thumbnails would have taken were kept empty, so on a 26 by 10 terminal only the first picture was named and on a smaller one none were, while the post still carried them all.
- The `?` help is readable on a narrow terminal: each description goes under its keys where there is no room beside them, and a description too long for its column wraps instead of losing its end. On a 26 column terminal every description used to be cut to four cells.
- A quote post that carries a picture of its own says whose post it quotes. The quote was dropped, so such a post looked like an ordinary picture post.
- A quote of a post that is not found, is from an account you cannot see, or was detached by its author says which of those it is, instead of showing nothing. A quote of a feed or a list is named too.
- A quote of a post that carries a picture or a video shows it, and `Space` opens it full screen, the way the quoted post itself would. Only the quoted words were shown before. A quote that adds a picture of its own still shows that one.
- A quote of a starter pack is named on one line with the pack's name, as a quote of a feed or a list is. It used to say the quoted post cannot be shown.
- Without pictures (on a terminal that cannot draw them, or with pictures off in the settings), the first letters of a name and of a post's text keep their weight and color. The empty place of the avatar was painted over them.
- `d` saves a picture with the extension of what it is (png, jpg, gif, webp), and does not save what is not a picture. The name used to come from the picture's address as it was, so a post whose picture address ended in `.exe` saved a file named `.exe` with whatever the server sent.
- The theme picker opened with `T` closes back to the list. After a picker opened from the settings screen was closed by the session expiring, the next one opened the settings screen when it closed.
- A delete asked with `D` is called off when the session expires before the `y`. The login form took the keys meanwhile, and the first `y` after logging in again deleted the post.
- The list `.` opens acts only on the post it was opened on. When that post left the list while it was open (a reload without it), the selection moved to another post, and the list's keys acted on that one.
- A theme being previewed in the picker goes back to the one in use when the session expires and the picker closes, as Esc does. It used to stay on screen as if chosen, though `settings.json` did not hold it.

## [0.2.0] - 2026-09-23

### Changed

- A terminal without kitty graphics, sixel, or iTerm2 inline images gets the client as text instead of being refused with exit status 2. Posts say how many pictures they carry, with their descriptions, or that they have a video; no avatar column is kept and nothing is downloaded to be drawn; `Space` opens the post on bsky.app in the web browser, and the key hints and `?` help say so and leave the viewer out. Exit status 2 now only means bsky was not started in an interactive terminal.
- `--service` and `BSKY_SERVICE` take an http or https URL with a host and nothing else, and the scheme may be written in any case. A URL with a path, a query, or a fragment used to be accepted and then made every request go to an address that does not exist.

### Fixed

- `R` in a thread keeps the reply that was selected instead of moving back to the post the thread was opened on, so the next `l`, `b`, or `r` acts on the reply you were reading.
- A video a little over three minutes long says it runs 3:01, not the 3:00 that is allowed.
- A failed save of the session file leaves no `session.json.<pid>.tmp` behind holding the tokens.
- An invalid service URL is quoted in the error the way it was typed.
- A video whose playlist names its files the scheme-relative (`//host/path`), absolute-path, query-only, or `..` way plays and saves. Those forms used to be joined to the playlist's address as text, which asked the server for a path that does not exist, so the video showed only its thumbnail with a warning.
- A post or a profile whose send fails because the session expired keeps what you typed: the composer or the profile editor is behind the login form, and after logging in as the same account it is there to send again. The draft used to be dropped when the login form came up. Logging in as another account drops it, as it drops the rest of that account's state.
- A hashtag followed straight away by a handle or a link, as text without spaces puts them (`#Rust【@alice.test】`), sends the tag as the word it is and the handle as a mention. The tag used to run to the next space, so it carried the handle and nobody was mentioned.
- A QuickTime video whose file does not start with an `ftyp` box, which is what QuickTime wrote before 2001 and what some cameras and editors still write, is posted instead of being refused as not a video.

## [0.1.1] - 2026-09-23

### Changed

- Loading runs beside other loads and beside likes, posts, and uploads. A thread, a profile, a search, or the next page no longer waits behind the notifications and feeds loaded at the start, a slow page, or a video being uploaded, and a like is sent while a load is still waiting. With every answer taking 400 ms, opening a thread right after the timeline appears went from 2.4 s to 0.9 s.
- A profile asks for the account and its posts at once instead of one after the other. With every answer taking 400 ms, opening a profile from the timeline went from 1.3 s to 0.9 s.

### Fixed

- Removing a like, a repost, or a follow deletes only that kind of record in your own account. When the server named a like, repost, or follow that was another record, such as one of your posts, bsky deleted that record; it now shows an error and sends nothing.
- A post whose record another client wrote with a damaged reply field (a reply without its CIDs) or a time that is not a string is shown with its text instead of blank, and a reply to it names that post as root and parent.
- `d` saves under a new name when anything already has the chosen one, a link to nothing included. It used to write through such a link, creating or replacing the file the link pointed to outside the download folder, and a file created between the check and the write was overwritten.
- A download is never named after a device Windows reserves (`CON`, `NUL`, `COM1`...), never ends in a dot or has an empty extension, and a `#` part of the URL no longer ends up in the name.
- One post with a field of the wrong type (a count that is null or a string, an avatar or a like that is a number, a missing handle) no longer fails the whole timeline, feed, search, thread, or notifications page it is on; the post shows as if the field were missing, and an item that is not a post at all is left out.
- A picture whose alt text is null, or whose aspect ratio is written as strings, no longer hides every picture of its post, and a video whose aspect ratio has fractions is shown.
- Tabs in a post are drawn as spaces and a CR as a line break, instead of vanishing and gluing the words together; the profile editor and the search box lay out text from the server with CRLF or tabs the way they draw it, so the cursor stays where the text is.
- A name or header cut on a narrow screen ends in a single `…`: a wide character no longer leaves `……`, and a name that fills the width exactly no longer drops the rest of the header without one.

## [0.1.0] - 2026-09-22

### Added

- `bsky`, a Bluesky client for the terminal. The timeline shows posts written by accounts you follow and by you, and nothing else: reposts and posts by accounts you do not follow are left out, and a post you send appears on it right away. Avatars and attached photos are drawn inline with kitty graphics, sixel, or iTerm2 inline images, each at its own aspect ratio.
- Posting, replying (the reply keeps the thread's root and names the post it answers), and liking. New posts carry link, mention, and hashtag facets at UTF-8 byte offsets, and the composer enforces Bluesky's 300-character limit before sending.
- Search for posts and for accounts, following and unfollowing from the results, from a post's author, or from a profile, and opening any account's profile with its recent posts.
- A profile editor for the display name, description, and avatar. Saving keeps every other field of the profile record and is guarded with `swapRecord` against the version the editor loaded, so an edit made elsewhere in the meantime is refused rather than overwritten.
- Login with the account's password or an app password (the login screen recommends the latter). Only the session tokens are stored, in `session.json` with owner-only permissions; an expired access token is refreshed once and the new tokens are saved. `bsky logout` removes the session.
- A terminal without an image protocol is refused at startup with exit status 2; `BSKY_GRAPHICS` forces a protocol for terminals that support one but do not answer the query. Usage errors exit 1 and a session file that cannot be read or parsed exits 3, each with an `error:` line and, where there is a next step, a `hint:`.
- Replies on the timeline are shown under the posts they answer: the parent, and the thread's first post with a gap marker when the reply sits deeper. `v` opens a post's thread: the posts above it, the post, and every reply expanded with indentation, with placeholders for deleted or blocked posts. Like, repost, reply, and follow work on any post in it.
- A Notifications tab (`3`) shows likes, reposts, follows, mentions, replies, and quotes with the posts they are about, counts the unread ones on the tab, and marks them seen up to the newest one shown, in the server's time rather than the local clock. Replies, mentions, and quotes can be answered and liked from it.
- Repost and remove a repost with `b`.
- Lists load their next page as the selection nears the end: the timeline, search results, a profile's posts, and notifications. A page is never loaded twice, repeated posts are skipped, and a page for a list that has since been refreshed is dropped.
- 42 color themes in a picker opened with `T`, previewed live and saved in `settings.json`: Bluesky's own looks (`bluesky`, the default, `bluesky-dark`, `bluesky-light`), well-known editor themes such as Dracula, Nord, Gruvbox, Solarized, the Catppuccin, Rosé Pine, and Tokyo Night families, One Dark, GitHub, Monokai, Kanagawa, and Everforest, the terminal's own palette (`terminal`), and `monochrome`. A test keeps every theme's text readable against its background. A `settings.json` that cannot be read is reported and never overwritten. 24-bit themes are mapped to 256 colors on terminals without 24-bit color, and `NO_COLOR` turns color off.
- Errors are shown in a box in the middle of the screen, over everything, and close with the next key (which still does what it does) or by themselves after ten seconds; passing messages stay on the status row. The theme picker shows ten themes at a time and scrolls, with arrows for what is above and below.
- A read that fails the way a busy server fails (`UpstreamFailure`, HTTP 502 to 504) is tried once more after a second; a write is never repeated.
- The viewer shows a picture's thumbnail at once and the full size as soon as it has downloaded, and pictures wanted on screen are downloaded before those fetched ahead.
- Links open in the web browser: `Space` on a post without pictures or video opens its link, and `o` opens the link of any post (its link card, else the first link in its text). Only http(s) links are opened, with the system's own opener or `BSKY_BROWSER`.
- The error box shows the hint that says what to do, not only what went wrong.
- Post results of a search, and the posts of a thread, say whether each author is followed (`✓ following` / `not following`), and `f` follows or unfollows the author from there too.
- The key hints wrap onto up to three rows on a narrow screen instead of being cut off; a hint is never split.
- A hint row under every view with the keys that work there, and a `?` help grouped by view that scrolls and closes only with Esc, `q`, or `?`.
- Up to four pictures on a post, with alt text and their aspect ratio. `Ctrl+O` in the composer opens a folder browser that lists folders and pictures and previews the selected one; `Space` marks pictures across folders. Each picture is turned upright from its EXIF orientation, scaled to at most 2000 pixels, re-encoded under 1 MB without its metadata, and all are read before any is uploaded. The profile editor chooses the avatar with the same browser.
- One video (MP4, MOV, WebM, MPEG; up to 100 MB and 3 minutes) or animated GIF on a post, with alt text and its shape read from the file's header. It is uploaded through Bluesky's video service with a token the PDS issues for it, and bsky waits for the service to process it; the service turns a GIF into a video, so bsky needs no codec. The browser and the composer describe a video on disk by its length, shape, and size, and a post has up to four pictures or one video, not both. Before uploading, bsky asks the service's upload limits, so a refusal (an unconfirmed email address, the daily allowance) is reported with its reason before the file is sent.
- A viewer on `Space`: a post's pictures full screen at their own shape, with their alt text, or its video played without sound, from Bluesky's HLS stream and decoded by OpenH264 built into bsky (no external program). A video bsky cannot play shows its thumbnail with a warning. `r` plays a video again, `d` saves the picture or video in `Downloads/bsky`, `Esc` goes back.
- Faster scrolling: pictures are scaled and encoded for the terminal on background threads instead of while drawing, the pictures of the next posts are prepared before they scroll into view and those of the 20 after them downloaded, lists draw Bluesky's small avatar thumbnails instead of the full-size avatars (about a tenth of the bytes), the next page is asked for 10 posts before the end, and keys pressed faster than a frame draws are applied together.
- Notifications load in the background at the start, so their tab shows the unread count at once and opens without waiting; they are marked seen only when the tab is visited, and a failure there waits on the tab instead of interrupting the timeline. Downloaded pictures are kept on disk (`BSKY_CACHE_DIR`, 256 MB, least recently used removed first), so the next start draws them without downloading.
- Esc on a profile opened from the search results, the timeline, or the notifications goes back there, with the results and the selection as they were.
- Custom feeds on the Timeline tab: `[` and `]` go through Following and the feeds pinned in the account's saved feeds, in their pinned order (Discover when none are pinned). A feed shows every post it serves, loads when it is first shown, keeps its place and its next pages, and `R` refreshes the one on screen.
- A bare domain in a new post, such as `example.com` or `docs.bsky.app/blog`, is linked with `https://` in front, as Bluesky's app links it: only when its last label is a top-level domain on the list the app uses, so `file.txt` and `v1.2` stay text.

### Changed

- The command is `bsky`, not `bs`, and so are its folders (`~/.config/bsky`, the cache, `Downloads/bsky`) and the prefix of its environment variables (`BSKY_CONFIG_DIR`, `BSKY_GRAPHICS`, and the rest). A config folder left by `bs` is moved to the new name on the first start, so the login is kept.
- Pictures up to 2048 pixels (every full-size Bluesky picture, and most screenshots) are no longer scaled down after decoding, a step that cost as much as decoding them. A screen of twelve such pictures is ready about 40% sooner; larger photos from a camera are still scaled down once.
- Pictures and video frames are scaled to their box with a faster scaler before they are encoded for the terminal. A full-screen 720p video frame is ready in 5 ms instead of 20 with kitty (43 instead of 63 with sixel, which now keeps up with 15 frames a second), and a screen of twelve photos in 26 ms instead of 63.
- The top row starts with the tabs: the ` bsky ` label before them is gone.
- A video starts from the first part of its first segment instead of after the whole segment has downloaded, which on a slow link was most of the wait, and the next video reuses the connections the last one opened.
- Pictures arrive sooner, most of all on a slow link. The connections to Bluesky's picture server are opened while the first list loads, and kept for two minutes instead of 15 seconds, so a post read for a while does not make the next pictures wait for new TCP and TLS handshakes; a download that fails on a connection the server had just closed is tried once more. With a 0.4 s round trip, the first screen's twelve pictures took 1.9 to 2.3 s instead of 3.5 to 4.4 s.

### Fixed

- Typing on the Search tab ran the letters as commands (`q` quit, `l` liked a result) unless `/` was pressed first. Arriving at an empty Search tab now puts the cursor in the box, and `Tab` leaves it.
- A status message such as "liked" replaced the key hints until the next message. Messages now have their own row and clear after a few seconds.
- A hashtag followed by full-width or other non-ASCII punctuation, such as `#Rust！` or `#タグ』`, was sent with the punctuation in the tag, so it linked to a different tag than the official app makes. The tag now ends before the punctuation, and a run of only digits and punctuation such as `#1.5` stays plain text, as in the official app.
- A shortened line (a link title, a quoted post, a name, a folder path) could end in the middle of an emoji or a flag, leaving half of it on screen. Lines are now cut between whole characters.
- In the composer, the search box, and the profile editor, Backspace after an emoji with a skin tone removed only the skin tone, a family emoji or a flag was deleted one piece at a time, the arrow keys could stop inside an emoji so typed text split it, and the cursor was drawn several columns right of an emoji. Emoji and accented letters are now edited, stepped over, and wrapped as one character.
- A link, mention, or tag in full-width brackets or followed by full-width punctuation, such as `（https://example.com）` or `https://example.com！`, was not linked or took the punctuation with it. The brackets and punctuation are now left out, and a URL keeps a full-width parenthesis it opened itself.
- After the terminal was made smaller and then larger, or near the end of a list, the posts above the selection stayed hidden while the lower part of the screen was empty. The screen is now filled from above when the list ends on it.
- Selecting a GIF in the folder browser, or attaching one, decoded its frames to tell whether it is animated, on a canvas of the size its header claims. A small or damaged GIF claiming a huge canvas froze the screen for seconds and asked for gigabytes of memory. Whether a GIF is animated is now read from its block structure without decoding it.
- A hashtag typed with the full-width ＃ of a Japanese or Chinese keyboard was not linked; it is now, as in Bluesky's app. The keycap emoji #️⃣ is no longer sent as a hashtag, and a tag ends at a zero-width character, so the same text gets the same tag whichever app posted it.
- A link or mention in parentheses glued to the text before it, as in `説明はこちら(https://example.com)` or `詳細は（@alice.test）まで`, was not linked. A link followed by Japanese text without a space, as in `https://example.com、あと`, took the text into the link; it now ends at the first full-width punctuation mark.
- A post of many emoji could fit the 300-character limit and still be refused by the server after it was sent, because a post may also be at most 3000 bytes and an emoji takes up to 25. The composer checks both limits before sending and shows the byte count once it nears the limit.
- After replying to a post further down the timeline, or refreshing it, the selection jumped back to the first post. It now stays on the post it was on when that post is still in the list.
- On Windows, emoji and other characters outside the Basic Multilingual Plane can be typed and pasted: crossterm 0.29 dropped them there, so bsky now reads their records from the console input itself (#20).
- In kitty, the avatars and photos of the list could be blank after a video or full-size pictures were shown with `Space`: every picture of a video was sent as a new image, kitty dropped the oldest images once its store was full, and bsky drew only their places. A video's pictures now replace one another in kitty, and closing the viewer sends the pictures on screen again.
