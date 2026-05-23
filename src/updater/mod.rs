pub mod registry;

use std::{collections::HashMap, fmt::Display};

use crate::{
    manifest::dependency::{DependencyCursor, RegistrySource, Source},
    ui::icons::Icons,
};
use derive_more::From;
use displaydoc::Display;
use error_stack::Report;
use ratatui::{
    style::{Color, Style},
    text::Span,
};
use thiserror::Error;

#[derive(Debug, Error, Display)]
pub enum CheckForUpdateError {
    /// Failed to send the request to the update server.
    RequestFailed,
    /// Failed to deserialize the response from the update server.
    DeserializationFailed,
    /// The dependency does not support update checking.
    UnsupportedDependency,
}

#[derive(Debug, Error, Display)]
/// Errors that can occur during dependency updates.
pub enum DependencyUpdateError {
    /// Expected to update an `{expected}` version, but got `{actual}`
    InvalidVersionType {
        expected: &'static str,
        actual: &'static str,
    },
    /// Could not save the updated manifest
    SaveManifestError,
    /// Unsupported version type. This is a bug, since this should never happen.
    Unsupported,
    /// There is no update available for this dependency.
    NoUpdate,
    /// We are still waiting for the update to be checked.
    Pending,
    /// There was an error while checking for updates.
    UpdateCheckError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// The result of checking for updates.
pub struct UpdateResult<VerType = VersionType> {
    /// The absolute latest version available.
    pub latest_version: Option<VerType>,
    /// The semantic latest version available.
    pub semantic_latest_version: Option<VerType>,
}

impl<T: Into<VersionType>> UpdateResult<T> {
    pub fn into(self) -> UpdateResult<VersionType> {
        UpdateResult {
            latest_version: self.latest_version.map(Into::into),
            semantic_latest_version: self.semantic_latest_version.map(Into::into),
        }
    }
}

pub type UpdateCache = HashMap<DependencyCursor, UpdateStatus<VersionType>>;

#[derive(Debug)]
pub enum UpdateStatus<VerType = VersionType> {
    Pending,
    UpToDate,
    UpdateAvailable(UpdateResult<VerType>),
    Error(Report<CheckForUpdateError>),
}

#[derive(Debug, Clone, From, Eq, PartialEq)]
pub enum VersionType {
    Registry(#[from] semver::Version),
}

impl PartialEq<semver::Version> for VersionType {
    fn eq(&self, other: &semver::Version) -> bool {
        match self {
            Self::Registry(version) => version == other,
        }
    }
}

impl PartialEq<Source<'_>> for VersionType {
    fn eq(&self, other: &Source) -> bool {
        match (self, other) {
            (Self::Registry(a), Source::Registry(RegistrySource::Version(b))) => a == b,

            (Self::Registry(_), Source::Registry(RegistrySource::Requirement(_)))
            | (Self::Registry(..), Source::Git { .. } | Source::Path { .. }) => false,
        }
    }
}

impl VersionType {
    pub const REGISTRY: &'static str = "registry";

    pub const fn name(&self) -> &'static str {
        match self {
            Self::Registry(_) => Self::REGISTRY,
        }
    }
}

impl Display for VersionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Registry(version) => write!(f, "{version}"),
        }
    }
}

impl<VerType> UpdateStatus<VerType> {
    pub const fn icon(&self) -> Icons {
        match self {
            Self::Pending => Icons::Loading,

            Self::UpToDate
            | Self::UpdateAvailable(UpdateResult {
                latest_version: None,
                semantic_latest_version: None,
            }) => Icons::Done,

            Self::UpdateAvailable(UpdateResult {
                latest_version: Some(_),
                semantic_latest_version: None,
            }) => Icons::Warning,

            Self::UpdateAvailable(UpdateResult {
                semantic_latest_version: Some(_),
                ..
            }) => Icons::Update,

            Self::Error(_) => Icons::Error,
        }
    }

    pub const fn color(&self) -> Color {
        match self {
            Self::Pending => Color::Gray,
            Self::UpToDate => Color::Green,
            Self::UpdateAvailable(_) => Color::Yellow,
            Self::Error(_) => Color::Red,
        }
    }

    pub fn as_span(&self) -> Span<'_> {
        Span::styled(self.icon().get_icon(), Style::default().fg(self.color()))
    }
}
