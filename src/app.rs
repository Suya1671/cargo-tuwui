use std::fmt::Debug;
use std::{collections::HashMap, path::PathBuf};

use crate::features::{FeaturesList, FetchFeaturesError, fetch_features};
use crate::ui::sidebar::SidebarFocusState;
use crate::ui::{self, AppUI};
use crate::updater::{CheckForUpdateError, UpdateCache};
use crate::{
    event::{AppEvent, Event, EventHandler},
    fields,
    manifest::{
        Manifest,
        dependency::{DependencyCursor, DependencyTableCursor, Source},
    },
    ui::modals::ModalState,
    updater::{DependencyUpdateError, UpdateResult, UpdateStatus, VersionType, registry},
};
use displaydoc::Display;
use error_stack::{IntoReport, Report, ResultExt};
use ratatui::DefaultTerminal;
use ratatui_input_manager::KeyMap;
use thiserror::Error;
use tracing::{Instrument, debug, error, trace, warn};

#[derive(Debug)]
pub enum MainFocusState {
    // default features and enabled features can be fetched from the manifest
    Dependency {
        cursor: DependencyCursor,
        versions: Option<Result<Vec<VersionType>, Report<CheckForUpdateError>>>,
        features: Option<Result<FeaturesList, Report<FetchFeaturesError>>>,
        focused_area: ui::main::dependency::FocusedArea,
    },
}

#[derive(Debug)]
pub enum FocusedArea {
    Sidebar,
    Main,
    // Since modals are always on top, they should always be in focus.
    // Thus, it makes sense to just put their state in their focused definition
    Modal(ModalState),
}

/// Application.
#[derive(Debug)]
pub struct App {
    /// Is the application running?
    pub running: bool,
    /// Event handler.
    pub events: EventHandler,
    /// The manifest file.
    pub manifest: Manifest,
    /// The collapsed state of each dependency table section.
    pub collapsed_sections: HashMap<DependencyTableCursor, bool>,
    /// The current focus state of the sidebar.
    pub sidebar_focus_state: SidebarFocusState,
    /// The current focus state of the main content area.
    pub main_focus_state: Option<MainFocusState>,

    /// Update check results for each dependency.
    pub update_cache: UpdateCache,

    /// The HTTP client used for making requests.
    pub client: reqwest::Client,
    /// The stack of current and previously focused areas.
    pub focus_stack: Vec<FocusedArea>,
}

/// Errors that can occur during application execution.
#[derive(Debug, Error, Display)]
pub enum AppError {
    /// Failed to render the terminal
    Render,
    /// Failed to handle an event
    Event,
}

#[derive(Debug, Error, Display)]
/// Errors that can occur during application construction.
pub enum AppConstructionError {
    /// Failed to load the manifest file
    Manifest,
    /// Failed to load the dependencies
    Dependencies,
    /// Failed to create the HTTP client
    Client,
}

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

impl App {
    /// Constructs a new instance of [`App`].
    pub fn new(manifest_path: PathBuf) -> Result<Self, Report<AppConstructionError>> {
        let manifest =
            Manifest::new(manifest_path).change_context(AppConstructionError::Manifest)?;
        let dependencies = manifest
            .dependencies()
            .change_context(AppConstructionError::Dependencies)?;

        Ok(Self {
            running: true,
            events: EventHandler::new(),
            sidebar_focus_state: SidebarFocusState::DependencyTable(
                dependencies
                    .first()
                    .ok_or_else(|| {
                        AppConstructionError::Dependencies
                            .into_report()
                            .attach("Could not find any dependencies")
                    })?
                    .cursor
                    .clone(),
            ),
            focus_stack: vec![FocusedArea::Sidebar],
            collapsed_sections: HashMap::new(),
            update_cache: HashMap::new(),
            main_focus_state: None,
            manifest,
            client: reqwest::Client::builder()
                .user_agent(APP_USER_AGENT)
                .build()
                .change_context(AppConstructionError::Client)?,
        })
    }

