use std::fmt::{self, Display};

use displaydoc::Display;
use error_stack::{IntoReport, Report, bail};
use semver::VersionReq;
use thiserror::Error;
use toml_edit::{Array, InlineTable, Item, TableLike, Value, value};
use tracing::debug;

use crate::{
    fields,
    updater::{DependencyUpdateError, VersionType},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DependencyKind {
    Normal,
    Dev,
    Build,
}

impl DependencyKind {
    pub const KINDS: &[Self] = &[Self::Normal, Self::Dev, Self::Build];

    pub const fn section(&self) -> &str {
        match self {
            Self::Normal => "dependencies",
            Self::Dev => "dev-dependencies",
            Self::Build => "build-dependencies",
        }
    }
}

impl Display for DependencyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Normal => "Dependencies",
                Self::Dev => "Dev Dependencies",
                Self::Build => "Build Dependencies",
            }
        )
    }
}

// todo: fancy Display impl for a Dependency Ref so I don't duplicate it all over the place (check for name usages once done)
#[derive(Debug, Clone)]
pub struct DependencyRef<'a> {
    item: &'a Item,
    name: &'a str,
    source: Source<'a>,
    default_features: bool,
    package: Option<&'a str>,
    // features should be gotten from the manifest method due to allocation if we wanted to embed it in here
}
impl<'a> DependencyRef<'a> {
    #[tracing::instrument]
    pub fn from_item(name: &'a str, item: &'a Item) -> Result<Self, Report<DependencyParseError>> {
        let source = match item {
            Item::Value(Value::String(version)) => {
                if let Ok(parsed_version) = semver::Version::parse(version.value()) {
                    Source::Registry(RegistrySource::Version(parsed_version))
                } else if let Ok(requirement) = semver::VersionReq::parse(version.value()) {
                    Source::Registry(RegistrySource::Requirement(requirement))
                } else {
                    bail!(DependencyParseError::InvalidSource)
                }
            }
            Item::Value(Value::InlineTable(table)) => Source::try_from_table(table)?,
            Item::Table(table) => Source::try_from_table(table)?,
            _ => bail!(DependencyParseError::InvalidSource),
        };

        let default_features = match item {
            Item::Value(Value::InlineTable(table)) => table
                .get("default-features")
                .is_none_or(|v| v.as_bool().unwrap_or_default()),
            Item::Value(_) => true,
            Item::Table(table) => table
                .get("default-features")
                .is_none_or(|v| v.as_bool().unwrap_or_default()),
            _ => bail!(DependencyParseError::InvalidSource),
        };

        let package = match item {
            Item::Value(Value::InlineTable(table)) => table.get("package").and_then(|v| v.as_str()),
            Item::Value(_) => None,
            Item::Table(table) => table.get("package").and_then(|v| v.as_str()),
            _ => bail!(DependencyParseError::InvalidSource),
        };

        Ok(Self {
            item,
            name,
            source,
            default_features,
            package,
        })
    }

    pub const fn name(&self) -> &'a str {
        self.name
    }

    pub const fn source(&self) -> &Source<'a> {
        &self.source
    }

    pub const fn default_features(&self) -> bool {
        self.default_features
    }

    pub const fn package(&self) -> Option<&'a str> {
        self.package
    }

    /// Returns the resolved name of the dependency, which is the package name if available, otherwise the dependency name.
    ///
    /// This can be used when fetching the dependency from a registry.
    // (I have no idea what else tbh)
    pub fn resolved_name(&self) -> &'a str {
        self.package.unwrap_or(self.name)
    }

    pub fn features(&'a self) -> Option<impl Iterator<Item = &'a str> + 'a> {
        let handle_table_like = |table: &'a dyn TableLike| {
            table
                .get("features")
                .and_then(|v| v.as_array())
                .map(move |arr| arr.iter().filter_map(|v| v.as_str()))
        };

        match self.item {
            Item::Value(Value::InlineTable(table)) => handle_table_like(table),
            Item::Table(table) => handle_table_like(table),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct MutableDependency<'a> {
    item: &'a mut Item,
}

impl<'a> MutableDependency<'a> {
    pub const fn new(item: &'a mut Item) -> Self {
        Self { item }
    }

