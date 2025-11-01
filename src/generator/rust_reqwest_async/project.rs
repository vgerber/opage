use std::{fs::File, io::Write, path::Path};

use log::info;

use super::cargo::generate_cargo_content;
use super::objects::write_object_database;
use super::paths::generate_paths;
use crate::parser::component::object_definition::types::ObjectDatabase;
use crate::utils::config::Config;

pub fn generate_project(
    output_dir: &str,
    mut object_database: &mut ObjectDatabase,
    config: &Config,
    spec: &oas3::Spec,
) {
    let generated_paths = generate_paths(output_dir, &spec, &mut object_database, &config)
        .expect("Failed to generated paths");

    write_object_database(output_dir, &object_database, &config.name_mapping)
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