    /// Run the application's main loop.
    pub async fn run(mut self, terminal: &mut DefaultTerminal) -> Result<(), Report<AppError>> {
        while self.running {
            terminal
                .draw(|frame| frame.render_widget(self.create_widget(), frame.area()))
                .change_context(AppError::Render)?;

            let event = self.events.next().await.change_context(AppError::Event)?;

            trace!(event = ?event, "Handling event");

            match event {
                Event::Crossterm(event) => {
                    if !self.handle_subkeybind(&event) {
                        self.handle(&event);
                    }
                }
                Event::App(app_event) => match app_event {
                    AppEvent::UpdateCheck { cursor } => self.check_for_update(cursor),
                    AppEvent::UpdateCheckResult { cursor, result } => {
                        self.update_cache.insert(cursor, result);
                    }
                    AppEvent::UpdateDependency { cursor, version } => {
                        self.update_dependency(&cursor, &version)
                            .change_context(AppError::Event)?;

                        // queue up a recheck
                        self.check_for_update(cursor);

                        self.close();
                    }
                    AppEvent::LoadDependencyVersions(dependency_cursor) => {
                        self.load_dependency_versions(&dependency_cursor);
                    }
                    AppEvent::UpdateDependencyVersions {
                        versions: new_versions,
                    } => {
                        if let Err(err) = &new_versions {
                            error!(?err, "Failed to fetch updatable versions");
                        }

                        if let Some(MainFocusState::Dependency { versions, .. }) =
                            &mut self.main_focus_state
                        {
                            *versions = Some(new_versions);
                        }
                    }

                    AppEvent::LoadDependencyFeatures(dependency_cursor) => {
                        self.load_dependency_features(&dependency_cursor);
                    }

                    AppEvent::UpdateDependencyFeatures { features } => {
                        if let Err(err) = &features {
                            error!(?err, "Failed to fetch features");
                        }

                        if let Some(MainFocusState::Dependency {
                            features: old_features,
                            ..
                        }) = &mut self.main_focus_state
                        {
                            *old_features = Some(features);
                        }
                    }
                },
            }
        }
        Ok(())
    }

    pub fn focused_area(&self) -> &FocusedArea {
        self.focus_stack
            .last()
            .expect("Expected there to be a focused area. None means the app should've closed.")
    }

