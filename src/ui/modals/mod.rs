pub mod help;
pub mod update_dependency;

use std::fmt::Debug;

use crate::{
    App,
    ui::{
        FOCUSED_BORDER_STYLE,
        modals::{
            help::HelpModal,
            update_dependency::{UpdateDependencyModal, UpdateDependencyModalKeybinds},
        },
    },
    updater::DependencyUpdateError,
};
use ansi_to_tui::IntoText;
use error_stack::Report;
use ratatui::{
    macros::constraint,
    prelude::*,
    widgets::{Block, BorderType, Clear, Paragraph},
};
use ratatui_input_manager::{CrosstermBackend, KeyBind, KeyMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionType {
    #[default]
    Cancel,
    SemverCompatible,
    Latest,
}

pub enum ModalState {
    Help(HelpModal),
    DependencyUpdateError(Report<DependencyUpdateError>),
    UpdateDependency(UpdateDependencyModal),
}

impl Debug for ModalState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Help(_) => f.debug_struct("ModalState::Help").finish(),

            Self::DependencyUpdateError(err) => f
                .debug_tuple("ModalState::DependencyUpdateError")
                .field(err)
                .finish(),

            Self::UpdateDependency(modal) => f
                .debug_tuple("ModalState::UpdateDependency")
                .field(modal)
                .finish(),
        }
    }
}

impl ModalState {
    pub fn render_dependency_update_error_modal(
        area: Rect,
        buf: &mut Buffer,
        error: &Report<DependencyUpdateError>,
    ) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().red())
            .border_style(FOCUSED_BORDER_STYLE);

        match error.current_context() {
            DependencyUpdateError::Pending => {
                // TODO: dynamic keybind rendering whenever I implement that system
                Paragraph::new("Waiting for update...\n\nClose this window with q or <esc>")
                    .block(block)
                    .centered()
                    .render(area, buf);
            }

            DependencyUpdateError::NoUpdate => {
                Paragraph::new("No update available.\n\nClose this window with q or <esc>")
                    .block(block)
                    .centered()
                    .render(area, buf);
            }

            _ => {
                let block = block
                    .title("Dependency Update Error!")
                    .border_style(Style::default().red());

                Paragraph::new(format!("{error:?}").to_text().unwrap())
                    .block(block)
                    .render(area, buf);
            }
        }
    }

    pub const fn get_area_subkeybinds(&self) -> &'static [KeyBind<CrosstermBackend>] {
        match self {
            Self::Help(_) | Self::DependencyUpdateError(_) => &[],
            Self::UpdateDependency(_) => UpdateDependencyModalKeybinds::KEYBINDS,
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, app: &App) {
        match self {
            Self::Help(help_modal) => help_modal.render(area, buf),
            Self::DependencyUpdateError(error) => {
                Self::render_dependency_update_error_modal(area, buf, error);
            }
            Self::UpdateDependency(modal) => {
                modal.render(area, buf, app);
            }
        }
    }
}

impl App {
    pub fn prepare_modal_area(area: Rect, buf: &mut Buffer) -> Rect {
        let modal_area = area.centered(constraint!(== 60%), constraint!(== 60%));
        Clear.render(modal_area, buf);
        modal_area
    }
}
