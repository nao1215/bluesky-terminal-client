//! A directory browser for choosing pictures and videos from the user's
//! disk. It lists folders and those files only, and the view previews the
//! selected one beside the list. In its folder mode it lists folders only,
//! and chooses the one it is in.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent};

use crate::media::{self, is_image_name};
use crate::tui::app::List;
use crate::video::is_video_name;

/// What an entry of the listing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryKind {
    /// `..`, the folder above.
    Parent,
    Dir,
    /// A picture or a video.
    Media,
}

/// One line of the listing.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub kind: EntryKind,
    /// File size, for pictures and videos.
    pub bytes: u64,
}

/// What the browser asks of whoever opened it.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    None,
    /// Close without choosing.
    Close,
    /// These pictures were chosen, in order.
    Choose(Vec<PathBuf>),
}

/// The browser's state.
#[derive(Debug, Clone)]
pub struct Browser {
    pub dir: PathBuf,
    pub list: List<Entry>,
    /// Whether names starting with a dot are listed.
    pub show_hidden: bool,
    /// Why the folder could not be read.
    pub error: Option<String>,
    /// Pictures marked with space, which Enter chooses together.
    pub marked: Vec<PathBuf>,
    /// How many pictures may still be chosen.
    pub room: usize,
    /// Whether videos are listed too (not for an avatar, nor next to
    /// pictures already attached).
    pub videos: bool,
    /// A message about the last key, such as "no room for more".
    pub note: Option<String>,
    /// Whether a folder is chosen rather than files: only folders are
    /// listed, and Space chooses the one shown.
    pub folders: bool,
    /// What each file looked at so far is, read from its header once.
    pub info: HashMap<PathBuf, media::Info>,
}