    const fn create_widget(&self) -> AppUI<'_> {
        AppUI { state: self }
    }

    /// Handles an update check for a dependency.
    #[tracing::instrument(skip(self), fields(registry_source))]
    fn check_for_update(&mut self, cursor: DependencyCursor) {
        debug!("Update check requested for dependency");
        if matches!(self.update_cache.get(&cursor), Some(UpdateStatus::Pending)) {
            debug!("Update check already pending for dependency");
            return;
        }

        let dependency = self
            .manifest
            .resolve_dependency(&cursor)
            .unwrap()
            .expect("How the hell did we run an update check on a non-existent dependency");

        // Clone the sender, not self — sender is cheap to clone and 'static
        let sender = self.events.sender().clone();

        self.update_cache
            .insert(cursor.clone(), UpdateStatus::Pending);

        match dependency.source() {
            Source::Registry(registry_source) => {
                let name = dependency.resolved_name().to_string();
                let registry_updater = registry::Updater::new(self.client.clone());
                let registry_source = registry_source.clone();

                fields!(registry_source = %&registry_source);

                tokio::spawn(
                    async move {
                        debug!("Checking for update for dependency {name}");
                        let versions =
                            registry_updater
                                .get_versions(&name)
                                .await
                                .and_then(|versions| {
                                    registry::Updater::filter_update_results(
                                        &registry_source,
                                        versions.iter(),
                                    )
                                });

                        let result = match versions {
                            Ok(UpdateResult {
                                latest_version: None,
                                semantic_latest_version: None,
                            }) => UpdateStatus::UpToDate,

                            Ok(UpdateResult {
                                latest_version: Some(latest),
                                semantic_latest_version: Some(semantic_latest),
                            }) if registry_source.matches(&latest)
                                && registry_source.matches(&semantic_latest) =>
                            {
                                UpdateStatus::UpToDate
                            }

                            Ok(result) => UpdateStatus::UpdateAvailable(result.into()),

                            Err(err) => UpdateStatus::Error(err),
                        };

                        // we don't care if the send fails, the event will be dropped
                        sender
                            .send(Event::App(AppEvent::UpdateCheckResult { cursor, result }))
                            .ok();
                    }
                    .in_current_span(),
                );
            }
            Source::Git { .. } => {
                todo!()
            }
            Source::Path { .. } => {
                debug!("Can't check for updates for path dependencies");
                sender
                    .send(Event::App(AppEvent::UpdateCheckResult {
                        cursor,
                        result: UpdateStatus::UpToDate,
                    }))
                    .ok();
            }
        }
    }

    fn load_dependency_versions(&self, cursor: &DependencyCursor) {
        debug!("Update check requested for dependency");

        let dependency = self
            .manifest
            .resolve_dependency(cursor)
            .unwrap()
            .expect("How the hell did we run an update check on a non-existent dependency");

        // Clone the sender, not self (sender is cheap to clone)
        let sender = self.events.sender().clone();

        match dependency.source() {
            Source::Registry(registry_source) => {
                let name = dependency.resolved_name().to_string();
                let registry_updater = registry::Updater::new(self.client.clone());
                let registry_source = registry_source.clone();

                fields!(registry_source = %&registry_source);

                tokio::spawn(
                    async move {
                        debug!("Checking for update for dependency {name}");

                        let versions = registry_updater.get_versions(&name).await.map(|versions| {
                            versions
                                .into_iter()
                                .filter(|v| !v.yanked)
                                .map(|version| version.num)
                                .map(VersionType::Registry)
                                .collect::<Vec<_>>()
                        });

                        sender.send(Event::App(AppEvent::UpdateDependencyVersions { versions }))
                    }
                    .in_current_span(),
                );
            }
            Source::Git { .. } => {
                sender
                    .send(Event::App(AppEvent::UpdateDependencyVersions {
                        versions: Err(CheckForUpdateError::UnsupportedDependency.into()),
                    }))
                    .ok();
            }
            Source::Path { .. } => {
                debug!("Can't check for updates for path dependencies");

                sender
                    .send(Event::App(AppEvent::UpdateDependencyVersions {
                        versions: Err(CheckForUpdateError::UnsupportedDependency.into()),
                    }))
                    .ok();
            }
        }
    }

    /// Queues checking for updates for all dependencies in the manifest.
    pub fn queue_check_for_updates(&self) -> Result<(), Report<AppError>> {
        for table in self
            .manifest
            .dependencies()
            .attach("Failed to parse dependencies")
            .change_context(AppError::Event)?
        {
            let cursor = &table.cursor;

            for dependency in table.deps {
                let dep_cursor = DependencyCursor {
                    table: cursor.clone(),
                    name: dependency.name().to_string(),
                };

                self.events
                    .send(AppEvent::UpdateCheck { cursor: dep_cursor });
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip(self), fields(dependency))]
    pub fn update_dependency(
        &mut self,
        cursor: &DependencyCursor,
        version: &VersionType,
    ) -> Result<(), Report<DependencyUpdateError>> {
        let mut dependency = self
            .manifest
            .resolve_dependency_mut(cursor)
            .expect("Failed to resolve dependency");

        fields!(dependency = %&dependency);

        dependency.set_version(version)?;

        self.manifest
            .save()
            .change_context(DependencyUpdateError::SaveManifestError)?;

        Ok(())
    }

    fn load_dependency_features(&self, dependency_cursor: &DependencyCursor) {
        // todo: cache metadata
        let metadata = cargo_metadata::MetadataCommand::new().exec();

        match metadata {
            Ok(metadata) => {
                let features = fetch_features(&metadata, dependency_cursor);

                self.events.send(AppEvent::UpdateDependencyFeatures {
                    features: features.cloned(),
                });
            }
            Err(err) => {
                self.events.send(AppEvent::UpdateDependencyFeatures {
                    features: Err(err).change_context(FetchFeaturesError::MetadataFetchError),
                });
            }
        }
    }
}
