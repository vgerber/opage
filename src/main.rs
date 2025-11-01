pub mod cli;
pub mod generator;
pub mod parser;
pub mod utils;

use std::{fs::File, io::Write, path::Path};

use cli::cli;
use generator::rust_reqwest_async::objects::write_object_database;
use generator::rust_reqwest_async::{cargo::generate_cargo_content, paths::generate_paths};
use log::info;
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
    let mut object_database = &mut generate_components(&spec, &config).unwrap();
    // 3.2 Generate paths requests
    let generated_paths = generate_paths(output_dir, &spec, &mut object_database, &config)
        .expect("Failed to generated paths");

    // 3.3 Write all registered objects to individual type definitions
    write_object_database(output_dir, &mut object_database, &config.name_mapping)
        .expect("Write objects failed");
    // 4. Project setup
    let mut lib_file =
        File::create(format!("{}/src/lib.rs", output_dir)).expect("Failed to create lib.rs");

    if object_database.len() > 0 {
        lib_file
            .write("pub mod objects;\n".to_string().as_bytes())
            .unwrap();
    }

    if generated_paths > 0 {
        lib_file
            .write("pub mod paths;\n".to_string().as_bytes())
            .unwrap();
    }

    let output_cargo_file_path = format!("{}/Cargo.toml", output_dir);
    let cargo_file_path = Path::new(&output_cargo_file_path);
    if cargo_file_path.exists() {
        info!("{:?} exists and will be skipped", output_cargo_file_path);
        return;
    }

    let mut cargo_file = File::create(output_cargo_file_path).expect("Failed to create Cargo.toml");
    cargo_file
        .write(
            generate_cargo_content(&config.project_metadata)
                .expect("Failed to generate Cargo.toml")
                .as_bytes(),
        )
        .expect("Failed to write Cargo.toml");
}
