//! What the client writes, with bsky's keys: a note composed with `n` (a
//! reply with `r`, a quote with `Q`), a reaction with `l` (bsky's like) or
//! one chosen with `e`, a renote with `b`, your own note deleted with `D`,
//! and the notifications marked read when their tab is visited.

use std::path::PathBuf;

use super::*;
use crate::cli::Visibility;
use crate::tui::files::{Action, Browser, is_video_name};
use crate::tui::input::TextInput;

/// The reactions `e` offers: Misskey's own default palette.
pub const REACTIONS: &[&str] = &["👍", "❤", "😆", "🤔", "😮", "🎉", "💢", "😥", "😇", "🍮"];

/// How many recent reactions the picker keeps.
const RECENT: usize = 10;

/// The reaction `l` gives, as Misskey's like button does.
pub const LIKE: &str = "❤";

/// The most pictures a note carries from the composer, as `msky post`.
pub const MAX_PICTURES: usize = 4;
/// The most characters of a content warning and of an alt text, as
/// `msky post` checks them.
const MAX_CW_CHARS: usize = 100;
const MAX_ALT_CHARS: usize = 512;

/// A file attached in the composer, with its alt text.
#[derive(Debug, Clone, PartialEq)]
pub struct Attached {
    pub path: PathBuf,
    pub alt: TextInput,
}

impl Attached {
    pub fn is_video(&self) -> bool {
        is_video_name(&self.path.to_string_lossy())
    }
}

/// A note the composer replies to or quotes: its ID, who wrote it, and the
/// start of what it says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub id: String,
    pub who: String,
    pub excerpt: String,
}

/// The composer.
#[derive(Debug, Clone, PartialEq)]
pub struct Compose {
    pub input: TextInput,
    /// The content warning, when ctrl+w turned it on.
    pub cw: Option<TextInput>,
    pub visibility: Visibility,
    pub media: Vec<Attached>,
    /// What typing goes to: 0 the text, then the warning when it is on,
    /// then the alt text of each attachment.
    pub focus: usize,
    pub reply: Option<Target>,
    pub quote: Option<Target>,
    pub sending: bool,
    /// The file browser, open over the composer.
    pub browser: Option<Browser>,
}

impl Compose {
    pub fn new(reply: Option<Target>, quote: Option<Target>) -> Self {
        Self {
            input: TextInput::multi(""),
            cw: None,
            visibility: Visibility::Public,
            media: Vec::new(),
            focus: 0,
            reply,
            quote,
            sending: false,
            browser: None,
        }
    }

    /// How many fields take typing.
    fn fields(&self) -> usize {
        1 + usize::from(self.cw.is_some()) + self.media.len()
    }

    /// The field typing goes to.
    fn field(&mut self) -> &mut TextInput {
        let cw = usize::from(self.cw.is_some());
        match (self.focus, &mut self.cw) {
            (0, _) => &mut self.input,
            (1, Some(cw)) => cw,
            (i, _) => &mut self.media[i - 1 - cw].alt,
        }
    }

    /// The index of the attachment whose alt text has focus, if one has.
    pub fn focused_media(&self) -> Option<usize> {
        let first = 1 + usize::from(self.cw.is_some());
        self.focus.checked_sub(first)
    }

    /// Whether the warning field has focus.
    pub fn cw_focused(&self) -> bool {
        self.cw.is_some() && self.focus == 1
    }
}

/// The next visibility ctrl+p goes to.
fn next_visibility(v: Visibility) -> Visibility {
    match v {
        Visibility::Public => Visibility::Home,
        Visibility::Home => Visibility::Followers,
        Visibility::Followers => Visibility::Public,
    }
}

