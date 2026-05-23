use std::iter;

use crossterm::event::KeyCode;
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, List, ListItem, ListState},
};
use ratatui_input_manager::keymap;
use tracing::info;

use crate::{
    app::{App, FocusedArea, MainFocusState},
    event::AppEvent,
    manifest::{
        dependency::{DependencyCursor, DependencyTableCursor},
        package::PackageInfo,
    },
    ui::{
        FOCUSED_BORDER_STYLE,
        main::dependency,
        modals::{ModalState, update_dependency::UpdateDependencyModal},
    },
    updater::{DependencyUpdateError, UpdateStatus},
};

/// A row in the sidebar
pub enum SidebarRow<'a> {
    DependencyTableHeader {
        cursor: DependencyTableCursor,
        collapsed: bool,
    },
    Dependency {
        cursor: DependencyCursor,
        version: String,
        update_status: &'a UpdateStatus,
    },
}

impl<'a> SidebarRow<'a> {
    fn into_list_item(self) -> ListItem<'a> {
        match self {
            SidebarRow::DependencyTableHeader {
                collapsed,
                cursor: DependencyTableCursor { kind, target },
                ..
            } => {
                let arrow = if collapsed { "▶" } else { "▼" };

                target
                    .as_ref()
                    .map_or_else(
                        || format!(" {arrow} {kind}"),
                        |target| format!(" {arrow} {kind} ({target})"),
                    )
                    .into()
            }
            SidebarRow::Dependency {
                cursor: DependencyCursor { name, .. },
                update_status,
                version,
                ..
            } => [
                "    ".into(),
                update_status.as_span(),
                " ".into(),
                name.into(),
                " ".into(),
                version.into(),
            ]
            .into_iter()
            .collect::<Line>()
            .into(),
        }
    }

    fn matches(&self, focus: &SidebarFocusState) -> bool {
        match (self, focus) {
            (
                SidebarRow::DependencyTableHeader { cursor, .. },
                SidebarFocusState::DependencyTable(c),
            ) => cursor == c,
            (SidebarRow::Dependency { cursor, .. }, SidebarFocusState::Dependency(c)) => {
                cursor == c
            }
            _ => false,
        }
    }
}

impl<'a> From<SidebarRow<'a>> for SidebarFocusState {
    fn from(row: SidebarRow<'a>) -> Self {
        match row {
            SidebarRow::DependencyTableHeader { cursor, .. } => Self::DependencyTable(cursor),
            SidebarRow::Dependency { cursor, .. } => Self::Dependency(cursor),
        }
    }
}

// this is explicit rather than being indexed-based so potential re-ordering shenanigans don't jump the cursor around
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarFocusState {
    DependencyTable(DependencyTableCursor),
    Dependency(DependencyCursor),
}

pub struct SidebarKeybinds<'a> {
    state: &'a mut App,
}

impl<'a> SidebarKeybinds<'a> {
    pub const fn new(state: &'a mut App) -> Self {
        Self { state }
    }
}

#[keymap(backend = "crossterm")]
impl SidebarKeybinds<'_> {
    /// Previous Item
    #[keybind(pressed(key=KeyCode::Up))]
    #[keybind(pressed(key=KeyCode::Char('k')))]
    #[tracing::instrument(skip(self))]
    pub fn up(&mut self) {
        let target = self
            .state
            .sidebar_rows()
            .take_while(|r| !r.matches(&self.state.sidebar_focus_state))
            .last()
            .map(SidebarFocusState::from);

        if let Some(focus) = target {
            self.state.sidebar_focus_state = focus;
        }
    }

    /// Next item
    #[keybind(pressed(key=KeyCode::Down))]
    #[keybind(pressed(key=KeyCode::Char('j')))]
    #[tracing::instrument(skip(self))]
    pub fn down(&mut self) {
        let target = self
            .state
            .sidebar_rows()
            .skip_while(|r| !r.matches(&self.state.sidebar_focus_state))
            .nth(1)
            .map(SidebarFocusState::from);

        if let Some(focus) = target {
            self.state.sidebar_focus_state = focus;
        }
    }

    /// Toggle tree / focus selected item
    #[keybind(pressed(key=KeyCode::Enter))]
    #[tracing::instrument(skip(self))]
    pub fn focus_selected(&mut self) {
        match &self.state.sidebar_focus_state {
            SidebarFocusState::DependencyTable(dependency_table_cursor) => {
                self.state.collapsed_sections.insert(
                    dependency_table_cursor.clone(),
                    self.state
                        .collapsed_sections
                        .get(dependency_table_cursor)
                        .copied()
                        .is_none_or(|v| !v),
                );
            }
            SidebarFocusState::Dependency(dependency_cursor) => {
                self.state.main_focus_state = Some(MainFocusState::Dependency {
                    cursor: dependency_cursor.clone(),
                    versions: None,
                    features: None,
                    focused_area: dependency::FocusedArea::Main,
                });

                // todo: extract this into it's own component + updating the main focus state in one
                self.state
                    .events
                    .send(AppEvent::LoadDependencyVersions(dependency_cursor.clone()));

                self.state
                    .events
                    .send(AppEvent::LoadDependencyFeatures(dependency_cursor.clone()));

                self.state.focus_stack.push(FocusedArea::Main);
            }
        }
    }

    /// Update the selected item
    #[keybind(pressed(key=KeyCode::Char('u')))]
    #[tracing::instrument(skip(self))]
    pub fn update(&mut self) {
        match &self.state.sidebar_focus_state {
            SidebarFocusState::DependencyTable(_cursor) => {
                info!("TODO: handle updating full dependency tables");
            }

            // todo: extract to own function
            SidebarFocusState::Dependency(cursor) => {
                let Some(update_info) = self.state.update_cache.get(cursor) else {
                    info!("TODO: queue an check update instead of showing a modal");

                    self.state.focus_stack.push(FocusedArea::Modal(
                        ModalState::DependencyUpdateError(DependencyUpdateError::NoUpdate.into()),
                    ));

                    return;
                };

                match update_info {
                    UpdateStatus::UpdateAvailable(update_result) => {
                        self.state.focus_stack.push(FocusedArea::Modal(
                            ModalState::UpdateDependency(UpdateDependencyModal::new(
                                cursor.clone(),
                                update_result.clone(),
                            )),
                        ));
                    }

                    UpdateStatus::Pending => self.state.focus_stack.push(FocusedArea::Modal(
                        ModalState::DependencyUpdateError(DependencyUpdateError::Pending.into()),
                    )),

                    UpdateStatus::UpToDate => self.state.focus_stack.push(FocusedArea::Modal(
                        ModalState::DependencyUpdateError(DependencyUpdateError::NoUpdate.into()),
                    )),

                    UpdateStatus::Error(_) => {
                        let Some(UpdateStatus::Error(report)) =
                            self.state.update_cache.remove(cursor)
                        else {
                            unreachable!(
                                "Code literally just checked that the update resulted in an error"
                            )
                        };

                        self.state.events.send(AppEvent::UpdateCheck {
                            cursor: cursor.clone(),
                        });

                        self.state.focus_stack.push(FocusedArea::Modal(
                            ModalState::DependencyUpdateError(
                                report.change_context(DependencyUpdateError::UpdateCheckError),
                            ),
                        ));
                    }
                }
            }
        }
    }
}

