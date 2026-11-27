//! The commands: what a Misskey server shows to visitors, from scripts and
//! pipes. Every command prints text for reading, or with `--json` one
//! object per note per line, in the same shape for every command (see
//! [`format::note_json`]).

mod format;

use std::collections::HashSet;
use std::io::{self, Write};
use std::path::PathBuf;

use clap::Subcommand;
use serde_json::{Value, json};

use crate::api::types::Note;
use crate::api::{self, Client, NoteFilter};
use crate::download;
use crate::error::{Error, Result};

/// The commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// An account's notes, newest first.
    ///
    /// USER is name, @name, @name@host, or the account's address
    /// (misskey.io/@name). Its replies and renotes are left out
    /// unless asked for.
    Author {
        user: String,
        /// How many notes.
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
        /// Include its replies.
        #[arg(long)]
        replies: bool,
        /// Include its renotes.
        #[arg(long)]
        renotes: bool,
        /// Only the notes with files attached.
        #[arg(long, conflicts_with = "replies")]
        with_files: bool,
        /// Save the files of the notes listed (in the folder -o names).
        #[arg(long)]
        download: bool,
        /// Folder the files are saved in; giving it saves them. Default:
        /// MSKY_DOWNLOAD_DIR, else msky in your Downloads folder.
        #[arg(short = 'o', long, value_name = "DIR")]
        output: Option<PathBuf>,
    },
    /// One note: its ID or its address (misskey.io/notes/ID).
    Note { note: String },
    /// Notes that match QUERY, newest first.
    Search {
        query: String,
        /// How many notes.
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
    /// Save the files attached to a note: its ID or its address.
    ///
    /// Each is named after the note, its place among the note's files, and
    /// its type (9abc-1.png), and none is written over.
    Download {
        note: String,
        /// Folder the files are saved in. Default: MSKY_DOWNLOAD_DIR, else
        /// msky in your Downloads folder.
        #[arg(short = 'o', long, value_name = "DIR")]
        output: Option<PathBuf>,
    },
}

/// What a command runs with.
pub struct Ctx {
    /// The server `--host` names.
    pub origin: String,
    pub json: bool,
}

impl Ctx {
    /// A client of `origin`, or of the server `--host` names.
    fn client(&self, origin: Option<&str>) -> Client {
        Client::new(origin.unwrap_or(&self.origin))
    }
}

/// Run `cmd`, writing to stdout.
pub fn run(cmd: Command, ctx: &Ctx) -> Result<()> {
    let mut out = io::stdout().lock();
    ended_by_reader(run_to(cmd, ctx, &mut out))
}

/// A command's result, where a reader that stopped reading (`| head`) is
/// not an error: what it wanted it has, so the command ends there, as the
/// tools it is piped into expect.
pub fn ended_by_reader(result: Result<()>) -> Result<()> {
    match result {
        Err(e) if e.message() == PIPE_CLOSED => Ok(()),
        r => r,
    }
}

/// What `write_err` says when the reader has closed the pipe.
const PIPE_CLOSED: &str = "the output was closed";

fn run_to(cmd: Command, ctx: &Ctx, out: &mut dyn Write) -> Result<()> {
    match cmd {
        Command::Author {
            user,
            limit,
            replies,
            renotes,
            with_files,
            download,
            output,
        } => {
            let filter = NoteFilter {
                replies,
                renotes,
                files: with_files,
            };
            let save_in = if download || output.is_some() {
                Some(download::download_dir(output)?)
            } else {
                None
            };
            author(ctx, out, &user, limit, filter, save_in)
        }
        Command::Note { note } => show_note(ctx, out, &note),
        Command::Search { query, limit } => search(ctx, out, &query, limit),
        Command::Download { note, output } => {
            download_note(ctx, out, &note, &download::download_dir(output)?)
        }
    }
}

/// Print `v` as one line of JSON.
fn json_line(out: &mut dyn Write, v: &Value) -> Result<()> {
    writeln!(out, "{v}").map_err(write_err)
}

fn write_err(e: io::Error) -> Error {
    if e.kind() == io::ErrorKind::BrokenPipe {
        return Error::io(PIPE_CLOSED);
    }
    Error::io(format!("cannot write the output: {e}"))
}

fn text(out: &mut dyn Write, s: &str) -> Result<()> {
    out.write_all(s.as_bytes()).map_err(write_err)
}

