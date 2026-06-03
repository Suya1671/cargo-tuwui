use ratatui::DefaultTerminal;

/// A terminal guard that initializes the terminal when created and restores it when dropped.
pub struct TerminalGuard {
    terminal: DefaultTerminal,
}

impl TerminalGuard {
    pub fn new() -> Self {
        let terminal = ratatui::init();
        Self { terminal }
    }
}

impl std::ops::Deref for TerminalGuard {
    type Target = DefaultTerminal;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl std::ops::DerefMut for TerminalGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}
