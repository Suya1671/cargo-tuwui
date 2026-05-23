pub mod dependency;

use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Paragraph},
};

use crate::{
    App,
    app::{FocusedArea, MainFocusState},
    features::create_implied_graph,
    ui::{FOCUSED_BORDER_STYLE, main::dependency::DependencyUI},
};

impl App {
    #[tracing::instrument(level = "debug", skip(self, area, buf), fields(main_focus_state = ?self.main_focus_state))]
    pub fn render_main_area(&self, area: Rect, buf: &mut Buffer) {
        match &self.main_focus_state {
            Some(MainFocusState::Dependency {
                cursor,
                versions,
                features,
                focused_area,
            }) => {
                let dependency = self.manifest.resolve_dependency(cursor).unwrap().unwrap();

                let enabled_features = dependency.features();

                let features = features.as_ref().map(|features| {
                    features.as_ref().map(|features| {
                        enabled_features.map_or_else(
                            || create_implied_graph(features, [], dependency.default_features()),
                            |enabled_features| {
                                create_implied_graph(
                                    features,
                                    enabled_features,
                                    dependency.default_features(),
                                )
                            },
                        )
                    })
                });

                let ui = DependencyUI {
                    app: self,
                    cursor,
                    dependency: &dependency,
                    versions: versions.as_ref().map(Result::as_ref),
                    features,
                    focused_area,
                };

                ui.render(area, buf);
            }
            None => self.empty_main(area, buf),
        }
    }

    #[tracing::instrument(level = "debug", skip(self, area, buf))]
    fn empty_main(&self, area: Rect, buf: &mut Buffer) {
        let is_focused = matches!(self.focused_area(), FocusedArea::Main);

        let border_style = if is_focused {
            FOCUSED_BORDER_STYLE
        } else {
            Style::default()
        };

        let main = Block::bordered()
            .title_top("Cargo tUWUi")
            .title_alignment(HorizontalAlignment::Center)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .style(Style::default().add_modifier(Modifier::BOLD));

        let main_area = main.inner(area);
        main.render(area, buf);

        let text = "Haii select smth from the sidebar :3";

        let main_text = Paragraph::new(text)
            .style(Style::default())
            .alignment(Alignment::Center);

        main_text.render(main_area, buf);
    }
}