/// Pages from `fetch` (how many to ask for, and the ID to list older notes
/// than) until `limit` notes have come or there are no more. A page that
/// brings nothing new ends it too: a server that ignores `untilId` would
/// otherwise be asked for the same page for good. Misskey may answer a page
/// with fewer notes than asked for while there are more, so only an empty
/// page says the end was reached.
fn pages(
    limit: usize,
    mut fetch: impl FnMut(usize, Option<&str>) -> Result<Vec<Note>>,
) -> Result<Vec<Note>> {
    let mut notes = Vec::new();
    let mut seen = HashSet::new();
    let mut until: Option<String> = None;
    while notes.len() < limit {
        let page = fetch((limit - notes.len()).min(api::MAX_PAGE), until.as_deref())?;
        let Some(last) = page.last().map(|n| n.id.clone()) else {
            break;
        };
        let before = notes.len();
        notes.extend(page.into_iter().filter(|n| seen.insert(n.id.clone())));
        if notes.len() == before || until.as_deref() == Some(last.as_str()) {
            break;
        }
        until = Some(last);
    }
    notes.truncate(limit);
    Ok(notes)
}

/// An account as typed: on which server to ask, its name, and the server it
/// is on when that was written.
#[derive(Debug, PartialEq, Eq)]
struct UserRef {
    /// The server of an address; `None` asks the one `--host` names.
    origin: Option<String>,
    username: String,
    host: Option<String>,
}

/// `name`, `@name`, `@name@host`, `name@host`, or an account's address,
/// `https://<server>/@name` or `https://<server>/@name@host`.
fn parse_user(typed: &str) -> Result<UserRef> {
    let t = typed.trim();
    let not_an_account = || {
        Error::usage(format!("{t:?} is not an account")).with_hint(
            "give name, @name, @name@host, or its address such as https://misskey.io/@name",
        )
    };
    let (origin, acct) = match web_address(t) {
        Some((origin, path)) => {
            let acct = path.strip_prefix('@').ok_or_else(not_an_account)?;
            if acct.contains('/') {
                return Err(not_an_account());
            }
            (Some(origin), acct.to_string())
        }
        None => (None, t.strip_prefix('@').unwrap_or(t).to_string()),
    };
    let (username, host) = match acct.split_once('@') {
        Some((name, host)) => (name, Some(host)),
        None => (acct.as_str(), None),
    };
    let name_ok = !username.is_empty()
        && username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if !name_ok {
        return Err(not_an_account());
    }
    // The host after the name is a host alone: a scheme or a path there
    // is a typo, not a server to ask.
    let host = match host {
        Some(h) => match api::normalize_host(h) {
            Ok(origin) if !h.contains("://") && !h.starts_with('@') => {
                Some(api::authority(&origin).to_string())
            }
            _ => return Err(not_an_account()),
        },
        None => None,
    };
    Ok(UserRef {
        origin,
        username: username.to_string(),
        host,
    })
}

/// The origin and the path of an http(s) address, the path without its
/// query, fragment or trailing slash. One written without its scheme
/// (`misskey.io/notes/ID`) is taken as https.
fn web_address(t: &str) -> Option<(String, String)> {
    let lower = t.to_ascii_lowercase();
    let (scheme, rest) = if lower.starts_with("https://") || lower.starts_with("http://") {
        t.split_once("://")?
    } else if !t.contains("://") && t.find('/').is_some_and(|i| i > 0) {
        ("https", t)
    } else {
        return None;
    };
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let origin = api::normalize_host(&format!("{scheme}://{authority}")).ok()?;
    let path = path.trim_start_matches('/').trim_end_matches('/');
    Some((origin, path.to_string()))
}

/// A note as typed: on which server to ask, and its ID.
fn parse_note(typed: &str) -> Result<(Option<String>, String)> {
    let t = typed.trim();
    let not_a_note = || {
        Error::usage(format!("{t:?} is not a note"))
            .with_hint("give its ID, or its address such as https://misskey.io/notes/ID")
    };
    let (origin, id) = match web_address(t) {
        Some((origin, path)) => {
            let id = path.strip_prefix("notes/").ok_or_else(not_a_note)?;
            (Some(origin), id.to_string())
        }
        None => (None, t.to_string()),
    };
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(not_a_note());
    }
    Ok((origin, id))
}

fn author(
    ctx: &Ctx,
    out: &mut dyn Write,
    user: &str,
    limit: usize,
    filter: NoteFilter,
    save_in: Option<PathBuf>,
) -> Result<()> {
    let who = parse_user(user)?;
    let client = ctx.client(who.origin.as_deref());
    // An account of the server asked is named without a host.
    let host = who
        .host
        .as_deref()
        .filter(|h| !h.eq_ignore_ascii_case(client.host()));
    let notes = if limit == 0 {
        Vec::new()
    } else {
        let account = client.user(&who.username, host)?;
        pages(limit, |n, until| {
            client.user_notes(&account.id, &filter, n, until)
        })?
    };
    let agent = api::agent();
    for (i, n) in notes.iter().enumerate() {
        let saved = match &save_in {
            Some(dir) => save_files(&agent, n.shown(), dir)?,
            None => Vec::new(),
        };
        print_note(
            ctx,
            out,
            client.origin(),
            n,
            i,
            save_in.as_ref().map(|_| &saved[..]),
        )?;
    }
    Ok(())
}