/// Why `c` cannot be sent, if it cannot. The length of the text is the
/// server's to say, from its `meta`, when the note is sent.
fn problem(c: &Compose) -> Option<String> {
    let text = c.input.text();
    if text.trim().is_empty() && c.media.is_empty() && c.quote.is_none() {
        return Some("the note is empty".into());
    }
    if let Some(cw) = &c.cw {
        let n = cw.text().chars().count();
        if cw.text().trim().is_empty() {
            return Some("the content warning is empty: type it, or ctrl+w takes it off".into());
        }
        if n > MAX_CW_CHARS {
            return Some(format!(
                "the content warning is {n} characters; the limit is {MAX_CW_CHARS}"
            ));
        }
    }
    if let Some(a) = c
        .media
        .iter()
        .find(|a| a.alt.text().chars().count() > MAX_ALT_CHARS)
    {
        return Some(format!(
            "an alt text is {} characters; the limit is {MAX_ALT_CHARS}",
            a.alt.text().chars().count()
        ));
    }
    media_problem(&c.media)
}

/// Why `media` cannot go on one note, if it cannot.
fn media_problem(media: &[Attached]) -> Option<String> {
    let videos = media.iter().filter(|a| a.is_video()).count();
    if videos > 0 && media.len() > 1 {
        Some("a note can have up to 4 pictures or one video, not both".into())
    } else if media.len() > MAX_PICTURES {
        Some(format!("a note can have at most {MAX_PICTURES} pictures"))
    } else {
        None
    }
}

