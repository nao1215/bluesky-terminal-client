//! The settings, as bsky's: the screen `s` opens (the theme, the
//! language, the download folder, the browser and the tabs), the theme
//! list `T` opens, previewed as the selection moves, and the language
//! list. What is chosen is kept in `settings.json`, beside what this
//! version does not know.

use super::*;
use crate::config::Settings;
use crate::i18n::{Lang, n, t, tf};
use crate::tui::input::TextInput;
use crate::tui::theme::{ColorDepth, THEMES};

/// One row of the settings screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingRow {
    pub name: &'static str,
    pub value: String,
    /// What enter does, or why it cannot.
    pub note: String,
    /// Whether the screen can change it.
    pub editable: bool,
    /// Whether `x` puts it back to the default: its value is in the file.
    pub resettable: bool,
}

/// The rows, by their place on the screen.
pub const ROW_THEME: usize = 0;
pub const ROW_LANGUAGE: usize = 1;
pub const ROW_DOWNLOAD: usize = 2;
pub const ROW_BROWSER: usize = 3;
pub const ROW_TABS: usize = 4;
pub const ROWS: usize = 5;

/// The environment variables that, set, decide a setting for the run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Environment {
    pub download_dir: Option<String>,
    pub browser: Option<String>,
    /// `LC_ALL`, `LC_MESSAGES` or `LANG`, the first set.
    pub locale: Option<String>,
}

impl Environment {
    /// The environment as `var` reads it.
    pub fn read(var: impl Fn(&str) -> Option<String>) -> Self {
        let get = |k: &str| var(k).filter(|v| !v.trim().is_empty());
        Environment {
            download_dir: get(crate::download::DOWNLOAD_DIR_ENV),
            browser: get(crate::browser::BROWSER_ENV),
            locale: ["LC_ALL", "LC_MESSAGES", "LANG"].into_iter().find_map(get),
        }
    }
}

/// The language `settings` name, else the one the environment asks for,
/// else English, as bsky's.
pub fn language(settings: &Settings, env: &Environment) -> Lang {
    settings
        .language
        .as_deref()
        .and_then(Lang::from_code)
        .or_else(|| env.locale.as_deref().and_then(Lang::from_code))
        .unwrap_or(Lang::En)
}

impl Tab {
    /// The name the `tabs` setting gives the tab.
    pub fn setting_name(self) -> &'static str {
        match self {
            Tab::Timeline(TimelineKind::Home) => "home",
            Tab::Timeline(TimelineKind::Local) => "local",
            Tab::Timeline(TimelineKind::Social) => "social",
            Tab::Timeline(TimelineKind::Global) => "global",
            Tab::Notifications => "notifications",
            Tab::Search => "search",
            Tab::Profile => "profile",
            Tab::Chat => "chat",
            // Kept as `kind/host/id/name`, which `tabs_setting` writes.
            Tab::Feed(_) => "feed",
        }
    }
}

impl App {
    /// Take the saved settings and the terminal's color depth into
    /// account: the language first, so that what is said from here on is
    /// in it, then the theme and the reactions given lately. A warning (a
    /// settings file that could not be used) is shown.
    pub fn apply_settings(
        &mut self,
        settings: Settings,
        depth: ColorDepth,
        env: Environment,
        warning: Option<String>,
    ) {
        self.color_depth = depth;
        self.settings_writable = warning.is_none();
        self.env = env;
        crate::i18n::set(language(&settings, &self.env));
        let mut warning = warning;
        let index = if depth == ColorDepth::None {
            theme::monochrome()
        } else {
            match settings.theme.as_deref() {
                Some(name) => theme::index_of(name).unwrap_or_else(|| {
                    warning = Some(tf(
                        "unknown theme {}; using {}",
                        &[&format!("{name:?}"), theme::DEFAULT],
                    ));
                    theme::chosen(None, depth).0
                }),
                None => theme::chosen(None, depth).0,
            }
        };
        if let Some(r) = &settings.recent_reactions {
            self.recent_reactions = r.clone();
        }
        self.settings = settings;
        self.set_theme(index);
        if let Some(w) = warning {
            self.error(w);
        }
    }

    pub(super) fn set_theme(&mut self, index: usize) {
        self.theme_index = index;
        self.theme = THEMES[index].for_depth(self.color_depth);
    }

    /// Settings to write, for the event loop.
    pub fn take_settings_save(&mut self) -> Option<Settings> {
        self.settings_to_save.take()
    }

    /// The outcome of writing the settings.
    pub fn settings_saved(&mut self, result: crate::error::Result<()>) {
        let note = self.save_note.take();
        match result {
            Ok(()) => {
                if let Some(n) = note {
                    self.info(n);
                }
            }
            Err(e) => self.error(e.message().to_string()),
        }
    }

