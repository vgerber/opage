use std::{fs::File, path::Path};

use serde::Deserialize;

use super::{name_mapping::NameMapping, spec_ignore::SpecIgnore};

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ProjectMetadata {
    pub name: String,
    pub version: String,
}

impl ProjectMetadata {
    pub fn new() -> Self {
        ProjectMetadata {
            name: String::new(),
            version: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Config {
    pub project_metadata: ProjectMetadata,
    pub name_mapping: NameMapping,
    pub ignore: SpecIgnore,
}

impl Config {
    pub fn from(config_file_path: &Path) -> Result<Self, String> {
        let file = match File::open(config_file_path) {
            Ok(file) => file,
            Err(err) => return Err(err.to_string()),
        };
        match serde_json::from_reader(file) {
            Ok(config_object) => Ok(config_object),
            Err(err) => return Err(err.to_string()),
        }
    }

    pub fn new() -> Self {
        Config {
            project_metadata: ProjectMetadata::new(),
            name_mapping: NameMapping::new(),
            ignore: SpecIgnore::new(),
        }
    }
}
