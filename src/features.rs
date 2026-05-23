use std::collections::{BTreeMap, HashSet};

use displaydoc::Display;
use error_stack::Report;
use thiserror::Error;
use tracing::{debug, trace};

use crate::{fields, manifest::dependency::DependencyCursor};

#[derive(Debug, Error, Display)]
/// Failed to fetch features for a dependency.
pub enum FetchFeaturesError {
    /// A cargo-metadata error occurred while fetching features.
    MetadataFetchError,
    /// The dependency is not in the metadata.
    NotFound,
    /// The root project is not in the metadata.
    RootNotFound,
    /// The resolution graph is not available.
    ResolutionNotFound,
}

// may someone with better knowledge of cargo_metadata bestow upon me the knowledge to make this not janky as FU-
#[tracing::instrument(skip(metadata), fields(root_package, dependency))]
pub fn fetch_features<'a>(
    metadata: &'a cargo_metadata::Metadata,
    cursor: &DependencyCursor,
) -> Result<&'a FeaturesList, Report<FetchFeaturesError>> {
    let resolve = &metadata
        .resolve
        .as_ref()
        .ok_or(FetchFeaturesError::RootNotFound)?;

    let root_package = metadata
        .root_package()
        .ok_or(FetchFeaturesError::ResolutionNotFound)?;

    fields!(root_package = ?root_package);

    let root_dependency = root_package
        .dependencies
        .iter()
        .find(|dep| {
            dep.rename.as_ref().is_some_and(|r| *r == cursor.name) || dep.name == cursor.name
        })
        .ok_or(FetchFeaturesError::NotFound)?;

    let resolver_root = resolve
        .nodes
        .iter()
        .find(|node| node.id == root_package.id)
        .ok_or(FetchFeaturesError::RootNotFound)?;

    let package = resolver_root
        .dependencies
        .iter()
        .find_map(|resolver_dependency_id| {
            // I genuinely can't think of another way to do this
            // This is basically "linking" together the resolver's dependency ID with the metadata's package ID and name
            //
            // This ensures that if there is a duplicate package for whatever reason, it doesn't get picked up by accident
            // If I didn't have to check that (idk if I even have to, I just assume I do for e.g. the same dependency on differing versions), I think I could just do this in 1 layer?
            metadata.packages.iter().find(|package| {
                package.id == *resolver_dependency_id && package.name == root_dependency.name
            })
        })
        .ok_or(FetchFeaturesError::NotFound)?;

    Ok(&package.features)
}

pub type FeaturesList = BTreeMap<String, Vec<String>>;
pub type FeaturesGraph<'a> = BTreeMap<&'a str, Vec<&'a str>>;

pub fn create_implied_graph<'a>(
    features: &'a FeaturesList,
    enabled: impl IntoIterator<Item = &'a str>,
    default_enabled: bool,
) -> FeaturesGraph<'a> {
    let mut implicit_from: FeaturesGraph<'a> = features
        .keys()
        .map(|k| (k.as_str(), vec![]))
        .collect::<FeaturesGraph<'a>>();

    // basically a path buffer of the current feature chain
    let mut via_chain: Vec<&'a str> = Vec::new();

    // (feature, via_chain_length)
    let mut stack: Vec<(&'a str, usize)> = enabled.into_iter().map(|f| (f, 0)).collect();

    let mut visited = HashSet::new();

    if default_enabled {
        stack.push(("default", 0));
    }

    while let Some((feature, via_chain_length)) = stack.pop()
        && visited.insert(feature)
        && let Some(deps) = features.get(feature)
    {
        trace!(?via_chain, "popped feature: {}", feature);

        // reset back to the part of the chain we're dealing with
        via_chain.truncate(via_chain_length);
        via_chain.push(feature);

        debug!(
            ?via_chain,
            ?deps,
            "Updating via_chain for feature: {}",
            feature
        );

        let filtered = deps
            .iter()
            .filter(|dep| !dep.starts_with("dep:") && !dep.contains('/'));

        for dep in filtered {
            trace!(?via_chain, "filtered dep: {}", dep);

            implicit_from.entry(dep).insert_entry(via_chain.clone());

            stack.push((dep.as_str(), via_chain.len()));
        }
    }

    implicit_from
}
