use crossterm::event::{KeyCode, KeyModifiers};
use ratatui_input_manager::{CrosstermBackend, KeyBind, KeyMap, keymap};

use crate::{
    app::{App, FocusedArea, MainFocusState},
    ui::{
        self,
        main::dependency::{self},
        modals::{ModalState, help::HelpModal, update_dependency::UpdateDependencyModalKeybinds},
        sidebar::SidebarKeybinds,
    },
};

impl App {
    pub fn get_area_subkeybinds(&self) -> &'static [KeyBind<CrosstermBackend>] {
        match self.focused_area() {
            FocusedArea::Sidebar => SidebarKeybinds::KEYBINDS,
            FocusedArea::Modal(modal) => modal.get_area_subkeybinds(),
            FocusedArea::Main => match self.main_focus_state {
                None => &[],
                Some(MainFocusState::Dependency {
                    focused_area: dependency::FocusedArea::Main,
                    ..
                }) => ui::main::dependency::DependencyKeybinds::KEYBINDS,
                Some(MainFocusState::Dependency {
                    focused_area: dependency::FocusedArea::Features { .. },
                    ..
                }) => ui::main::dependency::FeaturesListKeybinds::KEYBINDS,
                Some(MainFocusState::Dependency {
                    focused_area: dependency::FocusedArea::Versions { .. },
                    ..
                }) => ui::main::dependency::VersionListKeybinds::KEYBINDS,
            },
        }
    }

    pub fn handle_subkeybind(&mut self, key: &crossterm::event::Event) -> bool {
        match self.focused_area() {
            FocusedArea::Sidebar => SidebarKeybinds::new(self).handle(key),
            FocusedArea::Modal(ModalState::UpdateDependency(_)) => {
                UpdateDependencyModalKeybinds::new(self).handle(key)
            }
            FocusedArea::Main => match self.main_focus_state {
                None => false,
                Some(MainFocusState::Dependency {
                    focused_area: dependency::FocusedArea::Main,
                    ..
                }) => ui::main::dependency::DependencyKeybinds::new(self).handle(key),
                Some(MainFocusState::Dependency {
                    focused_area: dependency::FocusedArea::Features { .. },
                    ..
                }) => ui::main::dependency::FeaturesListKeybinds::new(self).handle(key),
                Some(MainFocusState::Dependency {
                    focused_area: dependency::FocusedArea::Versions { .. },
                    ..
                }) => ui::main::dependency::VersionListKeybinds::new(self).handle(key),
            },
            FocusedArea::Modal(ModalState::Help(_) | ModalState::DependencyUpdateError(_)) => false,
        }
    }
}

#[keymap(backend = "crossterm")]
/// Global keybinds for the application.
impl App {
    /// Open the help menu
    #[keybind(pressed(key=KeyCode::Char('?')))]
    pub fn help(&mut self) {
        self.focus_stack
            .push(FocusedArea::Modal(ModalState::Help(HelpModal::new(
                self.get_area_subkeybinds(),
            ))));
    }

    /// Close the current menu or application
    #[keybind(pressed(key=KeyCode::Esc))]
    #[keybind(pressed(key=KeyCode::Char('q')))]
    #[tracing::instrument(skip(self), fields(focused_area = ?self.focused_area()))]
    pub fn close(&mut self) {
        self.focus_stack.pop();
        if self.focus_stack.is_empty() {
            self.running = false;
        }
    }

    /// Quit the application.
    #[keybind(pressed(key=KeyCode::Char('c'), modifiers=KeyModifiers::CONTROL))]
    pub const fn quit(&mut self) {
        self.running = false;
    }
}
