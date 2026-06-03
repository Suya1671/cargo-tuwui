#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]

use displaydoc::Display;
use error_stack::{FutureExt, Report, ResultExt};
use thiserror::Error;

use crate::{app::App, terminal::TerminalGuard};

mod app;
mod config;
mod errors;
mod event;
mod features;
mod keybinds;
mod logging;
mod manifest;
mod terminal;
mod ui;
mod updater;

#[derive(Debug, Error, Display)]
/// An error occurred while running the application
pub enum Error {
    /// An error occurred while initializing the application.
    Init,
    /// An error occurred while running the TUI.
    Run,
}

#[tokio::main]
async fn main() -> Result<(), Report<Error>> {
    errors::init();
    let config = config::Config::new()?;

    logging::init(&config)?;

    let mut terminal = TerminalGuard::new();

    let current_dir = std::env::current_dir()
        .attach("Failed to get current directory")
        .change_context(Error::Init)?;

    let manifest_path = current_dir.join("Cargo.toml");

    let app = App::new(manifest_path)
        .attach("Failed to initialize application")
        .change_context(Error::Init)?;

    app.queue_check_for_updates()
        .attach("Failed to queue check for updates")
        .change_context(Error::Init)?;

    app.run(&mut terminal).change_context(Error::Run).await
}