impl Browser {
    /// Open at `dir`, where at most `room` pictures, or a video when
    /// `videos`, may be chosen.
    pub fn open(dir: &Path, room: usize, videos: bool) -> Self {
        let mut b = Self {
            dir: std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf()),
            list: List::default(),
            show_hidden: false,
            error: None,
            marked: Vec::new(),
            room: room.max(1),
            videos,
            note: None,
            folders: false,
            info: HashMap::new(),
        };
        b.read(None);
        b
    }

    /// Open at `dir` to choose a folder.
    pub fn folder(dir: &Path) -> Self {
        let mut b = Self::open(dir, 1, false);
        b.folders = true;
        b.read(None);
        b
    }

    /// The selected entry.
    pub fn current(&self) -> Option<&Entry> {
        self.list.current()
    }

    /// What the selected file is.
    pub fn current_info(&self) -> Option<media::Info> {
        let e = self.current().filter(|e| e.kind == EntryKind::Media)?;
        self.info.get(&e.path).copied()
    }

    fn look_at_current(&mut self) {
        if let Some(e) = self.list.current().filter(|e| e.kind == EntryKind::Media)
            && !self.info.contains_key(&e.path)
        {
            let i = media::inspect(&e.path);
            self.info.insert(e.path.clone(), i);
        }
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
                    } else if !self.folders
                        && meta.is_file()
                        && (is_image_name(&name) || (self.videos && is_video_name(&name)))
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
            Err(e) => {
                self.error = Some(crate::i18n::tf(
                    "cannot read {}: {}",
                    &[&(self.dir.display()).to_string(), &e.to_string()],
                ))
            }
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
        self.list = List {
            items,
            selected,
            loaded: true,
            ..List::default()
        };
        self.look_at_current();
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
        let Some(e) = self.list.current().filter(|e| e.kind == EntryKind::Media) else {
            return;
        };
        let path = e.path.clone();
        if let Some(i) = self.marked.iter().position(|p| *p == path) {
            self.marked.remove(i);
        } else if self.marked.len() < self.room {
            self.marked.push(path);
        } else {
            self.note = Some(crate::i18n::tf(
                "{} marked; there is no room for more",
                &[&self.marked.len().to_string()],
            ));
            return;
        }
        self.list.step(1);
    }

    /// What Enter on the picture at `current` chooses: the marked pictures
    /// and this one too while there is room. With room for one, the picture
    /// under the cursor is the choice.
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

    /// Handle a key.
    pub fn key(&mut self, key: KeyEvent) -> Action {
        self.note = None;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return Action::Close,
            KeyCode::Char('j') | KeyCode::Down => self.list.step(1),
            KeyCode::Char('k') | KeyCode::Up => self.list.step(-1),
            KeyCode::PageDown => self.list.step(10),
            KeyCode::PageUp => self.list.step(-10),
            KeyCode::Char('g') | KeyCode::Home => self.list.step(isize::MIN / 2),
            KeyCode::Char('G') | KeyCode::End => self.list.step(isize::MAX / 2),
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
            KeyCode::Char(' ') if self.folders => return Action::Choose(vec![self.dir.clone()]),
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
        self.look_at_current();
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

    fn png(path: &Path, w: u32, h: u32) {
        image::RgbImage::from_pixel(w, h, image::Rgb([9, 9, 9]))
            .save(path)
            .unwrap();
    }

    /// pics/ with two pictures, a text file, a hidden picture and a folder.
    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let pics = dir.path().join("pics");
        fs::create_dir_all(pics.join("Zoo")).unwrap();
        png(&pics.join("b.png"), 4, 3);
        png(&pics.join("A.PNG"), 2, 2);
        png(&pics.join(".secret.png"), 1, 1);
        fs::write(pics.join("notes.txt"), "x").unwrap();
        png(&pics.join("Zoo").join("lion.png"), 8, 8);
        dir
    }

    fn names(b: &Browser) -> Vec<&str> {
        b.list.items.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn folders_come_first_then_pictures_and_nothing_else() {
        let dir = tree();
        let b = Browser::open(&dir.path().join("pics"), 4, false);
        assert_eq!(names(&b), ["..", "Zoo", "A.PNG", "b.png"]);
        assert_eq!(b.current().unwrap().name, "Zoo", "not `..`");
    }

    #[test]
    fn hidden_names_show_on_request_and_the_selection_stays() {
        let dir = tree();
        let mut b = Browser::open(&dir.path().join("pics"), 4, false);
        b.key(key('G'));
        b.key(key('.'));
        assert!(names(&b).contains(&".secret.png"));
        assert_eq!(b.current().unwrap().name, "b.png");
        b.key(key('.'));
        assert!(!names(&b).contains(&".secret.png"));
    }

    #[test]
    fn entering_a_folder_and_going_up_selects_where_it_came_from() {
        let dir = tree();
        let pics = dir.path().join("pics");
        let mut b = Browser::open(&pics, 4, false);
        b.key(code(KeyCode::Enter));
        assert!(b.dir.ends_with("Zoo"));
        assert_eq!(names(&b), ["..", "lion.png"]);
        assert_eq!(
            b.current_info().and_then(|i| i.dims),
            Some((8, 8)),
            "the picture's size is read"
        );
        b.key(key('h'));
        assert_eq!(b.dir, std::path::absolute(&pics).unwrap());
        assert_eq!(b.current().unwrap().name, "Zoo");
        // `..` goes up too.
        b.key(key('g'));
        b.key(code(KeyCode::Enter));
        assert_eq!(b.dir, std::path::absolute(dir.path()).unwrap());
        assert_eq!(b.current().unwrap().name, "pics");
    }

    #[test]
    fn enter_chooses_the_picture_or_every_marked_one() {
        let dir = tree();
        let pics = dir.path().join("pics");
        let mut b = Browser::open(&pics, 4, false);
        b.key(key('j'));
        assert_eq!(
            b.key(code(KeyCode::Enter)),
            Action::Choose(vec![std::path::absolute(pics.join("A.PNG")).unwrap()])
        );
        // Space marks and moves on; Enter takes the marks, in order.
        b.key(key(' '));
        b.key(key(' '));
        assert_eq!(b.marked.len(), 2);
        let Action::Choose(chosen) = b.key(code(KeyCode::Enter)) else {
            panic!()
        };
        let chosen: Vec<_> = chosen.iter().map(|p| p.file_name().unwrap()).collect();
        assert_eq!(chosen, ["A.PNG", "b.png"]);
        assert_eq!(b.key(code(KeyCode::Esc)), Action::Close);
    }

    #[test]
    fn enter_on_an_unmarked_picture_adds_it_to_the_marks() {
        let dir = tree();
        let mut b = Browser::open(&dir.path().join("pics"), 4, false);
        b.key(key('j'));
        b.key(key(' '));
        let Action::Choose(chosen) = b.key(code(KeyCode::Enter)) else {
            panic!()
        };
        let chosen: Vec<_> = chosen.iter().map(|p| p.file_name().unwrap()).collect();
        assert_eq!(chosen, ["A.PNG", "b.png"]);
        // With room for one, the cursor wins over an earlier mark.
        let mut b = Browser::open(&dir.path().join("pics"), 1, false);
        b.key(key('j'));
        b.key(key(' '));
        let Action::Choose(chosen) = b.key(code(KeyCode::Enter)) else {
            panic!()
        };
        assert!(chosen[0].ends_with("b.png") && chosen.len() == 1);
    }

    #[test]
    fn marks_stop_at_the_room_left() {
        let dir = tree();
        let mut b = Browser::open(&dir.path().join("pics"), 1, false);
        b.key(key('j'));
        b.key(key(' '));
        b.key(key(' '));
        assert_eq!(b.marked.len(), 1);
        assert!(b.note.as_deref().unwrap().contains("no room"));
        // A folder cannot be marked.
        b.key(key('g'));
        b.key(key('j'));
        b.key(key(' '));
        assert_eq!(b.marked.len(), 1);
    }

    #[test]
    fn videos_are_listed_only_when_one_may_be_chosen() {
        let dir = tree();
        let pics = dir.path().join("pics");
        fs::write(pics.join("clip.mp4"), b"\0\0\0\x18ftypisom").unwrap();
        assert!(!names(&Browser::open(&pics, 4, false)).contains(&"clip.mp4"));
        let b = Browser::open(&pics, 4, true);
        assert_eq!(names(&b), ["..", "Zoo", "A.PNG", "b.png", "clip.mp4"]);
    }

    #[test]
    fn a_folder_that_cannot_be_read_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let b = Browser::open(&dir.path().join("missing"), 4, false);
        assert!(b.error.as_deref().unwrap().starts_with("cannot read"));
        assert_eq!(names(&b), [".."], "the way out is still there");
    }

    #[test]
    fn a_folder_is_chosen_where_the_browser_is_and_files_are_not_listed() {
        let dir = tree();
        let mut b = Browser::folder(&dir.path().join("pics"));
        // Only the folder above and the folder in it; the pictures are not.
        assert_eq!(names(&b), ["..", "Zoo"]);
        assert_eq!(b.key(code(KeyCode::Enter)), Action::None);
        assert_eq!(names(&b), [".."]);
        assert_eq!(
            b.key(key(' ')),
            Action::Choose(vec![
                std::path::absolute(dir.path().join("pics").join("Zoo")).unwrap()
            ])
        );
        b.key(key('h'));
        assert_eq!(b.dir, std::path::absolute(dir.path().join("pics")).unwrap());
    }
}
