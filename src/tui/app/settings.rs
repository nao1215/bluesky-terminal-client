//! The settings: the theme, pictures, the folders, the video service and the browser, and the settings screen.

use super::*;
use crate::i18n::Lang;

impl App {
    /// Take the saved settings and the terminal's color depth into account.
    /// A warning (a settings file that could not be used) is shown.
    pub fn apply_settings(
        &mut self,
        settings: Settings,
        depth: ColorDepth,
        warning: Option<String>,
    ) {
        self.color_depth = depth;
        self.settings_writable = warning.is_none();
        // First, so that what is said from here on is in it.
        crate::i18n::set(crate::config::language(&settings, &self.env));
        let mut warning = warning;
        let index = match settings.theme.as_deref() {
            Some(name) => theme::index_of(name).unwrap_or_else(|| {
                warning = Some(tf(
                    "unknown theme {}; using {}",
                    &[&format!("{name:?}"), THEMES[0].name],
                ));
                0
            }),
            None => 0,
        };
        self.settings = settings;
        self.load_columns();
        self.set_theme(index);
        if let Some(w) = warning {
            self.error(w);
        }
    }

    pub(super) fn set_theme(&mut self, index: usize) {
        self.theme_index = index;
        self.theme = THEMES[index].for_depth(self.color_depth);
    }

    /// Settings the user asked to save, for the event loop to write.
    pub fn take_settings_save(&mut self) -> Option<Settings> {
        self.settings_to_save.take()
    }

    /// The outcome of writing the settings.
    pub fn settings_saved(&mut self, result: crate::error::Result<()>) {
        let note = self.save_note.take();
        match result {
            Ok(()) => self.info(note.unwrap_or_default()),
            Err(e) => self.error(e.message().to_string()),
        }
    }

    /// The language the client is shown in now.
    pub fn language(&self) -> Lang {
        crate::config::language(&self.settings, &self.env)
    }

