use displaydoc::Display;
use thiserror::Error;

pub struct PackageInfo<'a> {
    pub name: &'a str,
    pub version: &'a str,
}

#[derive(Debug, Error, Display)]
/// Errors that can occur when parsing a package manifest.
pub enum PackageParseError {
    /// The `name` field is missing from the package manifest.
    MissingName,
    /// The `version` field is missing from the package manifest.
    MissingVersion,
}

impl<'a> PackageInfo<'a> {
    pub fn from_table(table: &'a toml_edit::Table) -> Result<Self, PackageParseError> {
        Ok(Self {
            name: table
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or(PackageParseError::MissingName)?,

            version: table
                .get("version")
                .and_then(|v| v.as_str())
                .ok_or(PackageParseError::MissingVersion)?,
        })
    }
}