/// Print a note, and where its files were saved when they were.
fn print_note(
    ctx: &Ctx,
    out: &mut dyn Write,
    origin: &str,
    n: &Note,
    i: usize,
    saved: Option<&[PathBuf]>,
) -> Result<()> {
    if ctx.json {
        let mut v = format::note_json(n, origin);
        if let Some(saved) = saved {
            v["saved"] = json!(
                saved
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
            );
        }
        return json_line(out, &v);
    }
    if i > 0 {
        text(out, "\n")?;
    }
    text(out, &format::note(n, origin))?;
    for p in saved.unwrap_or_default() {
        text(out, &format!("saved {}\n", p.display()))?;
    }
    Ok(())
}

/// Save every file of `n` in `dir`.
fn save_files(agent: &ureq::Agent, n: &Note, dir: &std::path::Path) -> Result<Vec<PathBuf>> {
    n.files
        .iter()
        .enumerate()
        .map(|(i, f)| download::save(agent, f, dir, &download::file_name(&n.id, i + 1, f)))
        .collect()
}

fn show_note(ctx: &Ctx, out: &mut dyn Write, note: &str) -> Result<()> {
    let (origin, id) = parse_note(note)?;
    let client = ctx.client(origin.as_deref());
    let n = client.note(&id)?;
    print_note(ctx, out, client.origin(), &n, 0, None)
}

fn search(ctx: &Ctx, out: &mut dyn Write, query: &str, limit: usize) -> Result<()> {
    if query.trim().is_empty() {
        return Err(Error::usage("the search query is empty"));
    }
    let client = ctx.client(None);
    let notes = pages(limit, |n, until| client.search(query, n, until))?;
    for (i, n) in notes.iter().enumerate() {
        print_note(ctx, out, client.origin(), n, i, None)?;
    }
    Ok(())
}

