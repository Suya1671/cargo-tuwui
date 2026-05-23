pub mod dependency;
pub mod package;

use std::{io, path::PathBuf};

use displaydoc::Display;
use error_stack::{IntoReport, Report, ResultExt};
use thiserror::Error;
use toml_edit::{Item, TableLike};

use crate::manifest::{
    dependency::{
        DependencyCursor, DependencyKind, DependencyParseError, DependencyRef, DependencyTable,
        DependencyTableCursor, MutableDependency,
    },
    package::PackageInfo,
};

/// An editable cargo manifest
#[derive(Debug)]
pub struct Manifest {
    /// The path to the manifest file
    path: PathBuf,
    /// The raw parsed TOML data
    data: toml_edit::DocumentMut,
}

#[derive(Debug, Error, Display)]
/// Errors that can occur when parsing a cargo manifest
pub enum Error {
    /// Failed to parse the manifest file
    ParseError,
    /// The manifest file does not exist
    NotFound,
}

#[derive(Debug, Error, Display)]
/// Errors that can occur when parsing a cargo manifest package
pub enum PackageError {
    /// The package section is missing from the manifest
    NoPackage,
    /// Failed to parse the package section
    ParseError,
}

impl Manifest {
    #[tracing::instrument]
    pub fn new(path: PathBuf) -> Result<Self, Report<Error>> {
        let data = std::fs::read_to_string(&path)
            .change_context(Error::NotFound)?
            .parse::<toml_edit::DocumentMut>()
            .change_context(Error::ParseError)?;

        Ok(Self { path, data })
    }

    #[tracing::instrument(skip(self))]
    pub fn package(&'_ self) -> Result<PackageInfo<'_>, Report<PackageError>> {
        PackageInfo::from_table(
            self.data
                .get("package")
                .ok_or_else(|| PackageError::NoPackage.into_report())?
                .as_table()
                .ok_or_else(|| {
                    PackageError::ParseError
                        .into_report()
                        .attach("Failed to interpret package section as a table")
                })?,
        )
        .change_context(PackageError::ParseError)
    }

    #[tracing::instrument(skip(self))]
    pub fn dependencies(&'_ self) -> Result<Vec<DependencyTable<'_>>, Report<PackageError>> {
        let mut sections = Vec::new();
        for &kind in DependencyKind::KINDS {
            if let Some(section) = self.data.get(kind.section()) {
                let deps = section
                    .as_table_like()
                    .ok_or_else(|| {
                        PackageError::ParseError
                            .into_report()
                            .attach("Failed to interpret dependency section as a table")
                    })?
                    .iter()
                    .map(|(name, value)| DependencyRef::from_item(name, value))
                    .collect::<Result<Vec<_>, _>>()
                    .attach("Failed to parse dependencies")
                    .change_context(PackageError::ParseError)?;

                sections.push(DependencyTable {
                    cursor: DependencyTableCursor { kind, target: None },
                    deps,
                });
            }
        }

        for target_specific_tables in self
            .data
            .get("target")
            .and_then(Item::as_table_like)
            .into_iter()
            .flat_map(TableLike::iter)
            .flat_map(|(target_name, table)| {
                DependencyKind::KINDS
                    .iter()
                    .filter_map(move |kind| Some((target_name, table.get(kind.section())?, kind)))
            })
            .map(
                |(target_name, table, &kind)| -> Result<DependencyTable<'_>, Report<PackageError>> {
                    let deps = table
                        .as_table_like()
                        .ok_or_else(|| {
                            PackageError::ParseError
                                .into_report()
                                .attach("Failed to interpret target section as a table")
                        })?
                        .iter()
                        .map(|(name, value)| DependencyRef::from_item(name, value))
                        .collect::<Result<Vec<_>, _>>()
                        .attach("Failed to parse dependencies")
                        .change_context(PackageError::ParseError)?;

                    Ok(DependencyTable {
                        cursor: DependencyTableCursor {
                            target: Some(target_name.to_owned()),
                            kind,
                        },
                        deps,
                    })
                },
            )
        {
            sections.push(target_specific_tables?);
        }

        Ok(sections)
    }

    pub fn resolve_dependency(
        &self,
        cursor: &DependencyCursor,
    ) -> Result<Option<DependencyRef<'_>>, Report<DependencyParseError>> {
        let table_name = cursor.table.to_table_name();

        self.data
            .get(&table_name)
            .and_then(|table| table.as_table_like())
            .and_then(|table| {
                table
                    .get_key_value(&cursor.name)
                    .map(|(key, value)| DependencyRef::from_item(key.get(), value))
            })
            .transpose()
    }

    pub fn resolve_dependency_mut(
        &mut self,
        cursor: &DependencyCursor,
    ) -> Option<MutableDependency<'_>> {
        let table_name = cursor.table.to_table_name();

        self.data
            .get_mut(&table_name)
            .and_then(|table| table.as_table_like_mut())
            .and_then(|table| table.get_mut(&cursor.name))
            .map(MutableDependency::new)
    }

    pub fn save(&self) -> Result<(), Report<io::Error>> {
        std::fs::write(&self.path, self.data.to_string()).attach("Failed to write manifest to disk")
    }
}
