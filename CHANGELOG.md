# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and this project follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- A long text pasted into the middle of a long draft goes in at once (it took seconds).
- Messages about files, downloads, videos, the network and the settings are in the language chosen too, not only the screens. The `bsky` commands still write English.
- Scrolling a timeline draws each frame about a fifth quicker: post lines are drawn straight into the screen, runs of plain ASCII text are split without the Unicode tables, and times are written without parsing a pattern each time.

### Removed

- A `session.json` of version 0.3 or before, and a config folder named `bs`, are no longer taken over: log in again after updating from those.

### Fixed

- With bsky running in two places (the client and a command in another terminal), the second one to refresh the session sent a spent token, and the session ended. It takes the tokens the other one saved.
- A refresh answered with another account's tokens was saved as this account's, and the next write was sent with them. It is refused, and the login form comes back.
- A picture the encoder could not draw (its panic caught) left the terminal half restored, with the client still drawing; and a crash left the terminal wrapping pasted text in escape codes.
- A handle mentioned many times in one post was looked up once per mention, holding up the post and the writes behind it.
- One pinned feed the server described oddly hid all the pinned feeds from `+`.
- A video was uploaded with where it was filmed (the location a phone writes in it) and its other metadata, and the service was told the name of the file. Both are left out now, as they are for pictures.
- Switching to another account and back while a like, repost, follow or block was on its way lost its answer, so the post looked unliked, and the same key could send it a second time.
- A reply sent in an open thread did not show there; a list of conversations asked for earlier could replace a newer one; a failed mark of notifications as seen was never tried again; and a list the settings opened could reopen the settings after the session expired.
- An account file that could not be read told you to run `bsky logout`, which failed on the same file. `bsky logout --all` removes it, and the message says so.
- `bsky delete`, `like` and the others refused your own post given as an at:// address with your handle instead of your DID.
- A video with no extension, or another one, was taken for a picture.
- A video whose times jumped ahead (at a break in the stream) stopped until the jump had passed, and could not be closed meanwhile.
- A `settings.json` saved with a byte order mark (as some editors do) was ignored whole.
- A video whose file claimed a part far larger than itself stopped every write after it (the post never went, nor did any like or follow until restart).
- `bsky like at://` and other at:// addresses with no account crashed the command; they are refused with a usage message.
- A column of a kind added by a newer bsky was dropped from `settings.json` the next time any setting was saved.
- A conversation shown read, with a message that came after the list was loaded, did not get marked read when opened, and came back as new.
- A mark of notifications as seen that had failed was sent again after a newer one went through, putting the unread notifications back.
- A Following column, and a profile, read again (after posting, unmuting, unblocking or `R`) lost their further pages and went back to the top; `R` on a profile also emptied its posts while it loaded.
- Deleting your post left the post count on your profile as it was.
- `bsky notifications --seen` compared times as text, so a time with fractions of a second could be taken for older, and the newest notification stayed unread.
- A tag written inside a link that follows another tag (`#a,https://x.test/(#b)`) was made both a link and a tag.
- Deleting your reply left the reply count of the post it answered as it was.

## [0.9.0] - 2026-09-24

### Added

- bsky is shown in English, Japanese, Simplified Chinese, Korean, Russian, Spanish, French, German or Portuguese: the language the system asks for (`LC_ALL`, `LC_MESSAGES`, `LANG`), or the one chosen on the settings screen's Language row, kept in `settings.json`. The `bsky` commands for scripts stay in English.

### Changed

- A picture loaded ahead that comes on screen before it is in no longer starts a second download: scrolling onto pictures loaded ahead loaded about 1.7 times as many as it showed.
- Drawing a list is about a third faster: a post's text is wrapped once, not on every frame.

### Fixed

