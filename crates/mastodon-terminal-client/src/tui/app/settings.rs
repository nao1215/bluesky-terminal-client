//! Local settings edits. Environment overrides remain visible and read-only.
use super::*;

impl App {
    fn fixed_setting(row: usize) -> Option<String> {
        let key = match row {
            2 => crate::download::DOWNLOAD_DIR_ENV,
            3 => crate::browser::BROWSER_ENV,
            _ => return None,
        };
        std::env::var(key).ok().filter(|v| !v.trim().is_empty())
    }

    pub(crate) fn settings_rows(&self) -> Vec<String> {
        let mut rows = vec![
            format!("{}: {}", t("Theme"), THEMES[self.theme_index].name),
            format!("{}: {}", t("Language"), crate::i18n::current().code()),
        ];
        for (row, label, saved) in [
            (
                2,
                n!("Download folder"),
                self.settings
                    .as_ref()
                    .and_then(|s| s.download_dir.as_deref()),
            ),
            (
                3,
                n!("Browser"),
                self.settings.as_ref().and_then(|s| s.browser.as_deref()),
            ),
        ] {
            let fixed = Self::fixed_setting(row);
            rows.push(format!(
                "{}: {}{}",
                t(label),
                fixed.as_deref().or(saved).unwrap_or(t("default")),
                if fixed.is_some() {
                    t(" (environment)")
                } else {
                    ""
                }
            ));
        }
        rows
    }

    pub(super) fn settings_key(&mut self, key: KeyEvent, mut selected: usize) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q' | 's') => return,
            KeyCode::Char('j') | KeyCode::Down => selected = (selected + 1) % 4,
            KeyCode::Char('k') | KeyCode::Up => selected = (selected + 3) % 4,
            KeyCode::Enter | KeyCode::Char(' ') => {
                if Self::fixed_setting(selected).is_some() {
                    self.say(t("set by the environment for this run"));
                } else if selected < 2 {
                    self.settings_return = true;
                    self.overlay = Some(if selected == 0 {
                        Overlay::Themes {
                            selected: self.theme_index,
                            before: self.theme_index,
                        }
                    } else {
                        Overlay::Languages {
                            selected: usize::from(crate::i18n::current() == crate::i18n::Lang::Ja),
                        }
                    });
                    return;
                } else {
                    let value = self
                        .settings
                        .as_ref()
                        .and_then(|s| {
                            if selected == 2 {
                                s.download_dir.as_deref()
                            } else {
                                s.browser.as_deref()
                            }
                        })
                        .unwrap_or("");
                    self.overlay = Some(Overlay::SettingText {
                        row: selected,
                        input: TextInput::single(value),
                    });
                    return;
                }
            }
            KeyCode::Char('x') if Self::fixed_setting(selected).is_none() => {
                if let Some(s) = &mut self.settings {
                    match selected {
                        0 => s.theme = None,
                        1 => s.language = None,
                        2 => s.download_dir = None,
                        _ => s.browser = None,
                    }
                    self.apply_settings(self.settings.clone(), self.depth);
                    self.settings_save = true;
                }
            }
            _ => {}
        }
        self.overlay = Some(Overlay::Settings { selected });
    }

    pub(super) fn setting_text_key(&mut self, key: KeyEvent, row: usize, mut input: TextInput) {
        match key.code {
            KeyCode::Esc => self.overlay = Some(Overlay::Settings { selected: row }),
            KeyCode::Enter => {
                if let Some(s) = &mut self.settings {
                    let text = input.text().trim().to_string();
                    let value = (!text.is_empty()).then_some(text);
                    if row == 2 {
                        s.download_dir = value;
                    } else {
                        s.browser = value;
                    }
                    self.settings_save = true;
                } else {
                    self.say(t("settings cannot be saved this run"));
                }
                self.overlay = Some(Overlay::Settings { selected: row });
            }
            _ => {
                input.handle_key(key);
                self.overlay = Some(Overlay::SettingText { row, input });
            }
        }
    }
}
