use ratatui::{
    layout::HorizontalAlignment,
    style::{Modifier, Style},
    widgets::{Block, BorderType, Widget},
};
use ratatui_input_manager::{CrosstermBackend, KeyBind, KeyMap};

use crate::{
    app::App,
    ui::{FOCUSED_BORDER_STYLE, help::Help},
};

pub struct HelpModal {
    area_binds: &'static [KeyBind<CrosstermBackend>],
}

impl HelpModal {
    pub const fn new(area_binds: &'static [KeyBind<CrosstermBackend>]) -> Self {
        Self { area_binds }
    }

    pub fn render(&self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let all_keybinds = self.area_binds.iter().chain(App::KEYBINDS.iter());

        Help::new(all_keybinds)
            .block(
                Block::bordered()
                    .title("Keybindings")
                    .border_type(BorderType::Rounded)
                    .title_alignment(HorizontalAlignment::Center)
                    .border_style(FOCUSED_BORDER_STYLE)
                    .style(Style::default().add_modifier(Modifier::BOLD)),
            )
            .render(area, buf);
    }
}