    /// Keep the settings as they are now, saying `note` once they are.
    pub(super) fn save_settings(&mut self, note: String) {
        if !self.settings_writable {
            self.info(tf(
                "{} (not kept: settings.json could not be read, so it is left as it is)",
                &[&note],
            ));
            return;
        }
        self.settings.recent_reactions =
            (!self.recent_reactions.is_empty()).then(|| self.recent_reactions.clone());
        self.settings_to_save = Some(self.settings.clone());
        self.save_note = Some(note);
    }

    /// A reaction given: kept with the settings, so the next run offers it.
    pub(super) fn keep_recent_reactions(&mut self) {
        if self.settings_writable {
            self.settings.recent_reactions = Some(self.recent_reactions.clone());
            self.settings_to_save = Some(self.settings.clone());
        }
    }

    /// The folder `d` saves in when the settings name one and
    /// `MSKY_DOWNLOAD_DIR` does not decide it.
    pub fn download_dir(&self) -> Option<std::path::PathBuf> {
        if self.env.download_dir.is_some() {
            return None;
        }
        self.settings
            .download_dir
            .as_deref()
            .filter(|d| !d.trim().is_empty())
            .map(std::path::PathBuf::from)
    }

    /// The program links open with when the settings name one and
    /// `MSKY_BROWSER` does not decide it.
    pub fn browser(&self) -> Option<String> {
        if self.env.browser.is_some() {
            return None;
        }
        self.settings
            .browser
            .clone()
            .filter(|b| !b.trim().is_empty())
    }

    /// The language the client is shown in now.
    pub fn language(&self) -> Lang {
        language(&self.settings, &self.env)
    }

    /// `s`: the settings screen.
    pub(super) fn open_settings(&mut self) {
        self.overlay = Some(Overlay::Settings {
            selected: 0,
            edit: None,
        });
    }

    /// `T`: the themes, previewed as the selection moves.
    pub(super) fn open_theme_picker(&mut self) {
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

    /// Close a list the settings screen opened: back to the settings when
    /// they opened it, else to nothing.
    fn back_to_settings(&mut self) {
        self.overlay = self
            .settings_return
            .take()
            .map(|selected| Overlay::Settings {
                selected,
                edit: None,
            });
    }

    /// What the settings screen lists, in order.
    pub fn settings_rows(&self) -> Vec<SettingRow> {
        let fixed = |var: &str| tf("set by {} for this run", &[var]);
        let theme = SettingRow {
            name: n!("Theme"),
            value: THEMES[self.theme_index].name.to_string(),
            note: if self.color_depth == ColorDepth::None {
                t(n!("colors are off because NO_COLOR is set")).into()
            } else {
                t(n!("enter chooses one from the list, as T does")).into()
            },
            editable: self.color_depth != ColorDepth::None,
            resettable: false,
        };
        let language = SettingRow {
            name: n!("Language"),
            value: self.language().name().to_string(),
            note: t(n!("enter chooses one from the list")).into(),
            editable: true,
            resettable: self.settings.language.is_some(),
        };
        let text_row = |name,
                        var: &'static str,
                        env: &Option<String>,
                        set: &Option<String>,
                        default: String| {
            match (env, set.as_deref().filter(|v| !v.trim().is_empty())) {
                (Some(v), _) => SettingRow {
                    name,
                    value: v.clone(),
                    note: fixed(var),
                    editable: false,
                    resettable: false,
                },
                (None, Some(v)) => SettingRow {
                    name,
                    value: v.to_string(),
                    note: t(n!("enter changes it; x goes back to the default")).into(),
                    editable: true,
                    resettable: true,
                },
                (None, None) => SettingRow {
                    name,
                    value: default,
                    note: t(n!("the default; enter changes it")).into(),
                    editable: true,
                    resettable: false,
                },
            }
        };
        let download = text_row(
            n!("Download folder"),
            crate::download::DOWNLOAD_DIR_ENV,
            &self.env.download_dir,
            &self.settings.download_dir,
            crate::download::download_dir(None)
                .map(|d| d.display().to_string())
                .unwrap_or_default(),
        );
        let browser = text_row(
            n!("Browser"),
            crate::browser::BROWSER_ENV,
            &self.env.browser,
            &self.settings.browser,
            t(n!("the system's")).to_string(),
        );
        let tabs = SettingRow {
            name: n!("Tabs"),
            value: self
                .tabs
                .iter()
                .map(|t| self.tab_label(*t))
                .collect::<Vec<_>>()
                .join(", "),
            note: if self.settings.tabs.is_some() {
                t(n!(
                    "enter changes them, by name, in order; x goes back to the default"
                ))
                .into()
            } else {
                t(n!("the default; enter changes them, by name, in order")).into()
            },
            editable: true,
            resettable: self.settings.tabs.is_some(),
        };
        vec![theme, language, download, browser, tabs]
    }

    /// A key on the settings screen.
    pub(super) fn settings_key(
        &mut self,
        key: KeyEvent,
        selected: usize,
        edit: Option<TextInput>,
    ) -> Jobs {
        if let Some(mut input) = edit {
            match key.code {
                KeyCode::Esc => {
                    self.overlay = Some(Overlay::Settings {
                        selected,
                        edit: None,
                    });
                }
                KeyCode::Enter => {
                    self.overlay = Some(Overlay::Settings {
                        selected,
                        edit: None,
                    });
                    return self.keep_text(selected, input.text().trim().to_string());
                }
                _ => {
                    input.handle_key(key);
                    self.overlay = Some(Overlay::Settings {
                        selected,
                        edit: Some(input),
                    });
                }
            }
            return Vec::new();
        }
        let rows = self.settings_rows();
        let at = |selected| {
            Some(Overlay::Settings {
                selected,
                edit: None,
            })
        };
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.overlay = at((selected + 1) % ROWS),
            KeyCode::Char('k') | KeyCode::Up => self.overlay = at((selected + ROWS - 1) % ROWS),
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('s') => self.overlay = None,
            KeyCode::Char('x') if rows.get(selected).is_some_and(|r| r.resettable) => {
                return self.reset_setting(selected);
            }
            KeyCode::Enter => {
                let Some(row) = rows.get(selected) else {
                    return Vec::new();
                };
                if !row.editable {
                    self.info(row.note.clone());
                    return Vec::new();
                }
                match selected {
                    ROW_THEME => {
                        self.open_theme_picker();
                        self.settings_return = Some(selected);
                    }
                    ROW_LANGUAGE => {
                        self.settings_return = Some(selected);
                        self.overlay = Some(Overlay::Languages {
                            selected: Lang::ALL
                                .iter()
                                .position(|l| *l == self.language())
                                .unwrap_or(0),
                        });
                    }
                    _ => {
                        let now = match selected {
                            ROW_DOWNLOAD => self.settings.download_dir.clone(),
                            ROW_BROWSER => self.settings.browser.clone(),
                            _ => Some(self.tabs_setting().join(", ")),
                        };
                        self.overlay = Some(Overlay::Settings {
                            selected,
                            edit: Some(TextInput::single(&now.unwrap_or_default())),
                        });
                    }
                }
            }
            _ => {}
        }
        Vec::new()
    }