/// Where the file browser opens: the folder it last showed, else the
/// current directory, else the home directory.
pub(super) fn browse_start(last: &Option<PathBuf>) -> PathBuf {
    last.clone()
        .or_else(|| std::env::current_dir().ok())
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

impl App {
    /// Whether the login in use may do what needs `permission`, saying
    /// why not when it may not. A token from `MSKY_TOKEN` carries no list:
    /// the server says what it lacks.
    pub(super) fn may(&mut self, permission: &str, what: &str) -> bool {
        let Some(s) = &self.session else {
            let host = self.host().to_string();
            self.error(format!(
                "to {what}, log in: press A then a (or run `msky login --host {host}`)"
            ));
            return false;
        };
        if s.permissions.is_empty() || s.permissions.iter().any(|p| p == permission) {
            return true;
        }
        let host = s.host().to_string();
        self.error(format!(
            "the login saved for {host} does not allow msky to {what}: it lacks {permission}; log in again (A then a, or `msky login --host {host}`)"
        ));
        false
    }

    /// Mark `key` as on its way; `false` when it is already.
    pub(super) fn claim(&mut self, key: String) -> bool {
        if self.in_flight.insert(key) {
            true
        } else {
            self.info("still waiting for the server");
            false
        }
    }

    /// The account in use's ID.
    fn my_id(&self) -> Option<&str> {
        self.session
            .as_ref()
            .map(|s| s.user_id.as_str())
            .filter(|id| !id.is_empty())
    }

    /// The target `r` or `Q` acts on: the note shown.
    fn target(&self) -> Option<Target> {
        let n = self.shown_note()?;
        let excerpt = crate::mfm::plain_text(n.text.as_deref().unwrap_or(""))
            .lines()
            .next()
            .unwrap_or("")
            .to_string();
        let who = match n.user.host.as_deref().filter(|h| !h.is_empty()) {
            Some(h) => format!("@{}@{h}", n.user.username),
            None => format!("@{}", n.user.username),
        };
        Some(Target {
            id: n.id.clone(),
            who,
            excerpt,
        })
    }

    /// `n`, `r`, `Q`: the composer, empty, replying, or quoting.
    pub(super) fn compose(&mut self, reply: bool, quote: bool) {
        if !self.may("write:notes", "post notes") {
            return;
        }
        let target = if reply || quote {
            match self.target() {
                Some(t) => Some(t),
                None => return,
            }
        } else {
            None
        };
        let (reply, quote) = if reply {
            (target, None)
        } else {
            (None, target)
        };
        let mut c = Compose::new(reply.clone(), quote);
        // A reply is seen by whom its note is, as Misskey's client sets it.
        if let Some(r) = &reply
            && let Some(n) = self.find_note(&r.id)
        {
            c.visibility = match n.visibility.as_deref() {
                Some("home") => Visibility::Home,
                Some("followers" | "specified") => Visibility::Followers,
                _ => Visibility::Public,
            };
        }
        self.overlay = Some(Overlay::Compose(Box::new(c)));
    }

    /// A key in the composer.
    pub(super) fn compose_key(&mut self, key: KeyEvent) -> Jobs {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let start = browse_start(&self.browse_from);
        let Some(Overlay::Compose(c)) = &mut self.overlay else {
            return Vec::new();
        };
        if c.sending {
            return Vec::new();
        }
        if let Some(b) = &mut c.browser {
            match b.key(key) {
                Action::None => {}
                Action::Close => {
                    self.browse_from = Some(b.dir.clone());
                    c.browser = None;
                }
                Action::Choose(paths) => {
                    self.browse_from = Some(b.dir.clone());
                    c.browser = None;
                    let mut all = c.media.clone();
                    all.extend(paths.into_iter().map(|path| Attached {
                        path,
                        alt: TextInput::single(""),
                    }));
                    match media_problem(&all) {
                        Some(why) => self.error(why),
                        None => c.media = all,
                    }
                }
            }
            return Vec::new();
        }
        let fields = c.fields();
        match key.code {
            KeyCode::Esc => self.overlay = None,
            KeyCode::Tab => c.focus = (c.focus + 1) % fields,
            KeyCode::BackTab => c.focus = (c.focus + fields - 1) % fields,
            KeyCode::Char('w') if ctrl => {
                if c.cw.take().is_none() {
                    c.cw = Some(TextInput::single(""));
                    c.focus = 1;
                } else {
                    c.focus = 0;
                }
            }
            KeyCode::Char('p') if ctrl => c.visibility = next_visibility(c.visibility),
            KeyCode::Char('o') if ctrl => {
                let room = MAX_PICTURES.saturating_sub(c.media.len());
                if c.media.iter().any(Attached::is_video) {
                    self.error("a note with a video can have nothing else attached");
                } else if room == 0 {
                    self.error(format!("a note can have at most {MAX_PICTURES} pictures"));
                } else {
                    c.browser = Some(Browser::open(&start, room, c.media.is_empty()));
                }
            }
            // The file whose alt text is being typed, or the last.
            KeyCode::Char('x') if ctrl && !c.media.is_empty() => {
                let i = c.focused_media().unwrap_or(c.media.len() - 1);
                c.media.remove(i);
                c.focus = c.focus.min(c.fields() - 1);
            }
            KeyCode::Char('s') if ctrl => {
                if let Some(why) = problem(c) {
                    self.error(why);
                    return Vec::new();
                }
                c.sending = true;
                let job = Job::Post {
                    text: c.input.text().trim().to_string(),
                    cw: c.cw.as_ref().map(TextInput::text),
                    visibility: c.visibility,
                    reply: c.reply.as_ref().map(|t| t.id.clone()),
                    renote: c.quote.as_ref().map(|t| t.id.clone()),
                    files: c
                        .media
                        .iter()
                        .map(|a| (a.path.clone(), a.alt.text()))
                        .collect(),
                };
                self.info("posting…");
                return vec![self.stamp(job)];
            }
            _ => {
                c.field().handle_key(key);
            }
        }
        Vec::new()
    }

    /// The answer to a post.
    pub(super) fn posted(&mut self, result: crate::error::Result<Box<Note>>) {
        match result {
            Ok(note) => {
                let note = *note;
                if matches!(self.overlay, Some(Overlay::Compose(_))) {
                    self.overlay = None;
                }
                self.info(format!("posted {}/notes/{}", self.origin, note.id));
                // Your own note is on your home timeline: it goes on top of
                // it, as the next load would put it.
                let home = &mut self.timelines[TimelineKind::Home.index()];
                if home.loaded && !home.items.iter().any(|n| n.id == note.id) {
                    if !home.items.is_empty() {
                        home.selected += 1;
                        home.offset += 1;
                    }
                    home.items.insert(0, note);
                }
            }
            Err(e) => {
                if let Some(Overlay::Compose(c)) = &mut self.overlay {
                    c.sending = false;
                }
                self.error(said(&e));
            }
        }
    }

    /// Text pasted: it goes to the field that has focus in the composer.
    pub fn handle_paste(&mut self, text: &str) {
        match &mut self.overlay {
            Some(Overlay::Compose(c)) if c.browser.is_none() && !c.sending => {
                c.field().insert_str(text)
            }
            Some(Overlay::Login(f)) => {
                if let Some(t) = &mut f.typed {
                    t.push_str(text.trim());
                }
            }
            _ => {}
        }
    }

    /// `l`: react with ❤, or take back the reaction given.
    pub(super) fn like(&mut self) -> Jobs {
        let Some(n) = self.shown_note() else {
            return Vec::new();
        };
        let (id, had) = (n.id.clone(), n.my_reaction.clone());
        let reaction = if had.is_some() {
            None
        } else {
            Some(LIKE.to_string())
        };
        self.react(id, reaction, had)
    }

    /// React with `r`, the picker's choice.
    pub(super) fn react_with(&mut self, r: String) -> Jobs {
        let Some(n) = self.shown_note() else {
            return Vec::new();
        };
        let (id, had) = (n.id.clone(), n.my_reaction.clone());
        // Your own reaction is written `:name@.:` on the note.
        let same = had
            .as_deref()
            .is_some_and(|h| h == r || h.replace("@.:", ":") == r);
        if same {
            self.info(format!("you reacted {r} already"));
            return Vec::new();
        }
        self.react(id, Some(r), had)
    }

    /// What the picker offers: the recent reactions and Misskey's default
    /// ones, or, with `query`, the server's custom emoji whose name has
    /// it, those that start with it first.
    pub fn reaction_choices(&self, query: &str) -> Vec<String> {
        let q = query.trim().trim_matches(':').to_ascii_lowercase();
        if q.is_empty() {
            let mut v: Vec<String> = self.recent_reactions.clone();
            for r in REACTIONS {
                if !v.iter().any(|x| x == r) {
                    v.push((*r).to_string());
                }
            }
            return v;
        }
        let mut names: Vec<&String> = self
            .emojis
            .keys()
            .filter(|n| n.to_ascii_lowercase().contains(&q))
            .collect();
        names.sort_by_key(|n| {
            (
                !n.to_ascii_lowercase().starts_with(&q),
                n.len(),
                (*n).clone(),
            )
        });
        names
            .into_iter()
            .take(40)
            .map(|n| format!(":{n}:"))
            .collect()
    }

    /// A key in the reaction picker: arrows move, letters search the
    /// server's custom emoji, a digit with nothing typed picks from
    /// Misskey's default reactions, enter reacts.
    pub(super) fn reactions_key(&mut self, key: KeyEvent) -> Jobs {
        let Some(Overlay::Reactions { selected, query }) = &self.overlay else {
            return Vec::new();
        };
        let (mut selected, mut query) = (*selected, query.clone());
        let choices = self.reaction_choices(&query);
        let n = choices.len().max(1);
        match key.code {
            KeyCode::Esc => {
                self.overlay = None;
                return Vec::new();
            }
            KeyCode::Down | KeyCode::Right | KeyCode::Tab => selected = (selected + 1) % n,
            KeyCode::Up | KeyCode::Left | KeyCode::BackTab => selected = (selected + n - 1) % n,
            KeyCode::Enter => {
                self.overlay = None;
                return match choices.get(selected) {
                    Some(r) => self.react_with(r.clone()),
                    None => Vec::new(),
                };
            }
            KeyCode::Backspace => {
                query.pop();
                selected = 0;
            }
            // A digit picks the default reaction at that place, 0 the tenth.
            KeyCode::Char(c @ '0'..='9') if query.is_empty() => {
                let i = (c as usize - '0' as usize + 9) % 10;
                self.overlay = None;
                return match REACTIONS.get(i) {
                    Some(r) => self.react_with((*r).to_string()),
                    None => Vec::new(),
                };
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                query.push(c);
                selected = 0;
            }
            _ => {}
        }
        self.overlay = Some(Overlay::Reactions { selected, query });
        Vec::new()
    }

    fn react(&mut self, id: String, reaction: Option<String>, had: Option<String>) -> Jobs {
        if !self.may("write:reactions", "react to notes") || !self.claim(format!("react:{id}")) {
            return Vec::new();
        }
        vec![self.stamp(Job::React { id, reaction, had })]
    }

    /// `b`: renote the note shown, or take back your renotes of it.
    pub(super) fn toggle_renote(&mut self) -> Jobs {
        let Some(selected) = self.selected_note() else {
            return Vec::new();
        };
        // Your own renote selected takes it back.
        let mine = selected.is_pure_renote() && Some(selected.author_id()) == self.my_id();
        let id = selected.shown().id.clone();
        let on = !(mine || self.renoted.contains(&id));
        if !self.may("write:notes", "renote") || !self.claim(format!("renote:{id}")) {
            return Vec::new();
        }
        self.info(if on {
            "renoting…"
        } else {
            "taking back the renote…"
        });
        vec![self.stamp(Job::Renote { id, on })]
    }

    /// `D`: ask before deleting your own note.
    pub(super) fn ask_delete(&mut self) {
        let Some(n) = self.selected_note() else {
            return;
        };
        if Some(n.author_id()) != self.my_id() || n.is_pure_renote() {
            self.info("only your own note can be deleted (b takes back a renote)");
            return;
        }
        let id = n.id.clone();
        self.info("press y to delete the note, any other key to keep it");
        self.confirm_delete = Some(id);
    }

    /// The key after `D`'s question: `y` deletes.
    pub(super) fn answer_delete(&mut self, id: String, key: KeyEvent) -> Jobs {
        if key.code != KeyCode::Char('y') {
            self.info("not deleted");
            return Vec::new();
        }
        if !self.may("write:notes", "delete notes") || !self.claim(format!("delete:{id}")) {
            return Vec::new();
        }
        self.info("deleting…");
        vec![self.stamp(Job::Delete { id })]
    }

    /// Mark the notifications read, once a list of them is loaded and on
    /// screen, as bsky marks them seen when their tab is visited.
    pub(super) fn mark_read(&mut self) -> Jobs {
        if self.tab != Tab::Notifications || !self.notifications.loaded || self.read_sent {
            return Vec::new();
        }
        // The newest ones, as many as the server counts unread, are the
        // ones not seen yet: they keep a dot, as bsky's, while shown.
        let unread = usize::try_from(self.unread.unwrap_or(0)).unwrap_or(0);
        self.fresh = (self.notifications.items.iter())
            .take(unread)
            .map(|n| n.id.clone())
            .collect();
        let allowed = self.session.as_ref().is_some_and(|s| {
            s.permissions.is_empty() || s.permissions.iter().any(|p| p == "write:notifications")
        });
        if !allowed || self.notifications.items.is_empty() {
            return Vec::new();
        }
        self.read_sent = true;
        vec![self.stamp(Job::MarkRead)]
    }

    /// The note `id` wherever a list has it, the one renoted or quoted too.
    fn find_note(&self, id: &str) -> Option<&Note> {
        let notes = self
            .timelines
            .iter()
            .chain(self.feeds.iter().map(|f| &f.list))
            .flat_map(|l| l.items.iter())
            .chain(
                self.notifications
                    .items
                    .iter()
                    .filter_map(|n| n.note.as_ref()),
            )
            .chain(
                self.threads
                    .iter()
                    .flat_map(|t| t.list.items.iter().map(|r| &r.note)),
            );
        for n in notes {
            if n.id == id {
                return Some(n);
            }
            if let Some(r) = n.renote.as_deref().filter(|r| r.id == id) {
                return Some(r);
            }
        }
        None
    }

    /// Change the note `id` wherever a list has it.
    fn update_notes(&mut self, id: &str, f: impl Fn(&mut Note)) {
        let apply = |n: &mut Note| {
            if n.id == id {
                f(n);
            }
            if let Some(r) = n.renote.as_deref_mut()
                && r.id == id
            {
                f(r);
            }
            if let Some(r) = n.reply.as_deref_mut()
                && r.id == id
            {
                f(r);
            }
        };
        for l in self
            .timelines
            .iter_mut()
            .chain(self.feeds.iter_mut().map(|f| &mut f.list))
        {
            l.items.iter_mut().for_each(apply);
        }
        self.notifications
            .items
            .iter_mut()
            .filter_map(|n| n.note.as_mut())
            .for_each(apply);
        for t in &mut self.threads {
            t.list.items.iter_mut().for_each(|r| apply(&mut r.note));
        }
    }

    /// A reaction the server took: the counts and your reaction follow it.
    pub(super) fn reacted(
        &mut self,
        id: &str,
        reaction: Option<String>,
        had: Option<String>,
        result: crate::error::Result<()>,
    ) {
        self.in_flight.remove(&format!("react:{id}"));
        if let Err(e) = result {
            self.error(said(&e));
            return;
        }
        // A note counts the server's own custom emoji as `:name@.:`.
        let on_note = |r: &str| -> String {
            match r.strip_prefix(':').and_then(|x| x.strip_suffix(':')) {
                Some(name) if !name.contains('@') => format!(":{name}@.:"),
                _ => r.to_string(),
            }
        };
        let reaction_key = reaction.as_deref().map(on_note);
        self.update_notes(id, |n| {
            if let Some(h) = &had
                && let Some(c) = n.reactions.get_mut(h)
            {
                *c = c.saturating_sub(1);
                if *c == 0 {
                    n.reactions.remove(h);
                }
            }
            if let Some(r) = &reaction_key {
                *n.reactions.entry(r.clone()).or_insert(0) += 1;
            }
            n.my_reaction = reaction_key.clone();
        });
        match reaction {
            Some(r) => {
                self.recent_reactions.retain(|x| *x != r);
                self.recent_reactions.insert(0, r.clone());
                self.recent_reactions.truncate(RECENT);
                self.keep_recent_reactions();
                self.info(format!("reacted {r}"));
            }
            None => self.info("took back the reaction"),
        }
    }

    /// A renote made or taken back.
    pub(super) fn renoted(&mut self, id: &str, on: bool, result: crate::error::Result<()>) {
        self.in_flight.remove(&format!("renote:{id}"));
        if let Err(e) = result {
            self.error(said(&e));
            return;
        }
        let me = self.my_id().map(str::to_string);
        if on {
            self.renoted.insert(id.to_string());
            self.update_notes(id, |n| n.renote_count += 1);
            self.info("renoted");
        } else {
            self.renoted.remove(id);
            self.update_notes(id, |n| n.renote_count = n.renote_count.saturating_sub(1));
            // Your own renotes of it go from the lists.
            let mine = |n: &Note| {
                n.is_pure_renote()
                    && n.renote_id.as_deref() == Some(id)
                    && Some(n.author_id()) == me.as_deref()
            };
            for l in self
                .timelines
                .iter_mut()
                .chain(self.feeds.iter_mut().map(|f| &mut f.list))
            {
                l.retain(|n| !mine(n));
            }
            self.info("took back the renote");
        }
    }

    /// A note deleted: it goes from every list.
    pub(super) fn deleted(&mut self, id: &str, result: crate::error::Result<()>) {
        self.in_flight.remove(&format!("delete:{id}"));
        if let Err(e) = result {
            self.error(said(&e));
            return;
        }
        for l in self
            .timelines
            .iter_mut()
            .chain(self.feeds.iter_mut().map(|f| &mut f.list))
        {
            l.retain(|n| n.id != id);
        }
        for t in &mut self.threads {
            t.list.retain(|r| r.note.id != id);
        }
        self.info("deleted");
    }
}