    #[tracing::instrument(skip(self))]
    pub fn set_version(
        &mut self,
        version: &VersionType,
    ) -> Result<(), Report<DependencyUpdateError>> {
        let handle_table_like = |table: &mut dyn TableLike| match &version {
            VersionType::Registry(new_version) => {
                table.insert("version", value(new_version.to_string()));
            }
        };

        match self.item {
            Item::Value(Value::String(_)) => {
                let VersionType::Registry(new_version) = version else {
                    bail!(DependencyUpdateError::InvalidVersionType {
                        expected: VersionType::REGISTRY,
                        actual: version.name(),
                    })
                };

                // I am very good at naming things
                debug!(new_version = %new_version, "Updating value registry dependency version");

                *self.item = value(new_version.to_string());
            }
            Item::Value(Value::InlineTable(table)) => {
                debug!(table = %&table, "Updating table-like version");
                handle_table_like(table);
            }
            Item::Table(table) => {
                debug!(table = %&table, "Updating table version");
                handle_table_like(table);
            }
            _ => bail!(DependencyUpdateError::Unsupported),
        }

        Ok(())
    }

    pub fn set_default_features(
        &mut self,
        default: bool,
    ) -> Result<(), Report<DependencyUpdateError>> {
        let handle_table_like = |table: &mut dyn TableLike| {
            table.insert("default-features", value(default));
        };

        match self.item {
            Item::Value(Value::String(version)) => {
                let mut table = InlineTable::default();
                table.insert("version", Value::String(version.clone()));
                handle_table_like(&mut table);
                *self.item = Item::Value(Value::InlineTable(table));
            }
            Item::Value(Value::InlineTable(table)) => {
                handle_table_like(table);
            }
            Item::Table(table) => {
                handle_table_like(table);
            }
            _ => bail!(DependencyUpdateError::Unsupported),
        }

        Ok(())
    }

    pub fn add_feature(&mut self, feature: &str) -> Result<(), Report<DependencyUpdateError>> {
        let handle_table_like = |table: &mut dyn TableLike| {
            let features = table
                .entry("features")
                .or_insert_with(|| Array::default().into())
                .as_array_mut()
                .ok_or(DependencyUpdateError::Unsupported)?;

            if !features.iter().any(|v| v.as_str() == Some(feature)) {
                features.push(feature);
            }

            Ok::<(), Report<DependencyUpdateError>>(())
        };

        match self.item {
            Item::Value(Value::String(version)) => {
                let mut table = InlineTable::default();
                table.insert("version", Value::String(version.clone()));
                handle_table_like(&mut table)?;
                *self.item = Item::Value(Value::InlineTable(table));
            }
            Item::Value(Value::InlineTable(table)) => {
                handle_table_like(table)?;
            }
            Item::Table(table) => {
                handle_table_like(table)?;
            }
            _ => bail!(DependencyUpdateError::Unsupported),
        }

        Ok(())
    }

    pub fn remove_feature(&mut self, feature: &str) -> Result<(), Report<DependencyUpdateError>> {
        let handle_table_like = |table: &mut dyn TableLike| {
            if let Some(features) = table.get_mut("features").and_then(|v| v.as_array_mut()) {
                features.retain(|v| v.as_str() != Some(feature));
            }

            Ok::<(), Report<DependencyUpdateError>>(())
        };

        match self.item {
            Item::Value(Value::String(version)) => {
                let mut table = InlineTable::default();
                table.insert("version", Value::String(version.clone()));
                handle_table_like(&mut table)?;
                *self.item = Item::Value(Value::InlineTable(table));
            }
            Item::Value(Value::InlineTable(table)) => {
                handle_table_like(table)?;
            }
            Item::Table(table) => {
                handle_table_like(table)?;
            }
            _ => bail!(DependencyUpdateError::Unsupported),
        }

        Ok(())
    }
}

impl Display for MutableDependency<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.item)
    }
}