fn download_note(ctx: &Ctx, out: &mut dyn Write, note: &str, dir: &std::path::Path) -> Result<()> {
    let (origin, id) = parse_note(note)?;
    let client = ctx.client(origin.as_deref());
    let n = client.note(&id)?;
    let shown = n.shown();
    if shown.files.is_empty() {
        return Err(Error::usage(format!("{} has no files", shown.id)));
    }
    let agent = api::agent();
    for (i, f) in shown.files.iter().enumerate() {
        let path = download::save(&agent, f, dir, &download::file_name(&shown.id, i + 1, f))?;
        if ctx.json {
            json_line(
                out,
                &json!({
                    "noteId": shown.id,
                    "url": f.url,
                    "type": f.mime,
                    "size": f.size,
                    "path": path.display().to_string(),
                }),
            )?;
        } else {
            text(out, &format!("{}\n", path.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn user(origin: Option<&str>, name: &str, host: Option<&str>) -> UserRef {
        UserRef {
            origin: origin.map(str::to_string),
            username: name.to_string(),
            host: host.map(str::to_string),
        }
    }

    #[rstest]
    #[case("alice", user(None, "alice", None))]
    #[case(" @alice ", user(None, "alice", None))]
    #[case("@alice@Misskey.Design", user(None, "alice", Some("misskey.design")))]
    #[case("alice@mstdn.jp", user(None, "alice", Some("mstdn.jp")))]
    #[case("@a_b-c.d", user(None, "a_b-c.d", None))]
    #[case(
        "https://misskey.io/@alice",
        user(Some("https://misskey.io"), "alice", None)
    )]
    #[case(
        "https://misskey.io/@alice@mstdn.jp/?tab=notes#x",
        user(Some("https://misskey.io"), "alice", Some("mstdn.jp"))
    )]
    #[case(
        "http://127.0.0.1:8080/@bob",
        user(Some("http://127.0.0.1:8080"), "bob", None)
    )]
    #[case("misskey.io/@alice", user(Some("https://misskey.io"), "alice", None))]
    fn an_account_is_read_however_it_is_written(#[case] typed: &str, #[case] want: UserRef) {
        assert_eq!(parse_user(typed).unwrap(), want);
    }

    #[rstest]
    #[case("")]
    #[case("@")]
    #[case("@@host")]
    #[case("@alice@")]
    #[case("@alice@host/x")]
    #[case("あいう")]
    #[case("alice 🐈")]
    #[case("https://misskey.io/notes/9abc")]
    #[case("https://misskey.io/@alice/following")]
    #[case("https://misskey.io/")]
    fn what_is_not_an_account_is_a_usage_error(#[case] typed: &str) {
        let e = parse_user(typed).unwrap_err();
        assert_eq!(e.kind(), crate::error::Kind::Usage, "{}", e.message());
    }

    #[rstest]
    #[case("9abcdef", None, "9abcdef")]
    #[case(" 9abc ", None, "9abc")]
    #[case("https://misskey.io/notes/9abc", Some("https://misskey.io"), "9abc")]
    #[case(
        "https://Misskey.IO/notes/9abc/?x=1#r",
        Some("https://misskey.io"),
        "9abc"
    )]
    #[case("http://127.0.0.1:9/notes/a1", Some("http://127.0.0.1:9"), "a1")]
    #[case("misskey.io/notes/9abc", Some("https://misskey.io"), "9abc")]
    fn a_note_is_read_however_it_is_written(
        #[case] typed: &str,
        #[case] origin: Option<&str>,
        #[case] id: &str,
    ) {
        assert_eq!(
            parse_note(typed).unwrap(),
            (origin.map(str::to_string), id.to_string())
        );
    }

    #[rstest]
    #[case("")]
    #[case("../x")]
    #[case("9abc?x")]
    #[case("ノート")]
    #[case("https://misskey.io/@alice")]
    #[case("https://misskey.io/notes/")]
    #[case("https://misskey.io/notes/a/b")]
    #[case("ftp://misskey.io/notes/a")]
    #[case("/notes/a")]
    fn what_is_not_a_note_is_a_usage_error(#[case] typed: &str) {
        let e = parse_note(typed).unwrap_err();
        assert_eq!(e.kind(), crate::error::Kind::Usage, "{}", e.message());
    }

    fn page(ids: &[&str]) -> Vec<Note> {
        ids.iter()
            .map(|id| Note {
                id: id.to_string(),
                ..Default::default()
            })
            .collect()
    }

    // Pages are asked for until -n notes came, each older than the last
    // note of the one before, and no more than the API's 100 at a time.
    #[test]
    fn pages_go_back_until_the_limit() {
        let mut asked = Vec::new();
        let got = pages(250, |n, until| {
            asked.push((n, until.map(str::to_string)));
            let start = asked.len() * 1000;
            Ok((0..n)
                .map(|i| Note {
                    id: format!("{}", start - i),
                    ..Default::default()
                })
                .collect())
        })
        .unwrap();
        assert_eq!(got.len(), 250);
        assert_eq!(
            asked,
            [
                (100, None),
                (100, Some("901".into())),
                (50, Some("1901".into()))
            ]
        );
    }

    // A short page is not the end (Misskey leaves out what the visitor may
    // not see); an empty page is, and so is one that brings nothing new.
    #[test]
    fn pages_end_on_an_empty_page_or_one_with_nothing_new() {
        let mut calls = 0;
        let got = pages(10, |_, until| {
            calls += 1;
            Ok(match until {
                None => page(&["c", "b"]),
                Some("b") => page(&["a"]),
                _ => Vec::new(),
            })
        })
        .unwrap();
        assert_eq!(
            got.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            ["c", "b", "a"]
        );
        assert_eq!(calls, 3);
        let mut calls = 0;
        let got = pages(10, |_, _| {
            calls += 1;
            Ok(page(&["b", "a"]))
        })
        .unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(calls, 2, "the same page twice ends it");
    }

    #[test]
    fn an_error_on_a_later_page_is_returned() {
        let e = pages(10, |_, until| match until {
            None => Ok(page(&["b"])),
            Some(_) => Err(Error::api("boom")),
        })
        .unwrap_err();
        assert_eq!(e.message(), "boom");
    }

    /// A stdout that refuses every write with `kind`.
    struct Refusing(io::ErrorKind);

    impl Write for Refusing {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(self.0, "refused"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    // A reader that stopped reading (`| head`) ends the command quietly; an
    // output that cannot be written (a full disk) is an error.
    #[test]
    fn a_closed_pipe_ends_the_command_and_a_full_disk_fails_it() {
        let gone = &mut Refusing(io::ErrorKind::BrokenPipe);
        assert_eq!(ended_by_reader(text(gone, "x")), Ok(()));
        let full = &mut Refusing(io::ErrorKind::StorageFull);
        let e = ended_by_reader(text(full, "x")).unwrap_err();
        assert_eq!(e.kind(), crate::error::Kind::Io);
        assert_eq!(e.message(), "cannot write the output: refused");
    }

    #[test]
    fn an_empty_query_is_refused_before_anything_is_asked() {
        let ctx = Ctx {
            origin: "http://127.0.0.1:9".into(),
            json: false,
        };
        let e = search(&ctx, &mut Vec::new(), "  ", 5).unwrap_err();
        assert_eq!(e.kind(), crate::error::Kind::Usage);
    }
}
