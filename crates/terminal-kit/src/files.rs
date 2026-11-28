//! A directory browser for choosing pictures or a video, with keyboard
//! navigation, multi-selection and a picture preview shared by the clients.

use std::fs;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent};

mod view;

/// The pictures a note can carry, by extension.
const PICTURES: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "avif"];
/// The videos, by extension.
const VIDEOS: &[&str] = &["mp4", "webm", "mov", "m4v"];

fn has_extension(name: &str, list: &[&str]) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| list.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// Whether `name` is a picture's.
pub fn is_picture_name(name: &str) -> bool {
    has_extension(name, PICTURES)
}

/// Whether `name` is a video's.
pub fn is_video_name(name: &str) -> bool {
    has_extension(name, VIDEOS)
}

/// What an entry of the listing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum EntryKind {
    /// `..`, the folder above.
    Parent,
    Dir,
    /// A picture or a video.
    Media,
}

/// One line of the listing.
#[derive(Debug, Clone, PartialEq)]
struct Entry {
    name: String,
    path: PathBuf,
    kind: EntryKind,
    /// File size, for pictures and videos.
    bytes: u64,
}

/// `path` with a leading `~` as the home folder, as a shell reads it.
pub fn expand_home(path: &str) -> PathBuf {
    match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\') => {
            match dirs::home_dir() {
                Some(home) => home.join(rest.trim_start_matches(['/', '\\'])),
                None => PathBuf::from(path),
            }
        }
        _ => PathBuf::from(path),
    }
}

/// What the browser asks of whoever opened it.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Keep browsing.
    None,
    /// Close without choosing.
    Close,
    /// These files were chosen, in order.
    Choose(Vec<PathBuf>),
}

/// The browser's state.
#[derive(Debug, Clone, PartialEq)]
pub struct Browser {
    /// The folder currently shown.
    pub dir: PathBuf,
    items: Vec<Entry>,
    selected: usize,
    offset: usize,
    /// Whether names starting with a dot are listed.
    show_hidden: bool,
    /// Why the folder could not be read.
    error: Option<String>,
    /// Files marked with space, which Enter chooses together.
    marked: Vec<PathBuf>,
    /// How many files may still be chosen.
    room: usize,
    /// Whether videos are listed too (not next to pictures attached).
    videos: bool,
    /// A message about the last key, such as "no room for more".
    note: Option<String>,
}

/// `dir` without `..` in it, each taking off the name before it, as going
/// up does.
fn without_dot_dot(dir: PathBuf) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in dir.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

