use std::collections::HashSet;

use ansi_to_tui::IntoText;
use crossterm::event::KeyCode;
use error_stack::Report;
use ratatui::{
    macros::vertical,
    prelude::*,
    widgets::{Block, BorderType, List, ListItem, ListState, Padding, Paragraph},
};

use crate::{
    App,
    app::{self, MainFocusState},
    event::AppEvent,
    features::{FeaturesGraph, FetchFeaturesError},
    manifest::dependency::{DependencyCursor, DependencyRef},
    ui::{FOCUSED_BORDER_STYLE, icons::Icons},
    updater::{CheckForUpdateError, UpdateStatus, VersionType},
};

use ratatui_input_manager::keymap;

#[derive(Debug, Clone)]
pub enum FocusedArea {
    Main,
    Versions {
        /// The currently selected version, if any.
        selected: usize,
    },
    Features {
        /// The currently selected feature, if any.
        selected: usize,
    },
}

pub struct DependencyKeybinds<'a> {
    app: &'a mut App,
}

impl<'a> DependencyKeybinds<'a> {
    pub const fn new(app: &'a mut App) -> Self {
        Self { app }
    }
}

#[keymap(backend = "crossterm")]
impl DependencyKeybinds<'_> {
    /// Focus the versions list
    #[keybind(pressed(key=KeyCode::Char('v')))]
    #[tracing::instrument(skip(self))]
    fn focus_versions(&mut self) {
        let Some(MainFocusState::Dependency { focused_area, .. }) = &mut self.app.main_focus_state
        else {
            panic!("Tried to focus versions but the main focus state was not a dependency.")
        };

        *focused_area = FocusedArea::Versions { selected: 0 };
    }

    /// Focus the features list
    #[keybind(pressed(key=KeyCode::Char('f')))]
    #[tracing::instrument(skip(self))]
    fn focus_features(&mut self) {
        let Some(MainFocusState::Dependency { focused_area, .. }) = &mut self.app.main_focus_state
        else {
            panic!("Tried to focus features but the main focus state was not a dependency.")
        };

        *focused_area = FocusedArea::Features { selected: 0 };
    }

    /// Toggle default features
    #[keybind(pressed(key=KeyCode::Char('d')))]
    #[tracing::instrument(skip(self))]
    fn toggle_default_features(&mut self) {
        let Some(MainFocusState::Dependency { cursor, .. }) = &mut self.app.main_focus_state else {
            panic!(
                "Tried to toggle default features but the main focus state was not a dependency."
            )
        };

        let dependency = self
            .app
            .manifest
            .resolve_dependency(cursor)
            .unwrap()
            .unwrap();

        let new_value = !dependency.default_features();

        let mut dependency_item = self.app.manifest.resolve_dependency_mut(cursor).unwrap();

        dependency_item.set_default_features(new_value).unwrap();

        self.app.manifest.save().unwrap();
    }
}

pub struct VersionListKeybinds<'a> {
    app: &'a mut App,
}

impl<'a> VersionListKeybinds<'a> {
    pub const fn new(app: &'a mut App) -> Self {
        Self { app }
    }
}