- A new search that failed kept the results of the one before on screen, and moving down them asked the new search for their next page.
- In a long conversation, holding `k` at the top did not load the older messages (only `g` did), and after a first page that failed, the older messages could not be reached at all.
- A conversation marked read showed its unread count again when a list of conversations asked for before it came.
- A video's alt text over 1000 characters was refused only after the video was uploaded, with the post. It is refused before, with the reason.
- A line break in alt text glued the words on either side together where it is drawn on one line.
- A status message too long for the bottom row ran into the "loading…" on its right. It is cut short of it, with an ellipsis.
- In languages with longer words, the key hints lost their last keys (`q`, `esc`) at 80 columns, and the composer and folder browser no longer said how to cancel. The hints take a second row when they need one, and a footer too long for its box leaves out keys from its middle, keeping the last.
- The login form's first lines were cut in some languages; they wrap, and the form grows to hold them.
- Setting names longer than 16 cells were cut on the settings screen; the column is wider.
- A wide character just left of a box (in Chinese, Japanese or Korean) drew over the box's left border.
- In languages other than English, the status after sending a post read like a notification ("… posted"), the follower count was labelled as the ✓ badge is, and a browser named `open` was shown translated on the settings screen.
- A tag ending in punctuation of a script other than Latin or CJK (Arabic `؟`, Hebrew `׃`, `〽`, the vertical and small forms) kept the mark in the tag it sent, unlike Bluesky's app; and a tag of 33 to 64 emoji was sent where the app sends none. Tags end at any Unicode punctuation now, and their 64 are counted as the app counts them.
- Arabic text with the lam-alif ligature `لا` could run past the edge of its column or the screen: text is measured as it is drawn, a character at a time.
- In a text box, a character typed in front of a flag or before an emoji it joins left the cursor inside the new emoji, and the next key went elsewhere. The cursor is drawn where the next character will go, also before a wide character moved to the next line and at the end of a full line.

## [0.8.0] - 2026-09-24

### Added

- The settings screen has an Account row: Enter opens the account list `A` opens, to switch, add or log out an account, and Esc goes back to the settings.

### Changed

- The pinned feeds are asked for when `+` is first pressed, not at every start: one request fewer before the timeline shows.
- The `?` help lists `+` once, with the other column keys.

### Fixed

- After `M` muted an account from a list, the status said "M again unmutes", but the next `M` acted on the author of the post selected next, and muted them. It says to press `M` on their profile now, and `B` likewise. An unmute or an unblock loads the timeline again, so their posts come back.
- A reload sent after a like or a delete, before the server answered it, could undo it on screen: the next `l` liked again, or the deleted post came back.
- Columns that failed because the session expired stayed failed after logging in again.
- A post sent while your profile was open, or a reply sent in an open thread, did not show there until it was loaded again.
- Following or unfollowing from a profile left its follower count as it was.
- A download started before switching accounts did not say where it was saved.

## [0.7.1] - 2026-09-24

### Fixed

- Unfollowing an account left its posts in the Following column, and a post you sent did not show in the Following column or a column of your posts until they were loaded again.
- The count of columns off screen to the right was cut off when the title before it was long.
- A `settings.json` holding a column of a kind this version does not know was ignored whole, theme included, and nothing more could be saved. Only that column is left out now.
- A list loaded again while its next page was on its way could ask for its next page twice.
- A post date that was not a date was printed as it was, control characters included, by the `bsky` commands: a post could send escape sequences to your terminal. They are left out.
- A handle written with `@` in front, in `bsky login` or the login form, failed with "Invalid identifier or password".
- `bsky chat -n 0` failed with InvalidRequest, and `-n` over 100 showed at most 100 conversations or messages.
- An account logged out while a load of it was still on its way could be back at the next start.
- Right after `a` logged in another account, a read receipt for what was still on screen could be sent as the new account.

## [0.7.0] - 2026-09-24

### Changed

- The Timeline tab shows the accounts you follow and no longer has a row of feeds switched with `[` and `]`. Your pinned feeds, or Discover when none is pinned, are offered by `+` and shown as columns beside the timeline.

## [0.6.0] - 2026-09-24

### Changed

- The Columns tab is gone: `+` on the Timeline tab adds a column beside the timeline, which becomes a column of its own, and the Timeline tab shows them side by side from then on. `x` removes a column; with none left the timeline is shown alone again. Columns kept from 0.4 or 0.5 are shown on the Timeline tab.
- The tabs are Timeline, Chat, Search, Notifications and Profile, on `1` to `5`: Chat is next to the Timeline.

### Fixed