impl Browser {
    /// Open at `dir`, where at most `room` files, a video among them when
    /// `videos`, may be chosen.
    pub fn open(dir: &Path, room: usize, videos: bool) -> Self {
        let mut b = Self {
            dir: without_dot_dot(std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf())),
            items: Vec::new(),
            selected: 0,
            offset: 0,
            show_hidden: false,
            error: None,
            marked: Vec::new(),
            room: room.max(1),
            videos,
            note: None,
        };
        b.read(None);
        b
    }

    /// The selected entry.
    fn current(&self) -> Option<&Entry> {
        self.items.get(self.selected)
    }

    /// List the folder again, selecting `select` (a name) when it is there.
    fn read(&mut self, select: Option<&str>) {
        self.error = None;
        let mut items = Vec::new();
        if let Some(parent) = self.dir.parent() {
            items.push(Entry {
                name: "..".into(),
                path: parent.to_path_buf(),
                kind: EntryKind::Parent,
                bytes: 0,
            });
        }
        match fs::read_dir(&self.dir) {
            Ok(dir) => {
                for entry in dir.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if !self.show_hidden && name.starts_with('.') {
                        continue;
                    }
                    let path = entry.path();
                    // Through links: a link to a folder is a folder.
                    let Ok(meta) = fs::metadata(&path) else {
                        continue;
                    };
                    let kind = if meta.is_dir() {
                        EntryKind::Dir
                    } else if meta.is_file()
                        && (is_picture_name(&name) || (self.videos && is_video_name(&name)))
                    {
                        EntryKind::Media
                    } else {
                        continue;
                    };
                    items.push(Entry {
                        name,
                        path,
                        kind,
                        bytes: meta.len(),
                    });
                }
            }
            Err(e) => self.error = Some(format!("cannot read {}: {e}", self.dir.display())),
        }
        items.sort_by(|a, b| {
            (a.kind, a.name.to_lowercase(), &a.name).cmp(&(b.kind, b.name.to_lowercase(), &b.name))
        });
        let selected = select
            .and_then(|n| items.iter().position(|e| e.name == n))
            // The first thing in the folder rather than `..`, so Enter does
            // not leave a folder by accident.
            .unwrap_or(usize::from(
                items.len() > 1 && items[0].kind == EntryKind::Parent,
            ));
        self.items = items;
        self.selected = selected;
        self.offset = 0;
    }

    fn enter(&mut self, dir: PathBuf, select: Option<String>) {
        self.dir = dir;
        self.read(select.as_deref());
    }

    fn up(&mut self) {
        if let Some(parent) = self.dir.parent().map(Path::to_path_buf) {
            let from = self
                .dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned());
            self.enter(parent, from);
        }
    }

    fn toggle_mark(&mut self) {
        let Some(e) = self
            .items
            .get(self.selected)
            .filter(|e| e.kind == EntryKind::Media)
        else {
            return;
        };
        let path = e.path.clone();
        if let Some(i) = self.marked.iter().position(|p| *p == path) {
            self.marked.remove(i);
        } else if self.marked.len() < self.room {
            self.marked.push(path);
        } else {
            self.note = Some(format!(
                "{} marked; there is no room for more",
                self.marked.len()
            ));
            return;
        }
        self.step(1);
    }

    /// What Enter on the file at `current` chooses: the marked files and
    /// this one too while there is room. With room for one, the file under
    /// the cursor is the choice.
    fn chosen(&self, current: PathBuf) -> Vec<PathBuf> {
        if self.room == 1 {
            return vec![current];
        }
        let mut chosen = self.marked.clone();
        if !chosen.contains(&current) && chosen.len() < self.room {
            chosen.push(current);
        }
        chosen
    }

    fn step(&mut self, by: isize) {
        self.selected = self
            .selected
            .saturating_add_signed(by)
            .min(self.items.len().saturating_sub(1));
    }

    /// Handle a key.
    pub fn key(&mut self, key: KeyEvent) -> Action {
        self.note = None;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Action::Close,
            KeyCode::Char('j') | KeyCode::Down => self.step(1),
            KeyCode::Char('k') | KeyCode::Up => self.step(-1),
            KeyCode::PageDown => self.step(10),
            KeyCode::PageUp => self.step(-10),
            KeyCode::Char('g') | KeyCode::Home => self.step(isize::MIN / 2),
            KeyCode::Char('G') | KeyCode::End => self.step(isize::MAX / 2),
            KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => self.up(),
            KeyCode::Char('.') => {
                self.show_hidden = !self.show_hidden;
                let name = self.current().map(|e| e.name.clone());
                self.read(name.as_deref());
            }
            KeyCode::Char('~') => {
                if let Some(home) = dirs::home_dir() {
                    self.enter(home, None);
                }
            }
            KeyCode::Char(' ') => self.toggle_mark(),
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                let Some(e) = self.current().cloned() else {
                    return Action::None;
                };
                match e.kind {
                    EntryKind::Parent => self.up(),
                    EntryKind::Dir => self.enter(e.path, None),
                    EntryKind::Media => return Action::Choose(self.chosen(e.path)),
                }
            }
            _ => {}
        }
        Action::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn code(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    /// pics/ with two pictures, a text file, a hidden picture, a video and
    /// a folder.
    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let pics = dir.path().join("pics");
        fs::create_dir_all(pics.join("Zoo")).unwrap();
        for p in [
            "b.png",
            "A.PNG",
            ".secret.png",
            "Zoo/lion.png",
            "写真👨‍👩‍👧.jpg",
        ] {
            fs::write(pics.join(p), b"x").unwrap();
        }
        fs::write(pics.join("notes.txt"), "x").unwrap();
        fs::write(pics.join("clip.mp4"), "x").unwrap();
        dir
    }

    fn names(b: &Browser) -> Vec<&str> {
        b.items.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn folders_come_first_then_pictures_and_a_video_when_one_may_go() {
        let dir = tree();
        let pics = dir.path().join("pics");
        let b = Browser::open(&pics, 4, false);
        assert_eq!(names(&b), ["..", "Zoo", "A.PNG", "b.png", "写真👨‍👩‍👧.jpg"]);
        assert_eq!(b.current().unwrap().name, "Zoo", "not `..`");
        let b = Browser::open(&pics, 4, true);
        assert!(names(&b).contains(&"clip.mp4"));
    }

    #[test]
    fn enter_chooses_the_file_or_every_marked_one_and_goes_through_folders() {
        let dir = tree();
        let pics = dir.path().join("pics");
        let mut b = Browser::open(&pics, 4, false);
        b.key(code(KeyCode::Enter));
        assert!(b.dir.ends_with("Zoo"));
        b.key(key('h'));
        assert_eq!(b.current().unwrap().name, "Zoo");
        b.key(key('j'));
        b.key(key(' '));
        b.key(key(' '));
        let Action::Choose(chosen) = b.key(code(KeyCode::Enter)) else {
            panic!()
        };
        let chosen: Vec<_> = chosen.iter().map(|p| p.file_name().unwrap()).collect();
        assert_eq!(chosen, ["A.PNG", "b.png", "写真👨‍👩‍👧.jpg"]);
        b.key(key('.'));
        assert!(names(&b).contains(&".secret.png"));
        assert_eq!(b.key(code(KeyCode::Esc)), Action::Close);
    }

    #[test]
    fn marks_stop_at_the_room_left_and_a_missing_folder_says_why() {
        let dir = tree();
        let mut b = Browser::open(&dir.path().join("pics"), 1, false);
        b.key(key('j'));
        b.key(key(' '));
        b.key(key(' '));
        assert_eq!(b.marked.len(), 1);
        assert!(b.note.as_deref().unwrap().contains("no room"));
        let b = Browser::open(&dir.path().join("missing"), 4, false);
        assert!(b.error.as_deref().unwrap().starts_with("cannot read"));
        assert_eq!(names(&b), [".."]);
    }
}