    /// A key on the list of languages: the one chosen is used at once and
    /// kept, and the settings come back.
    pub(super) fn languages_key(&mut self, key: KeyEvent, selected: usize) {
        let n = Lang::ALL.len();
        let at = |selected| Some(Overlay::Languages { selected });
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.overlay = at((selected + 1) % n),
            KeyCode::Char('k') | KeyCode::Up => self.overlay = at((selected + n - 1) % n),
            KeyCode::Enter => {
                let lang = Lang::ALL[selected.min(n - 1)];
                self.settings.language = Some(lang.code().to_string());
                crate::i18n::set(lang);
                self.back_to_settings();
                self.save_settings(tf("language: {}", &[lang.name()]));
            }
            KeyCode::Esc | KeyCode::Char('q') => self.back_to_settings(),
            _ => {}
        }
    }

    /// Close a list the settings screen opened (themes, accounts): back to
    /// the settings when they opened it, else to nothing.
    pub(super) fn back_to_settings(&mut self) {
        self.overlay = self
            .settings_return
            .take()
            .map(|selected| Overlay::Settings {
                selected,
                edit: None,
            });
    }

    pub(super) fn open_theme_picker(&mut self) {
        // Back to the list unless the settings screen, which opens it too,
        // says otherwise after this.
        self.settings_return = None;
        if self.color_depth == ColorDepth::None {
            self.error(n!("colors are off because NO_COLOR is set"));
            return;
        }
        self.overlay = Some(Overlay::Themes {
            selected: self.theme_index,
            previous: self.theme_index,
        });
    }

    /// Run as text, for a terminal that cannot show pictures or video.
    pub fn without_pictures(&mut self) {
        self.pictures = false;
        if self.status.is_none() {
            self.info(n!(
                "this terminal cannot show pictures; bsky runs without them"
            ));
        }
    }

    /// The answer to pictures turned back on: whether the terminal draws
    /// them after all.
    pub fn pictures_back(&mut self, shown: bool) {
        self.pictures = shown;
        if !shown {
            self.info(n!(
                "this terminal cannot show pictures; bsky runs without them"
            ));
        }
    }

    /// Pictures turned on or off on the settings screen, for the event loop.
    pub fn take_pictures_change(&mut self) -> Option<bool> {
        self.pictures_change.take()
    }

    /// Where `d` saves, as the environment and the settings decide.
    pub fn download_dir(&self) -> Option<PathBuf> {
        crate::config::download_dir(&self.env, &self.settings).0
    }

    /// Where pictures are cached, or `None` for no cache.
    pub fn cache_dir(&self) -> Option<PathBuf> {
        crate::config::cache_dir(&self.env, &self.settings).0
    }

    /// The video service uploads go to.
    pub(super) fn video_service(&self) -> String {
        crate::config::video_service(&self.env, &self.settings).0
    }

    /// The program that opens links, or `None` for the system's.
    pub(super) fn browser(&self) -> Option<String> {
        crate::config::browser(&self.env, &self.settings).0
    }

    /// A job that opens `url` in the browser the settings name.
    pub(super) fn open_url(&self, url: String) -> Job {
        Job::OpenLink {
            url,
            browser: self.browser(),
        }
    }

    /// What the settings screen lists, in order.
    pub fn settings_rows(&self) -> Vec<SettingRow> {
        use crate::config::{self, Source};
        let fixed = |var: &str| tf("set by {} for this run", &[var]);
        let path = |p: Option<PathBuf>, none: &str| {
            p.map_or_else(|| none.to_string(), |p| p.display().to_string())
        };
        let theme = if self.color_depth == ColorDepth::None {
            SettingRow {
                name: n!("Theme"),
                value: THEMES[self.theme_index].name.to_string(),
                note: n!("colors are off because NO_COLOR is set").into(),
                editable: false,
                resettable: false,
            }
        } else {
            SettingRow {
                name: n!("Theme"),
                value: THEMES[self.theme_index].name.to_string(),
                note: n!("enter chooses one from the list, as T does").into(),
                editable: true,
                resettable: false,
            }
        };
        let pictures = match &self.env.graphics {
            Some(v) => SettingRow {
                name: n!("Pictures"),
                value: v.clone(),
                note: fixed(crate::terminal::GRAPHICS_ENV),
                editable: false,
                resettable: false,
            },
            None if self.settings.pictures_off() => SettingRow {
                name: n!("Pictures"),
                value: crate::i18n::t("off").into(),
                note: n!("enter draws them again where the terminal can").into(),
                editable: true,
                resettable: false,
            },
            None => SettingRow {
                name: n!("Pictures"),
                value: crate::i18n::t("auto").into(),
                note: if self.pictures {
                    n!("enter turns them off: posts say what they carry").into()
                } else {
                    n!("this terminal cannot show them").into()
                },
                editable: true,
                resettable: false,
            },
        };
        // A row whose value may come from a variable, the file, or bsky.
        let row = |name, var: &'static str, value: String, from: Source, change: &str| {
            let note = match from {
                Source::Env => fixed(var),
                Source::File => tf("{}; x goes back to the default", &[crate::i18n::t(change)]),
                Source::Default => tf("the default; {}", &[crate::i18n::t(change)]),
            };
            SettingRow {
                name,
                value,
                note,
                editable: from != Source::Env,
                resettable: from == Source::File,
            }
        };
        let (download, download_from) = config::download_dir(&self.env, &self.settings);
        let (cache, cache_from) = config::cache_dir(&self.env, &self.settings);
        let (video, video_from) = config::video_service(&self.env, &self.settings);
        let (browser, browser_from) = config::browser(&self.env, &self.settings);
        vec![
            theme,
            pictures,
            row(
                n!("Download folder"),
                config::DOWNLOAD_DIR_ENV,
                path(download, crate::i18n::t("none")),
                download_from,
                n!("enter chooses another folder"),
            ),
            row(
                n!("Picture cache"),
                config::CACHE_DIR_ENV,
                path(cache, crate::i18n::t("off")),
                cache_from,
                n!("enter chooses another folder"),
            ),
            row(
                n!("Video service"),
                config::VIDEO_SERVICE_ENV,
                video,
                video_from,
                n!("enter types another address"),
            ),
            row(
                n!("Browser"),
                crate::browser::BROWSER_ENV,
                browser.unwrap_or_else(|| crate::browser::system_opener().to_string()),
                browser_from,
                n!("enter types the program that opens links"),
            ),
            SettingRow {
                name: n!("Language"),
                value: self.language().name().to_string(),
                note: if self.settings.language.is_some() {
                    n!("enter chooses another; x goes back to the system's")
                } else {
                    n!("the system's; enter chooses another")
                }
                .into(),
                editable: true,
                resettable: self.settings.language.is_some(),
            },
            SettingRow {
                name: n!("Account"),
                value: self.session.as_ref().map_or_else(
                    || crate::i18n::t("none").into(),
                    |s| format!("@{}", s.handle),
                ),
                note: n!("enter switches, adds or logs out an account, as A does").into(),
                editable: true,
                resettable: false,
            },
        ]
    }

    /// A key on the settings screen, or on the folder browser or the line
    /// of text it opened.
    pub(super) fn settings_key(&mut self, key: KeyEvent, selected: usize) -> Vec<Job> {
        if let Some(Overlay::Settings {
            edit: Some(edit), ..
        }) = &mut self.overlay
        {
            let chosen = match edit {
                SettingEdit::Folder(b) => match b.key(key) {
                    Action::None => return Vec::new(),
                    Action::Close => None,
                    Action::Choose(mut dirs) => Some(dirs.pop().map(|d| d.display().to_string())),
                },
                SettingEdit::Text(input) => match key.code {
                    KeyCode::Esc => None,
                    KeyCode::Enter => Some(Some(input.text())),
                    _ => {
                        input.handle_key(key);
                        return Vec::new();
                    }
                },
            };
            match chosen {
                Some(value) => self.set_setting(selected, value.unwrap_or_default()),
                None => self.close_edit(selected),
            }
            return Vec::new();
        }
        let n = self.settings_rows().len();
        let moved = |selected| {
            Some(Overlay::Settings {
                selected,
                edit: None,
            })
        };
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.overlay = moved((selected + 1) % n),
            KeyCode::Char('k') | KeyCode::Up => self.overlay = moved((selected + n - 1) % n),
            KeyCode::Esc | KeyCode::Char('q' | 's') => self.overlay = None,
            KeyCode::Enter | KeyCode::Char(' ') => self.change_setting(selected),
            KeyCode::Char('x') => self.reset_setting(selected),
            _ => {}
        }
        Vec::new()
    }

    /// Back to the list, on the row that was being changed.
    pub(super) fn close_edit(&mut self, selected: usize) {
        self.overlay = Some(Overlay::Settings {
            selected,
            edit: None,
        });
    }

    /// Enter on a row of the settings screen.
    pub(super) fn change_setting(&mut self, selected: usize) {
        let Some(row) = self.settings_rows().into_iter().nth(selected) else {
            return;
        };
        if !row.editable {
            self.info(row.note);
            return;
        }
        let edit = match row.name {
            "Theme" => {
                self.open_theme_picker();
                self.settings_return = Some(selected);
                return;
            }
            "Language" => {
                let at = Lang::ALL.iter().position(|l| *l == self.language());
                self.overlay = Some(Overlay::Languages {
                    selected: at.unwrap_or(0),
                });
                self.settings_return = Some(selected);
                return;
            }
            "Account" => {
                let at = self.current_account_index().unwrap_or(0);
                self.overlay = Some(Overlay::Accounts { selected: at });
                self.settings_return = Some(selected);
                return;
            }
            "Pictures" => {
                let off = !self.settings.pictures_off();
                let value = if off { n!("off") } else { n!("auto") };
                self.settings.pictures = Some(value.into());
                self.pictures_change = Some(!off);
                if off {
                    self.pictures = false;
                }
                self.save_settings(tf("pictures: {}", &[crate::i18n::t(value)]));
                return;
            }
            "Download folder" => SettingEdit::Folder(Box::new(Browser::folder(
                &self.folder_start(self.download_dir()),
            ))),
            "Picture cache" => SettingEdit::Folder(Box::new(Browser::folder(
                &self.folder_start(self.cache_dir()),
            ))),
            "Video service" => SettingEdit::Text(TextInput::single(&self.video_service())),
            "Browser" => SettingEdit::Text(TextInput::single(&self.browser().unwrap_or_default())),
            _ => return,
        };
        self.overlay = Some(Overlay::Settings {
            selected,
            edit: Some(edit),
        });
    }

    /// Where the folder browser opens: at the folder set now, or the
    /// nearest folder above it that is there, else where pictures are
    /// browsed from.
    pub(super) fn folder_start(&self, now: Option<PathBuf>) -> PathBuf {
        now.as_deref()
            .and_then(|p| p.ancestors().find(|a| a.is_dir()))
            .map(PathBuf::from)
            .unwrap_or_else(|| browse_start(&self.browse_from))
    }

    /// `x`: a setting kept in `settings.json` goes back to its default.
    pub(super) fn reset_setting(&mut self, selected: usize) {
        let Some(row) = self.settings_rows().into_iter().nth(selected) else {
            return;
        };
        if !row.resettable {
            return;
        }
        self.set_setting(selected, String::new());
    }

    /// Keep `value` for the setting on row `selected`; an empty value is
    /// the default. A folder is refused, with the reason, when it cannot be
    /// written, and a video service that is not a web address is refused.
    pub(super) fn set_setting(&mut self, selected: usize, value: String) {
        let Some(row) = self.settings_rows().into_iter().nth(selected) else {
            return;
        };
        let value = value.trim().to_string();
        let folder = matches!(row.name, "Download folder" | "Picture cache");
        if folder
            && !value.is_empty()
            && let Err(why) = crate::config::check_writable(std::path::Path::new(&value))
        {
            // The browser stays open for another choice.
            self.error(why);
            return;
        }
        if row.name == "Video service"
            && !value.is_empty()
            && !(value.starts_with("https://") || value.starts_with("http://"))
        {
            self.error(n!(
                "the video service is a web address, such as https://video.bsky.app"
            ));
            return;
        }
        let kept = (!value.is_empty()).then(|| value.clone());
        match row.name {
            "Download folder" => self.settings.download_dir = kept,
            "Picture cache" => {
                self.settings.cache_dir = kept;
                // The pictures are kept in the new folder from now on.
                if self.pictures {
                    self.pictures_change = Some(true);
                }
            }
            "Video service" => self.settings.video_service = kept,
            "Browser" => self.settings.browser = kept,
            "Language" => {
                self.settings.language = kept;
                crate::i18n::set(self.language());
            }
            _ => return,
        }
        self.close_edit(selected);
        let shown = self
            .settings_rows()
            .into_iter()
            .nth(selected)
            .map(|r| r.value)
            .unwrap_or_default();
        let name = crate::i18n::t(row.name).to_lowercase();
        self.save_settings(if value.is_empty() {
            tf("{}: back to the default, {}", &[&name, &shown])
        } else {
            format!("{name}: {shown}")
        });
    }

    /// Save the settings as they are now, saying `note` once they are
    /// written. An unreadable `settings.json` is not overwritten: the change
    /// holds for this run, and the reason is shown.
    pub(super) fn save_settings(&mut self, note: String) {
        if self.settings_writable {
            self.settings_to_save = Some(self.settings.clone());
            self.save_note = Some(note);
        } else {
            self.error(tf(
                "{} for this session only; settings.json could not be read, so it is not overwritten (fix or remove it to save)",
                &[&note],
            ));
        }
    }
}