#[keymap(backend = "crossterm")]
impl VersionListKeybinds<'_> {
    /// Unfocus the versions list
    #[keybind(pressed(key=KeyCode::Esc))]
    #[keybind(pressed(key=KeyCode::Char('q')))]
    #[tracing::instrument(skip(self))]
    fn close_versions(&mut self) {
        let Some(MainFocusState::Dependency { focused_area, .. }) = &mut self.app.main_focus_state
        else {
            panic!("Tried to close versions but the main focus state was not a dependency.")
        };

        *focused_area = FocusedArea::Main;
    }

    /// Next version
    #[keybind(pressed(key=KeyCode::Down))]
    #[keybind(pressed(key=KeyCode::Char('j')))]
    #[tracing::instrument(skip(self))]
    pub fn next_version(&mut self) {
        let Some(MainFocusState::Dependency {
            focused_area,
            versions,
            ..
        }) = &mut self.app.main_focus_state
        else {
            panic!("Tried to select next version but the main focus state was not a dependency.")
        };

        let FocusedArea::Versions { selected } = focused_area else {
            panic!("Tried to select next version but the focused area was not versions.")
        };

        if let Some(Ok(versions)) = versions {
            let next = (*selected + 1) % versions.len();
            *selected = next;
        }
    }

    /// Previous version
    #[keybind(pressed(key=KeyCode::Up))]
    #[keybind(pressed(key=KeyCode::Char('k')))]
    #[tracing::instrument(skip(self))]
    fn previous_version(&mut self) {
        let Some(MainFocusState::Dependency {
            focused_area,
            versions,
            ..
        }) = &mut self.app.main_focus_state
        else {
            panic!(
                "Tried to select previous version but the main focus state was not a dependency."
            )
        };

        let FocusedArea::Versions { selected } = focused_area else {
            panic!("Tried to select previous version but the focused area was not versions.")
        };

        if let Some(Ok(versions)) = versions {
            let next = (*selected + versions.len() - 1) % versions.len();
            *selected = next;
        }
    }

    /// Set version
    #[keybind(pressed(key=KeyCode::Enter))]
    fn set_version(&mut self) {
        let Some(MainFocusState::Dependency {
            cursor,
            focused_area,
            versions,
            ..
        }) = &self.app.main_focus_state
        else {
            panic!("Tried to set version but the main focus state was not a dependency.")
        };

        let FocusedArea::Versions { selected } = focused_area else {
            panic!("Tried to set version but the focused area was not versions.")
        };

        if let Some(Ok(versions)) = versions {
            let version = versions
                .get(*selected)
                .expect("Tried to set version but the selected index was out of bounds.");

            let mut dependency_item = self.app.manifest.resolve_dependency_mut(cursor).unwrap();

            dependency_item.set_version(version).unwrap();

            self.app.manifest.save().unwrap();

            self.app.events.send(AppEvent::UpdateCheck {
                cursor: cursor.clone(),
            });
        }
    }
}

pub struct FeaturesListKeybinds<'a> {
    app: &'a mut App,
}

impl<'a> FeaturesListKeybinds<'a> {
    pub const fn new(app: &'a mut App) -> Self {
        Self { app }
    }
}

#[keymap(backend = "crossterm")]
impl FeaturesListKeybinds<'_> {
    /// Toggle default features
    #[keybind(pressed(key=KeyCode::Char('d')))]
    #[tracing::instrument(skip(self))]
    fn toggle_default_features(&mut self) {
        let Some(MainFocusState::Dependency { cursor, .. }) = &mut self.app.main_focus_state else {
            panic!(
                "Tried to toggle default features but the main focus state was not a dependency."
            )
        };

        let dependency = self
            .app
            .manifest
            .resolve_dependency(cursor)
            .unwrap()
            .unwrap();

        let new_value = !dependency.default_features();

        let mut dependency_item = self.app.manifest.resolve_dependency_mut(cursor).unwrap();

        dependency_item.set_default_features(new_value).unwrap();

        self.app.manifest.save().unwrap();
    }

    /// Unfocus the features list
    #[keybind(pressed(key=KeyCode::Esc))]
    #[keybind(pressed(key=KeyCode::Char('q')))]
    #[tracing::instrument(skip(self))]
    fn close_versions(&mut self) {
        let Some(MainFocusState::Dependency { focused_area, .. }) = &mut self.app.main_focus_state
        else {
            panic!(
                "Tried to close the features list but the main focus state was not a dependency."
            )
        };

        *focused_area = FocusedArea::Main;
    }

    /// Next feature
    #[keybind(pressed(key=KeyCode::Down))]
    #[keybind(pressed(key=KeyCode::Char('j')))]
    fn next_feature(&mut self) {
        let Some(MainFocusState::Dependency {
            focused_area,
            features,
            ..
        }) = &mut self.app.main_focus_state
        else {
            panic!("Tried to select next feature but the main focus state was not a dependency.")
        };

        let FocusedArea::Features { selected } = focused_area else {
            panic!("Tried to select next feature but the focused area was not features.")
        };

        if let Some(Ok(features)) = features {
            let next = (*selected + 1) % features.len();
            *selected = next;
        }
    }

    /// Previous feature
    #[keybind(pressed(key=KeyCode::Up))]
    #[keybind(pressed(key=KeyCode::Char('k')))]
    fn previous_feature(&mut self) {
        let Some(MainFocusState::Dependency {
            focused_area,
            features,
            ..
        }) = &mut self.app.main_focus_state
        else {
            panic!(
                "Tried to select previous feature but the main focus state was not a dependency."
            )
        };

        let FocusedArea::Features { selected } = focused_area else {
            panic!("Tried to select previous feature but the focused area was not features.")
        };

        if let Some(Ok(features)) = features {
            let next = (*selected + features.len() - 1) % features.len();
            *selected = next;
        }
    }

    /// Toggle the currently selected feature
    #[keybind(pressed(key=KeyCode::Enter))]
    fn toggle_selected_feature(&mut self) {
        let Some(MainFocusState::Dependency {
            cursor,
            focused_area,
            features,
            ..
        }) = &self.app.main_focus_state
        else {
            panic!(
                "Tried to toggle selected feature but the main focus state was not a dependency."
            )
        };

        let FocusedArea::Features { selected } = focused_area else {
            panic!("Tried to toggle selected feature but the focused area was not features.")
        };

        if let Some(Ok(features)) = features {
            let Some(feature) = features
                .keys()
                .filter(|name| **name != "default")
                .nth(*selected)
            else {
                panic!("Tried to toggle selected feature but the selected index was out of bounds.")
            };

            let is_enabled = self
                .app
                .manifest
                .resolve_dependency(cursor)
                .unwrap()
                .unwrap()
                .features()
                .is_some_and(|mut f| f.any(|f| f == feature));

            let mut dependency_item = self.app.manifest.resolve_dependency_mut(cursor).unwrap();

            if is_enabled {
                dependency_item.remove_feature(feature).unwrap();
            } else {
                dependency_item.add_feature(feature).unwrap();
            }

            self.app.manifest.save().unwrap();
        }
    }
}

