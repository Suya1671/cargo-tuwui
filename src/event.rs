use crossterm::event::Event as CrosstermEvent;
use displaydoc::Display;
use error_stack::{IntoReport, Report};
use futures::{FutureExt, StreamExt};
use tokio::sync::mpsc;
use tracing::{Instrument, debug, trace};

use crate::{
    features::{FeaturesList, FetchFeaturesError},
    manifest::dependency::DependencyCursor,
    updater::{CheckForUpdateError, UpdateStatus, VersionType},
};

/// Representation of all possible events.
#[derive(Debug)]
pub enum Event {
    /// Crossterm events.
    ///
    /// These events are emitted by the terminal.
    Crossterm(CrosstermEvent),
    /// Application events.
    ///
    /// Use this event to emit custom events that are specific to your application.
    App(AppEvent),
}

/// Application events.
///
/// You can extend this enum with your own custom events.
#[derive(Debug)]
pub enum AppEvent {
    /// Queue an check for a dependency.
    UpdateCheck { cursor: DependencyCursor },

    /// Result of an update check for a dependency.
    UpdateCheckResult {
        cursor: DependencyCursor,
        result: UpdateStatus,
    },

    /// Queue an update for a dependency.
    UpdateDependency {
        cursor: DependencyCursor,
        version: VersionType,
    },

    /// Load the versions of a dependency in the main view.
    LoadDependencyVersions(DependencyCursor),

    /// Load the features of a dependency in the main view.
    LoadDependencyFeatures(DependencyCursor),

    /// Update the versions of a dependency in the main view.
    UpdateDependencyVersions {
        versions: Result<Vec<VersionType>, Report<CheckForUpdateError>>,
    },

    /// Update the features of a dependency in the main view.
    UpdateDependencyFeatures {
        features: Result<FeaturesList, Report<FetchFeaturesError>>,
    },
}

/// Terminal event handler.
#[derive(Debug)]
pub struct EventHandler {
    /// Event sender channel.
    sender: mpsc::UnboundedSender<Event>,
    /// Event receiver channel.
    receiver: mpsc::UnboundedReceiver<Event>,
}

#[derive(Debug, thiserror::Error, Display)]
pub enum Error {
    /// Failed to receive an event from the sender.
    RecvFail,
}

impl EventHandler {
    /// Constructs a new instance of [`EventHandler`] and spawns a new thread to handle events.
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let actor = EventTask::new(sender.clone());
        tokio::spawn(async { actor.run().await }.in_current_span());
        Self { sender, receiver }
    }

    /// Receives an event from the sender.
    ///
    /// This function blocks until an event is received.
    ///
    /// # Errors
    ///
    /// This function returns an error if the sender channel is disconnected. This can happen if an
    /// error occurs in the event thread. In practice, this should not happen unless there is a
    /// problem with the underlying terminal.
    pub async fn next(&mut self) -> Result<Event, Report<Error>> {
        self.receiver
            .recv()
            .await
            .ok_or_else(|| Error::RecvFail.into_report())
    }

    /// Queue an app event to be sent to the event receiver.
    ///
    /// This is useful for sending events to the event handler which will be processed by the next
    /// iteration of the application's event loop.
    pub fn send(&self, app_event: AppEvent) {
        // Ignore the result as the reciever cannot be dropped while this struct still has a
        // reference to it
        let _ = self.sender.send(Event::App(app_event));
    }

    pub const fn sender(&self) -> &mpsc::UnboundedSender<Event> {
        &self.sender
    }
}

/// A thread that handles reading crossterm events.
struct EventTask {
    /// Event sender channel.
    sender: mpsc::UnboundedSender<Event>,
}

impl EventTask {
    /// Constructs a new instance of [`EventTask`].
    const fn new(sender: mpsc::UnboundedSender<Event>) -> Self {
        Self { sender }
    }

    /// Runs the event thread.
    ///
    /// This function polls for crossterm events in between.
    #[tracing::instrument(skip(self))]
    async fn run(self) {
        let mut reader = crossterm::event::EventStream::new();

        loop {
            let crossterm_event = reader.next().fuse();
            tokio::select! {
              () = self.sender.closed() => {
                debug!("Loop terminated");
                break;
              }
              Some(Ok(evt)) = crossterm_event => {
                trace!(evt = ?evt, "New crossterm event");
                self.send(Event::Crossterm(evt));
              }
            };
        }
    }

    /// Sends an event to the receiver.
    fn send(&self, event: Event) {
        // Ignores the result because shutting down the app drops the receiver, which causes the send
        // operation to fail. This is expected behavior and should not panic.
        let _ = self.sender.send(event);
    }
}
