use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, HorizontalAlignment, Layout},
    macros::vertical,
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Paragraph, Widget},
};
use ratatui_input_manager::keymap;

use crate::{
    app::{App, FocusedArea},
    event::AppEvent,
    manifest::dependency::DependencyCursor,
    ui::{
        FOCUSED_BORDER_STYLE,
        modals::{ModalState, SelectionType},
    },
    updater::UpdateResult,
};

pub struct UpdateDependencyModalKeybinds<'a> {
    app: &'a mut App,
}

impl<'a> UpdateDependencyModalKeybinds<'a> {
    pub const fn new(app: &'a mut App) -> Self {
        Self { app }
    }

    fn get_state(&self) -> &UpdateDependencyModal {
        self.app
            .focus_stack
            .iter()
            .rev()
            .find_map(|area| {
                if let FocusedArea::Modal(ModalState::UpdateDependency(modal)) = area {
                    Some(modal)
                } else {
                    None
                }
            })
            .unwrap()
    }

    fn get_state_mut(&mut self) -> &mut UpdateDependencyModal {
        self.app
            .focus_stack
            .iter_mut()
            .rev()
            .find_map(|area| {
                if let FocusedArea::Modal(ModalState::UpdateDependency(modal)) = area {
                    Some(modal)
                } else {
                    None
                }
            })
            .unwrap()
    }
}

#[keymap(backend = "crossterm")]
impl UpdateDependencyModalKeybinds<'_> {
    /// Move the cursor left
    #[keybind(pressed(key=KeyCode::Left))]
    #[keybind(pressed(key=KeyCode::Char('h')))]
    #[tracing::instrument(skip(self))]
    #[allow(clippy::match_same_arms)]
    pub fn left(&mut self) {
        let state = self.get_state_mut();
        let semver_exists = state.result.semantic_latest_version.is_some();
        let latest_exists = state.result.latest_version.is_some();

        // order: cancel, semver, latest
        state.selected = match (state.selected, semver_exists, latest_exists) {
            (SelectionType::Cancel, _, true) => SelectionType::Latest,
            (SelectionType::Cancel, true, _) => SelectionType::Cancel,
            (SelectionType::Cancel, _, _) => SelectionType::Cancel,

            (SelectionType::SemverCompatible, _, _) => SelectionType::Cancel,

            (SelectionType::Latest, true, _) => SelectionType::SemverCompatible,
            (SelectionType::Latest, _, _) => SelectionType::Cancel,
        };
    }

    /// Move the cursor right
    #[keybind(pressed(key=KeyCode::Right))]
    #[keybind(pressed(key=KeyCode::Char('l')))]
    #[tracing::instrument(skip(self))]
    #[allow(clippy::match_same_arms)]
    pub fn right(&mut self) {
        let state = self.get_state_mut();
        let semver_exists = state.result.semantic_latest_version.is_some();
        let latest_exists = state.result.latest_version.is_some();

        // order: cancel, semver, latest
        state.selected = match (state.selected, semver_exists, latest_exists) {
            (SelectionType::Cancel, true, _) => SelectionType::SemverCompatible,
            (SelectionType::Cancel, _, true) => SelectionType::Latest,
            (SelectionType::Cancel, _, _) => SelectionType::Cancel,

            (SelectionType::SemverCompatible, _, true) => SelectionType::Latest,
            (SelectionType::SemverCompatible, _, _) => SelectionType::Cancel,

            (SelectionType::Latest, _, _) => SelectionType::Cancel,
        };
    }

    /// Update/Cancel update
    #[keybind(pressed(key=KeyCode::Enter))]
    #[tracing::instrument(skip(self))]
    pub fn enter(&mut self) {
        let state = self.get_state();

        match state.selected {
            SelectionType::Cancel => self.app.close(),
            SelectionType::SemverCompatible => self.app.events.send(AppEvent::UpdateDependency {
                cursor: state.dependency.clone(),
                version: state
                    .result
                    .semantic_latest_version
                    .as_ref()
                    .expect("semantic_latest_version is None")
                    .clone(),
            }),
            SelectionType::Latest => self.app.events.send(AppEvent::UpdateDependency {
                cursor: state.dependency.clone(),
                version: state
                    .result
                    .latest_version
                    .as_ref()
                    .expect("latest_version is None")
                    .clone(),
            }),
        }
    }
}

#[derive(Debug)]
pub struct UpdateDependencyModal {
    dependency: DependencyCursor,
    selected: SelectionType,
    result: UpdateResult,
}

impl UpdateDependencyModal {
    pub const fn new(dependency: DependencyCursor, result: UpdateResult) -> Self {
        Self {
            dependency,
            selected: SelectionType::Cancel,
            result,
        }
    }

    pub fn render(
        &self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        app: &crate::App,
    ) {
        let dependency = app
            .manifest
            .resolve_dependency(&self.dependency)
            .expect("Failed to resolve dependency")
            .expect("Dependency not found");

        let block = Block::bordered()
            .title(format!("Updating {}", dependency.name()))
            .border_type(BorderType::Rounded)
            .title_alignment(HorizontalAlignment::Center)
            .border_style(FOCUSED_BORDER_STYLE)
            .style(Style::default().add_modifier(Modifier::BOLD));

        let inner_area = block.inner(area);
        block.render(area, buf);

        let [information_area, buttons_area] = vertical![>= 1, == 3].areas(inner_area);

        let selected_version = match self.selected {
            SelectionType::Cancel => "[N/A]".to_string(),
            SelectionType::SemverCompatible => self
                .result
                .semantic_latest_version
                .as_ref()
                .unwrap()
                .to_string(),
            SelectionType::Latest => self.result.latest_version.as_ref().unwrap().to_string(),
        };

        Paragraph::new(format!(
            "Updating from {} to {}",
            dependency.source(),
            selected_version
        ))
        .alignment(HorizontalAlignment::Center)
        .render(information_area, buf);

        let buttons = [
            Some(("Cancel".to_string(), SelectionType::Cancel)),
            self.result
                .semantic_latest_version
                .as_ref()
                .map(|v| (format!("Compatible ({v})"), SelectionType::SemverCompatible)),
            self.result
                .latest_version
                .as_ref()
                .map(|v| (format!("Latest ({v})"), SelectionType::Latest)),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

        let constraints = vec![Constraint::Fill(1); buttons.len()];
        let areas = Layout::horizontal(constraints).split(buttons_area);

        for (area, (label, selection_type)) in areas.iter().zip(buttons) {
            let selected = selection_type == self.selected;

            let block = Block::bordered()
                .border_type(BorderType::Rounded)
                .style(if selected {
                    Style::default().fg(Color::Black).bg(Color::White)
                } else {
                    Style::default()
                });

            Paragraph::new(label)
                .block(block)
                .alignment(HorizontalAlignment::Center)
                .render(*area, buf);
        }
    }
}