/// Errors that can occur when parsing a dependency.
#[derive(Debug, Display, Error)]
pub enum DependencyParseError {
    /// The source of the dependency could not be parsed.
    InvalidSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Source<'a> {
    Registry(RegistrySource),
    Git {
        url: &'a str,
        branch: Option<&'a str>,
        tag: Option<&'a str>,
        git_ref: Option<&'a str>,
    },
    Path {
        path: &'a str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RegistrySource {
    Version(semver::Version),
    Requirement(semver::VersionReq),
}

impl RegistrySource {
    pub fn matches(&self, version: &semver::Version) -> bool {
        match self {
            Self::Version(v) => v == version,
            Self::Requirement(r) => r.matches(version),
        }
    }

    pub fn compatible_req(&self) -> VersionReq {
        match self {
            Self::Version(v) => {
                VersionReq::parse(&format!("^{v}")).expect("invalid version requirement")
            }
            Self::Requirement(r) => r.clone(),
        }
    }
}

impl Display for RegistrySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Version(v) => write!(f, "{v}"),
            Self::Requirement(r) => write!(f, "{r}"),
        }
    }
}

impl<'a> Source<'a> {
    #[tracing::instrument(fields(version))]
    pub fn try_from_table(
        table: &'a (impl TableLike + fmt::Debug),
    ) -> Result<Self, Report<DependencyParseError>> {
        // TODO: handle version + other fields existing at the same time, and thus ignore version
        // I think the easiest way is to make version the final check/fallback if git/path/etc doesn't exist
        table
            .get("version")
            .and_then(|v| v.as_str())
            .map(|v| {
                fields!(version = v);
                // clippy I fear your idea of better code is wrong here
                #[allow(clippy::option_if_let_else)]
                if let Ok(version) = semver::Version::parse(v) {
                    Ok(Self::Registry(RegistrySource::Version(version)))
                } else if let Ok(requirement) = semver::VersionReq::parse(v) {
                    Ok(Self::Registry(RegistrySource::Requirement(requirement)))
                } else {
                    Err(DependencyParseError::InvalidSource
                        .into_report()
                        .attach("Could not convert version to semver"))
                }
            })
            .ok_or_else(|| {
                DependencyParseError::InvalidSource
                    .into_report()
                    .attach("version field is required")
            })?
    }
}

impl Display for Source<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::Registry(RegistrySource::Version(version)) => write!(f, "{version}"),
            Source::Registry(RegistrySource::Requirement(requirement)) => {
                write!(f, "{requirement}")
            }
            Source::Git {
                url,
                branch,
                tag,
                git_ref,
            } => {
                write!(f, "git+{url}")?;
                if let Some(branch) = branch {
                    write!(f, "#{branch}")?;
                }
                if let Some(tag) = tag {
                    write!(f, "#{tag}")?;
                }
                if let Some(git_ref) = git_ref {
                    write!(f, "#{git_ref}")?;
                }
                Ok(())
            }
            Source::Path { path } => write!(f, "path+{path}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// A cursor to a dependency table parsed from a `Cargo.toml` file
pub struct DependencyTableCursor {
    /// The kind of dependency table this is (e.g. `dependencies`, `dev-dependencies`)
    pub kind: DependencyKind,
    /// The target platform this table is for (e.g. `x86_64-unknown-linux-gnu`), if any
    pub target: Option<String>,
}

impl DependencyTableCursor {
    pub fn to_table_name(&self) -> String {
        self.target.as_ref().map_or_else(
            || self.kind.section().to_owned(),
            |target| format!("target.{}.{}", target, self.kind.section()),
        )
    }
}

impl Display for DependencyTableCursor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(target) = &self.target {
            write!(f, "{} ({target})", self.kind)
        } else {
            write!(f, "{}", self.kind)
        }
    }
}

#[derive(Debug)]
/// A table of dependencies parsed from a `Cargo.toml` file
pub struct DependencyTable<'a> {
    /// The table cursor information
    pub cursor: DependencyTableCursor,
    /// The dependencies in this table
    pub deps: Vec<DependencyRef<'a>>,
}

/// A cursor to a specific dependency in a [`DependencyTable`]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DependencyCursor {
    /// The table this cursor is pointing to
    pub table: DependencyTableCursor,
    /// The name of the dependency this cursor is pointing to
    pub name: String,
}