- The Chat tab and `bsky chat` failed with "MethodNotImplemented" for an account logged in through bsky.social: the chat calls went to the entryway, which does not pass them on. They go to the account's own PDS, named by its DID document.
- A post in a thread showed twice in a row on the timeline: once above the reply to it, and again as the next post. It is shown once, above the reply.

## [0.5.0] - 2026-09-24

### Added

- Mute and block accounts. `M` mutes the author of the selected post, or the account whose profile is shown, and `M` again unmutes; `B` blocks after a `y` that confirms it, and `B` again unblocks. Their posts and notifications leave the lists on screen at once, and a profile says "muted" or "blocked". Both are in the `.` list with what they would do now. On the command line: `bsky mute`, `unmute`, `mutes`, `block`, `unblock` and `blocks`.
- `bsky likes POST` and `bsky reposts POST` print who liked and who reposted a post; `bsky lists [ACTOR]` prints the lists an account made (name, purpose, number of members, description, URI), and `bsky list LIST` a list's members. A list is named by its at:// URI or its bsky.app address.
- `bsky report TARGET --reason spam|violation|misleading|sexual|rude|other [--comment TEXT]` reports a post or an account to Bluesky's moderators. `bsky app-passwords` lists the account's app passwords, `bsky app-passwords add NAME [--privileged]` makes one and prints its password once, and `bsky app-passwords revoke NAME` revokes one; a session made with an app password is told to log in with the account's password for these.

### Changed

- The first posts appear sooner. The timeline is asked for before the terminal is asked what it can draw, so the two round trips overlap: with a terminal that answers in 150 ms, as over ssh, and a server in 300 ms, the first post is on screen after 308 ms instead of 509 ms. An answer from the server is also shown as it comes rather than at the next 50 ms tick (55 ms to 8 ms with a local server).
- Every key in the `?` help is described on one line on an 80-column terminal; the long descriptions were shortened.

### Fixed