pub struct DependencyUI<'a> {
    pub app: &'a App,
    pub cursor: &'a DependencyCursor,
    pub dependency: &'a DependencyRef<'a>,
    pub versions: Option<Result<&'a Vec<VersionType>, &'a Report<CheckForUpdateError>>>,
    pub features: Option<Result<FeaturesGraph<'a>, &'a Report<FetchFeaturesError>>>,
    pub focused_area: &'a FocusedArea,
}

impl DependencyUI<'_> {
    fn main_area_focused(&self) -> bool {
        matches!(self.focused_area, FocusedArea::Main)
            && matches!(self.app.focused_area(), app::FocusedArea::Main)
    }

    #[tracing::instrument(level = "debug", skip(self, area, buf))]
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let is_focused = matches!(self.app.focused_area(), app::FocusedArea::Main);

        let border_style = if is_focused {
            FOCUSED_BORDER_STYLE
        } else {
            Style::default()
        };

        let update_status = self.app.update_cache.get(self.cursor);

        let update_status_icon = update_status.map_or("", |status| status.icon().get_icon());

        let main = Block::bordered()
            .padding(Padding::horizontal(1))
            .title_top(format!(
                " {} - {} {} {} ",
                self.cursor.table,
                self.dependency.name(),
                self.dependency.source(),
                update_status_icon
            ))
            .title_alignment(HorizontalAlignment::Center)
            .border_style(border_style)
            .border_type(BorderType::Rounded)
            .style(Style::default().add_modifier(Modifier::BOLD));

        let main_area = main.inner(area);
        main.render(area, buf);

        let [versions_area, features_area] = vertical![== 1/2, == 1/2].areas(main_area);

        self.render_dependency_version_section(versions_area, buf);

        self.render_dependency_features_section(features_area, buf);
    }

    fn render_dependency_version_section(&self, area: Rect, buf: &mut Buffer) {
        let is_focused = matches!(self.focused_area, FocusedArea::Versions { .. });

        let border_style = if is_focused {
            FOCUSED_BORDER_STYLE
        } else {
            Style::default()
        };

        let title = if self.main_area_focused() {
            Line::from_iter([
                Span::styled("V", Style::default().bold().white()),
                Span::raw("ersions"),
            ])
        } else {
            "Versions".into()
        };

        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .padding(Padding::horizontal(1))
            .title(title);

        if let Some(versions) = self.versions {
            match versions {
                Ok(versions) => {
                    let current_version = self.dependency.source();

                    let mut list_state = ListState::default();

                    if let FocusedArea::Versions { selected } = *self.focused_area {
                        list_state.select(Some(selected));
                    }

                    let list = versions.iter().map(|version| {
                        let status = self.app.update_cache.get(self.cursor);

                        let is_current = version == current_version;

                        let update = status.is_some_and(|status| match status {
                            // Checks if the version is in the update cache (i.e. it's one of the main update targets)
                            // Genuinely cooked code but idk anything cleaner sob
                            UpdateStatus::UpdateAvailable(update_result) => {
                                matches!(
                                    &update_result.latest_version,
                                    Some(v) if v == version
                                ) | matches!(
                                    &update_result.semantic_latest_version,
                                    Some(v) if v == version
                                )
                            }
                            _ => false,
                        });

                        let spans = [
                            Some(Span::raw(version.to_string())),
                            Some(Span::raw(" ")),
                            is_current.then(|| {
                                Span::styled(
                                    Icons::Done.get_icon(),
                                    Style::default().fg(Color::Green),
                                )
                            }),
                            update.then(|| {
                                Span::styled(
                                    Icons::Update.get_icon(),
                                    Style::default().fg(Color::Yellow),
                                )
                            }),
                        ]
                        .into_iter()
                        .flatten();

                        spans.collect::<Line>()
                    });

                    StatefulWidget::render(
                        List::new(list).block(block).highlight_style(
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .on_white()
                                .black(),
                        ),
                        area,
                        buf,
                        &mut list_state,
                    );
                }
                Err(error) => Paragraph::new(format!("Failed to check for updates! {error:?}"))
                    .block(block)
                    .render(area, buf),
            }
        } else {
            Paragraph::new("Loading")
                .centered()
                .block(block)
                .render(area, buf);
        }
    }

    fn render_dependency_features_section(&self, area: Rect, buf: &mut Buffer) {
        let is_focused = matches!(self.focused_area, FocusedArea::Features { .. });

        let border_style = if is_focused {
            FOCUSED_BORDER_STYLE
        } else {
            Style::default()
        };

        let title = if self.main_area_focused() {
            Line::from_iter([
                Span::styled("F", Style::default().bold().white()),
                Span::raw("eatures"),
            ])
        } else {
            "Features".into()
        };

        let default_features_line = Line::from_iter([
            Span::styled(
                " D",
                if is_focused || self.main_area_focused() {
                    Style::default().bold().white()
                } else {
                    Style::default()
                },
            ),
            Span::raw("efault Features - "),
            if self.dependency.default_features() {
                Span::styled("ON ", Style::default().green())
            } else {
                Span::styled("OFF ", Style::default().red())
            },
        ])
        .alignment(HorizontalAlignment::Right);

        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .padding(Padding::horizontal(1))
            .title_top(title)
            .title_top(default_features_line);

        if let Some(features) = &self.features {
            match features {
                Ok(features) => {
                    let enabled_features = self
                        .dependency
                        .features()
                        .map_or_else(HashSet::new, Iterator::collect::<HashSet<_>>);

                    let mut list_state = ListState::default();

                    if let FocusedArea::Features { selected } = *self.focused_area {
                        list_state.select(Some(selected));
                    }

                    let list = features
                        .iter()
                        .filter(|(name, _)| **name != "default")
                        .map(|(name, implied_chain)| {
                            let enabled = enabled_features.contains(name);

                            let line_parts = [
                                "[".into(),
                                if enabled {
                                    Span::styled(Icons::Done.get_icon(), Style::default().green())
                                } else if !implied_chain.is_empty() {
                                    Span::styled("*", Style::default().yellow())
                                } else {
                                    " ".into()
                                },
                                "] ".into(),
                                (*name).into(),
                                " ".into(),
                                Span::styled(implied_chain.join(" -> "), Modifier::DIM),
                            ];

                            let line = Line::from_iter(line_parts);

                            ListItem::new(line)
                        })
                        .collect::<Vec<ListItem>>();

                    StatefulWidget::render(
                        List::new(list).block(block).highlight_style(
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .on_white()
                                .black(),
                        ),
                        area,
                        buf,
                        &mut list_state,
                    );
                }
                Err(error) => Paragraph::new(
                    format!("Failed to get features list! {error:?}")
                        .to_text()
                        .unwrap(),
                )
                .block(block)
                .render(area, buf),
            }
        } else {
            Paragraph::new("Loading")
                .centered()
                .block(block)
                .render(area, buf);
        }
    }
}