pub struct SidebarUI<'a, I: Iterator<Item = SidebarRow<'a>>> {
    package: PackageInfo<'a>,
    focused: bool,

    rows: I,
    selected_index: Option<usize>,
}

impl<'a, I: Iterator<Item = SidebarRow<'a>>> Widget for SidebarUI<'a, I> {
    #[tracing::instrument(level = "debug", skip(self, area, buf))]
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            FOCUSED_BORDER_STYLE
        } else {
            Style::default()
        };

        let sidebar = Block::bordered()
            .title_top("Cargo tUWUi")
            .title_alignment(HorizontalAlignment::Center)
            .title_bottom(format!("{} {}", self.package.name, self.package.version))
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .style(Style::default().add_modifier(Modifier::BOLD));

        let sidebar_inner = sidebar.inner(area);
        sidebar.render(area, buf);

        let mut list_state = ListState::default().with_selected(self.selected_index);
        let list_items = self.rows.map(SidebarRow::into_list_item);

        let highlight_style = if self.focused {
            Style::default()
                .add_modifier(Modifier::BOLD)
                .on_white()
                .black()
        } else {
            Style::default()
                .add_modifier(Modifier::BOLD | Modifier::DIM)
                .on_gray()
                .black()
        };

        let deps_list = List::new(list_items).highlight_style(highlight_style);
        StatefulWidget::render(deps_list, sidebar_inner, buf, &mut list_state);
    }
}

impl<'a, I: Iterator<Item = SidebarRow<'a>>> SidebarUI<'a, I> {
    pub fn from_app_state(app: &'a App, rows: I) -> Self {
        let package = app.manifest.package().unwrap();
        let focused = matches!(app.focused_area(), FocusedArea::Sidebar);
        let selected_index = app.sidebar_selected_index();

        Self {
            package,
            focused,
            rows,
            selected_index,
        }
    }
}

impl App {
    #[tracing::instrument(level = "debug", skip(self), fields(focus_state = ?self.sidebar_focus_state, rows = ?self.manifest.dependencies().unwrap().len()))]
    pub fn sidebar_rows(&self) -> impl Iterator<Item = SidebarRow<'_>> + '_ {
        let deps = self.manifest.dependencies().unwrap();

        deps.into_iter().flat_map(move |table| {
            let collapsed = self
                .collapsed_sections
                .get(&table.cursor)
                .copied()
                .unwrap_or(false);

            let header_row = SidebarRow::DependencyTableHeader {
                cursor: table.cursor.clone(),
                collapsed,
            };

            let dependencies = table
                .deps
                .into_iter()
                .filter(move |_| !collapsed) // Don't show collapsed dependencies
                .map(move |dep| {
                    let cursor = DependencyCursor {
                        table: table.cursor.clone(),
                        name: dep.name().to_owned(),
                    };

                    SidebarRow::Dependency {
                        version: dep.source().to_string(),
                        update_status: self
                            .update_cache
                            .get(&cursor)
                            .unwrap_or(&UpdateStatus::Pending),
                        cursor,
                    }
                });

            iter::once(header_row).chain(dependencies)
        })
    }

    /// Returns the index of the selected item in the sidebar, if any.
    #[tracing::instrument(level = "debug", skip(self), fields(focus_state = ?self.sidebar_focus_state))]
    pub fn sidebar_selected_index(&self) -> Option<usize> {
        let mut rows = self.sidebar_rows();
        rows.position(|row| row.matches(&self.sidebar_focus_state))
    }
}
