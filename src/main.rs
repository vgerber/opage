pub mod cli;
pub mod generator;
pub mod parser;
pub mod utils;

use std::path::Path;

use cli::cli;
use generator::rust_reqwest_async::project::generate_project;
use parser::component::generate_components;
use utils::{config::Config, log::Logger};

static LOGGER: Logger = Logger;

fn main() {
    let matches = cli().get_matches();

    let output_dir = matches
        .get_one::<String>("output-dir")
        .map(String::as_str)
        .expect("output-dir missing");
    let spec_file_path = matches
        .get_one::<String>("spec")
        .map(String::as_str)
        .expect("spec missing");
    let config_file_path = matches.get_one::<String>("config").map(String::as_str);

    log::set_logger(&LOGGER).expect("Failed to set logger");
    log::set_max_level(log::LevelFilter::Trace);

    // Start generating

    // 1. Read spec
    let spec_yaml = std::fs::read_to_string(spec_file_path).expect("Failed to read yaml");
    let spec = oas3::from_yaml(spec_yaml).expect("Failed to read spec");

    // 2. Load config (Get mapper for invalid language names, ignores...)
    let config = match config_file_path {
        Some(mapping_file) => {
            Config::from(Path::new(mapping_file)).expect("Failed to parse config")
        }
        None => Config::new(),
    };

    // 3. Generate Code
    // 3.1 Components and database for type referencing
    let object_database = &mut generate_components(&spec, &config).unwrap();
    // 3.2 Generate paths requests

    // 3.3 Write all registered objects to individual type definitions
    generate_project(output_dir, object_database, &config, &spec);
}
