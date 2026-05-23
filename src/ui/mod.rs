pub mod help;
pub mod icons;
pub mod main;
pub mod modals;
pub mod sidebar;

use ratatui::{
    macros::{horizontal, vertical},
    prelude::*,
};

use crate::{
    app::{App, FocusedArea},
    ui::sidebar::SidebarUI,
};

pub const FOCUSED_BORDER_STYLE: Style = Style::new().add_modifier(Modifier::BOLD).fg(Color::White);

pub struct AppUI<'a> {
    pub state: &'a App,
}

impl Widget for AppUI<'_> {
    #[tracing::instrument(level = "debug", skip(self, area, buf), fields(focused_area = ?self.state.focus_stack.last()))]
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [content_area, help_area] = vertical![>= 0, == 1].areas(area);

        let [sidebar_area, main_area] = horizontal![== 50, >= 0].areas(content_area);

        let sidebar = SidebarUI::from_app_state(self.state, self.state.sidebar_rows());

        sidebar.render(sidebar_area, buf);
        self.state.render_main_area(main_area, buf);
        self.state.render_help_area(help_area, buf);

        if let FocusedArea::Modal(modal_state) = &self.state.focused_area() {
            let modal_area = App::prepare_modal_area(content_area, buf);

            modal_state.render(modal_area, buf, self.state);
        }
    }
}