    /// A text row changed to `value`.
    fn keep_text(&mut self, row: usize, value: String) -> Jobs {
        let set = (!value.is_empty()).then(|| value.clone());
        match row {
            ROW_DOWNLOAD => {
                self.settings.download_dir = set;
                self.save_settings(tf("download folder: {}", &[&value]));
            }
            ROW_BROWSER => {
                self.settings.browser = set;
                self.save_settings(tf("browser: {}", &[&value]));
            }
            ROW_TABS => {
                let names: Vec<String> = value
                    .split(',')
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                    .map(str::to_string)
                    .collect();
                let before = self.feeds.clone();
                let (tabs, unknown) = self.tabs_for_host(&names);
                if let Some(why) = unknown {
                    self.feeds = before;
                    self.error(why);
                    return Vec::new();
                }
                self.tabs = tabs.clone();
                let jobs = self.set_tabs(tabs);
                let shown = self.tabs_setting().join(", ");
                self.keep_tabs(tf("tabs: {}", &[&shown]));
                return jobs;
            }
            _ => {}
        }
        Vec::new()
    }

    /// `x`: the row's setting back to the default.
    fn reset_setting(&mut self, row: usize) -> Jobs {
        let mut jobs = Vec::new();
        match row {
            ROW_LANGUAGE => {
                self.settings.language = None;
                crate::i18n::set(self.language());
            }
            ROW_DOWNLOAD => self.settings.download_dir = None,
            ROW_BROWSER => self.settings.browser = None,
            ROW_TABS => {
                self.settings.tabs = None;
                jobs = self.set_tabs(DEFAULT_TABS.to_vec());
            }
            _ => return jobs,
        }
        let name = t(self.settings_rows()[row].name).to_string();
        self.save_settings(tf("{}: back to the default", &[&name]));
        jobs
    }

    /// A key on the theme list: j k preview, enter keeps, esc puts back.
    pub(super) fn themes_key(&mut self, key: KeyEvent, selected: usize, previous: usize) {
        let n = THEMES.len();
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let s = (selected + 1) % n;
                self.set_theme(s);
                self.overlay = Some(Overlay::Themes {
                    selected: s,
                    previous,
                });
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let s = (selected + n - 1) % n;
                self.set_theme(s);
                self.overlay = Some(Overlay::Themes {
                    selected: s,
                    previous,
                });
            }
            KeyCode::Enter => {
                self.set_theme(selected);
                self.settings.theme = Some(THEMES[selected].name.to_string());
                self.back_to_settings();
                self.save_settings(tf("theme: {}", &[THEMES[selected].name]));
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.set_theme(previous);
                self.back_to_settings();
            }
            _ => {}
        }
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
}