- `bsky logout` and `bsky logout --all` stopped after removing a `session.json` an earlier version wrote, and left the accounts logged in. Both now log out that file and the accounts they name.
- With `BSKY_ACCOUNT` (or `-a`) naming an account not logged in yet, `bsky login` and `bsky accounts` failed with "is not logged in", so that account could not be logged in from that shell.
- With `--json`, a usage error such as a missing argument printed no JSON, and `bsky logout` printed text. Both print JSON on stdout now; `bsky logout --json` prints `{"loggedOut": [...]}`.
- `bsky timeline -n N` printed fewer than N posts when a page was mostly reposts, which are left out. It reads the next pages until it has N, or there are no more.
- After `m` on a profile, moving to another tab or a thread before the conversation came still switched to the Chat tab and marked the conversation read. It opens only while the profile is still shown; otherwise it waits on the Chat tab.
- The question `x` asks on the Columns tab or in the account list survived the session expiring: after logging in again, the first `y` removed the column or logged the account out. It is called off, as `D`'s is.
- `bsky delete`, `like` and the other commands took an at:// URI with a trailing slash, a query or a fragment as a different record key, and the server refused it. What follows the record key is dropped.
- The picture cache removed the user's own files: with the cache set to a folder whose `images/` folder held other files, the oldest of them were deleted once it passed 256 MB, and any file starting with `tmp.` after an hour. Only the files bsky wrote there, by their names, are counted and removed now.
- Checking that a chosen download or cache folder can be written followed a link left at the name of its test file and emptied the file the link pointed to. The test file is always a new one.
- A command piped into a reader that stops early, as `bsky tl | head -1` does, exited 3 with "Broken pipe", and with `--json` it panicked. It stops writing and exits 0.
- `bsky unmute` on an account muted by one of your mute lists said "unmuted" and changed nothing, and `M` in the client did the same. It now says which list mutes them, and `bsky mute` and `M` add your own mute. A profile says "muted by the list" with its name.
- Esc pressed to close an error over the composer or the profile editor also closed the window and threw the draft away. It closes the error only; Esc again closes the window.
- Text pasted while a post was being sent, or while the profile editor loaded or saved, showed in the box and was lost when the answer came. A paste then is not taken.
- A paste of several lines into a field of one line (alt text, a display name, the search box) ran the words on either side of each line break together. Each run of line breaks is a space now.
- Saving the profile after changing only the avatar, or nothing, rewrote the display name and description as the editor shows them: a tab another client wrote became spaces. A field that was not changed is left as it was.
- After R, or after posting, the Timeline dropped the posts loaded below its first page and the selection went back to the top, so the next `l` or `b` acted on the first post. The posts loaded stay, and so does the selection.
- The `.` list offered "open the profile" on the Profile tab, where Enter opens nothing, and "follow" on your own post or profile, which `f` refuses.
- On a profile opened from the Columns tab, the key hints said Esc goes to your profile; it goes back to the columns, and the hint says so.
- After `M` or `B`, the notifications of that account left the list but still counted on the Notifications tab.
- When the session had expired, each request of the start that found it out brought up the login form again, emptying the password being typed. The form the first one brings up stays; and after logging in again, an answer to a request sent before no longer asks for a login.
- A post taken out of a list elsewhere (deleted on the Profile tab, its author muted, blocked or unfollowed) above the selected one moved the selection onto the next post, so `l`, `b` or `D` then acted on a post that was not selected. The selection stays on its post.
- A conversation that came from `m` went to the top of the Chat list and moved the selection onto the one above, so Enter opened, and marked read, a conversation not chosen. The selection stays.
- Closing a conversation while its message was being sent, and opening it again, let the next Enter send a second message before the first was answered. A conversation cannot be closed while its message is on its way.
- `m` on the profile of someone whose conversation was open already opened it anew, dropping the draft. It brings the open one back as it was.
- A question waiting for its `y` (`D`, `B`, `x`) outlived its prompt: on the Chat tab it could not be called off with Esc, and a `y` pressed long after the prompt had gone still deleted, blocked or removed. It is answered by the next key wherever the screen went, and ends with its prompt.
- The error of a profile left for another before it came was shown on the next profile, which stopped loading.
- A thread reloaded with `R` and opened again with `v` before the answer came could stay "loading" for good.
- After a search for another word failed while the next page of the first search was on its way, that list never loaded more.
- With the `+` list open, pinned feeds that came in moved the selection, and Enter added another column than the one selected.
- Back on the Notifications tab with Esc from a profile opened there, what came meanwhile was not marked seen.
- Messages that came into the conversation being read were not marked read, and came back as unread in the list and in other apps.
- On a like or a repost notification, the `.` list left out `v`, `Space`, `o` and `c`, which act on the post liked.
- A conversation opened with `m` before the Chat tab was ever shown left the list of conversations unloaded, so Esc showed that one alone.

## [0.4.0] - 2026-09-24

### Added

- A Chat tab (`6`) for direct messages: the conversations with their unread counts (also on the tab's title), a conversation's messages, the newest just above the box to write in and the older ones read on scrolling up (`i`, `enter` sends). A conversation is marked read when it is opened, not when the list is loaded, and the tab reads the server again every 15 seconds while it is shown. `m` on someone's profile opens the conversation with them, starting it if needed. An app password made without access to direct messages is told how to make one that has it. On the command line, `bsky chat` lists the conversations, `bsky chat <actor>` prints the messages with them, and `bsky chat <actor> <text>` sends one.

- A Columns tab (`5`) shows several lists side by side: the following timeline, a pinned feed, the notifications, your posts, or a search. `+` adds one, `x` removes the focused one after a `y`, `<` `>` move it, and `←` `→` (or `H` `L`) choose the column the keys act on; every key of a post works on the column's selected post as on its own tab, and a like, repost, follow or delete shows in every column and tab that holds the post. The columns are kept in `settings.json` per account, and loaded when the tab is shown.

- Commands for scripts: `bsky timeline` (`tl`), `feed`, `thread`, `notifications` (`notif`), `search`, `profile`, `followers`, `follows`, `post`, `like`, `unlike`, `repost`, `unrepost`, `follow`, `unfollow`, `delete`, `login` and `accounts`. They run as the account in use or the one `-a` names, take a post's at:// URI or bsky.app address, and print text for reading or, with `--json`, the server's own objects (a write prints what the server answered, and an error is printed as JSON on stdout too). A post from `bsky post` is the same record the composer sends. A network or server error exits with status 4.

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
