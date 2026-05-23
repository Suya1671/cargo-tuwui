// taken from ratatui_input_manager::widgets::Help
// But modified to use Vectors of references instead of slices
use itertools::Itertools;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, HorizontalAlignment, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Block, Cell, Row, Table, Widget},
};
use ratatui_input_manager::{Backend, KeyBind, KeyMap, KeyPress};

use crate::app::App;

impl App {
    pub fn render_help_area(&self, area: Rect, buffer: &mut Buffer) {
        let keybinds = self
            .get_area_subkeybinds()
            .iter()
            .chain(Self::KEYBINDS.iter())
            .collect::<Vec<_>>();

        let help = HelpBar::new(keybinds);
        help.render(area, buffer);
    }
}

pub struct Help<'k, B: Backend + 'static> {
    keybinds: Vec<&'k KeyBind<B>>,
    block: Option<Block<'k>>,
    style: Style,
    key_style: Style,
    description_style: Style,
}

impl<'k, B: Backend> Help<'k, B> {
    /// Construct a [`Help`] [`Widget`] from a collection of [`KeyBind`]s, typically obtained by
    /// inspecting the metadata as [`crate::KeyMap::KEYBINDS`] or
    /// [`crate::DynKeyMap::keybinds`]
    pub fn new(keybinds: impl IntoIterator<Item = &'k KeyBind<B>>) -> Self {
        Self {
            keybinds: keybinds.into_iter().collect(),
            block: None,
            style: Style::default(),
            key_style: Style::default(),
            description_style: Style::default(),
        }
    }

    /// Wraps the help table with the given [`Block`]
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn block(mut self, block: Block<'k>) -> Self {
        self.block = Some(block);
        self
    }
}

impl<B: Backend + 'static> Widget for Help<'_, B>
where
    KeyPress<B>: std::fmt::Display,
{
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        #[allow(clippy::cast_possible_truncation)]
        let max_key_width = self
            .keybinds
            .iter()
            .map(|kb| {
                let key_lens: Vec<usize> = kb.pressed.iter().map(|p| p.to_string().len()).collect();
                key_lens.iter().sum::<usize>() + key_lens.len().saturating_sub(1) * 2
            })
            .max()
            .unwrap_or_default()
            .max(3) as u16;

        let table = Table::new(
            self.keybinds.iter().map(
                |KeyBind {
                     pressed,
                     description,
                     ..
                 }| {
                    let key_str = pressed
                        .iter()
                        .map(ToString::to_string)
                        .format(", ")
                        .to_string();
                    let mut text = Text::from(key_str);
                    text.alignment = Some(HorizontalAlignment::Right);
                    Row::new([
                        Cell::new(text).style(self.key_style),
                        Cell::new(*description).style(self.description_style),
                    ])
                },
            ),
            [Constraint::Length(max_key_width), Constraint::Fill(1)],
        )
        .style(self.style);

        let area = self.block.map_or(area, |block| {
            block.clone().render(area, buf);
            block.inner(area)
        });

        table.render(area, buf);
    }
}

/// A [`Widget`] displaying a single row of bound keys and their descriptions
///
/// # Example
///
/// ```rust
/// use crossterm::event::KeyCode;
/// use ratatui_core::{
///     style::{Color, Style},
///     terminal::Frame,
/// };
/// use ratatui_input_manager::{CrosstermBackend, KeyMap, keymap};
/// use ratatui_input_manager::widgets::HelpBar;
///
/// #[derive(Default)]
/// struct App;
///
/// #[keymap(backend = "crossterm")]
/// impl App {
///     /// Quit the application
///     #[keybind(pressed(key = KeyCode::Char('q')))]
///     fn quit(&mut self) {}
/// }
///
/// fn render_help_bar(frame: &mut Frame) {
///     let help = HelpBar::<CrosstermBackend>::new(App::KEYBINDS)
///         .key_style(Style::default().fg(Color::Cyan))
///         .separator_style(Style::default().fg(Color::DarkGray));
///     frame.render_widget(help, frame.area());
/// }
/// ```
pub struct HelpBar<'k, B: Backend + 'static> {
    keybinds: Vec<&'k KeyBind<B>>,
    style: Style,
    key_style: Style,
    description_style: Style,
    separator_style: Style,
}

impl<'k, B: Backend> HelpBar<'k, B> {
    /// Construct a [`HelpBar`] [`Widget`] from a collection of [`KeyBind`]s, typically obtained by
    /// inspecting the metadata as [`crate::KeyMap::KEYBINDS`] or
    /// [`crate::DynKeyMap::keybinds`]
    pub fn new(keybinds: impl IntoIterator<Item = &'k KeyBind<B>>) -> Self {
        Self {
            keybinds: keybinds.into_iter().collect(),
            style: Style::default(),
            key_style: Style::default(),
            description_style: Style::default(),
            separator_style: Style::default(),
        }
    }
}

impl<B: Backend + 'static> Widget for HelpBar<'_, B>
where
    KeyPress<B>: std::fmt::Display,
{
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.keybinds
            .iter()
            .enumerate()
            .flat_map(
                |(
                    idx,
                    KeyBind {
                        pressed,
                        description,
                        ..
                    },
                )| {
                    [
                        Span::styled(if idx == 0 { "" } else { " | " }, self.separator_style),
                        Span::styled(format!("{description}: "), self.description_style),
                        Span::styled(
                            pressed
                                .iter()
                                .map(ToString::to_string)
                                .format(", ")
                                .to_string(),
                            self.key_style,
                        ),
                    ]
                },
            )
            .collect::<Line>()
            .style(self.style)
            .render(area, buf);
    }
}
