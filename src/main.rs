pub mod cli;
pub mod generator;
pub mod utils;

use std::{fs::File, io::Write, path::Path};

use cli::cli;
use generator::{
    component::{generate_components, write_object_database},
    paths::generate_paths,
};
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
    let spec = match oas3::from_path(Path::new(spec_file_path)) {
        Ok(spec) => spec,
        Err(err) => panic!("{}", err.to_string()),
    };

    // 2. Load config (Get mapper for invalid language names, ignores...)
    let config = match config_file_path {
        Some(mapping_file) => {
            Config::from(Path::new(mapping_file)).expect("Failed to parse config")
        }
        None => Config::new(),
    };

    // 3. Generate Code
    // 3.1 Components and database for type referencing
    let mut object_database = &mut generate_components(&spec, &config).unwrap();
    // 3.2 Generate paths requests
    generate_paths(output_dir, &spec, &mut object_database, &config);

    // 3.3 Write all registered objects to individual type definitions
    if let Err(err) = write_object_database(output_dir, &mut object_database, &config.name_mapping)
    {
        panic!("{}", err)
    }

    // 4. Project setup
    let mut lib_file =
        File::create(format!("{}/src/lib.rs", output_dir)).expect("Failed to create lib.rs");

    lib_file
        .write("pub mod objects;\n".to_string().as_bytes())
        .unwrap();
    lib_file
        .write("pub mod paths;\n".to_string().as_bytes())
        .unwrap();
}
