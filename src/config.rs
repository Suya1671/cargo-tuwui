use std::path::{Path, PathBuf};

use config::{Environment, File};
use directories::ProjectDirs;
use error_stack::{Report, ResultExt};
use serde::Deserialize;

use crate::Error;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Config {
    pub logging_path: PathBuf,
}

impl Config {
    pub fn new() -> Result<Self, Report<Error>> {
        let proj_dirs = ProjectDirs::from("in", "wobbl", "tuwui").unwrap();

        config::Config::builder()
            .set_default(
                "logging_path",
                proj_dirs
                    .data_dir()
                    .to_path_buf()
                    .join("tuwui.log")
                    .display()
                    .to_string(),
            )
            .attach("Could not set default logging path")
            .change_context(Error::Init)?
            .add_source(File::from(proj_dirs.config_dir().join("config.toml")).required(false))
            .add_source(File::with_name("tuwui").required(false))
            .add_source(Environment::with_prefix("TUWUI"))
            .build()
            .attach("Could not build config")
            .change_context(Error::Init)?
            .try_deserialize()
            .attach("Could not deserialize config")
            .change_context(Error::Init)
    }

    pub fn logging_path(&self) -> &Path {
        &self.logging_path
    }
}
