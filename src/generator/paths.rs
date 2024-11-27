use std::{
    fs::{self, File},
    io::Write,
};

use oas3::{spec::Operation, Spec};

use crate::utils::{config::Config, name_mapping::NameMapping};

use super::{
    component::ObjectDatabase,
    path::{default_request, websocket_request},
};

pub fn generate_paths(
    output_path: &str,
    spec: &Spec,
    object_database: &mut ObjectDatabase,
    config: &Config,
) {
    let paths = match spec.paths {
        Some(ref paths) => paths,
        None => return (),
    };

    fs::create_dir_all(format!("{}/src/paths", output_path)).expect("Creating objects dir failed");

    let mut mod_file = match File::create(format!("{}/src/paths/mod.rs", output_path)) {
        Ok(file) => file,
        Err(err) => {
            println!("Unable to create file mod.rs {}", err.to_string());
            return;
        }
    };

    for (name, path_item) in paths {
        if config.ignore.path_ignored(&name) {
            println!("{} ignored", name);
            continue;
        }

        println!("{}", name);

        let mut operations = vec![];
        if let Some(ref operation) = path_item.get {
            operations.push((reqwest::Method::GET, operation));
        }
        if let Some(ref operation) = path_item.post {
            operations.push((reqwest::Method::POST, operation));
        }
        if let Some(ref operation) = path_item.delete {
            operations.push((reqwest::Method::DELETE, operation));
        }
        if let Some(ref operation) = path_item.put {
            operations.push((reqwest::Method::PUT, operation));
        }
        if let Some(ref operation) = path_item.patch {
            operations.push((reqwest::Method::PATCH, operation));
        }

        for operation in operations {
            match write_operation_to_file(
                spec,
                &operation.0,
                &name,
                operation.1,
                object_database,
                &config.name_mapping,
            ) {
                Ok(operation_id) => {
                    mod_file
                        .write(format!("pub mod {};\n", operation_id).as_bytes())
                        .expect("Failed to write to mod.rs");
                    ()
                }
                Err(err) => {
                    println!("{}", err);
                }
            }
        }
    }
}

fn write_operation_to_file(
    spec: &Spec,
    method: &reqwest::Method,
    path: &str,
    operation: &Operation,
    object_database: &mut ObjectDatabase,
    name_mapping: &NameMapping,
) -> Result<String, String> {
    let operation_id = match operation.operation_id {
        Some(ref operation_id) => &name_mapping.name_to_module_name(operation_id),
        None => {
            return Err(format!("{} get has no id", path));
        }
    };

    let generate_websocket = match operation.extensions.get("serverstream") {
        Some(extension_value) => match extension_value {
            serde_json::Value::Bool(generate_websocket) => generate_websocket,
            _ => return Err("Invalid x-serverstream value".to_owned()),
        },
        None => &false,
    };

    let request_code = match generate_websocket {
        true => match websocket_request::generate_operation(
            spec,
            name_mapping,
            &path,
            &operation,
            object_database,
        ) {
            Ok(request_code) => request_code,
            Err(err) => return Err(format!("Failed to generated websocket code {}", err)),
        },
        _ => match default_request::generate_operation(
            spec,
            name_mapping,
            method,
            &path,
            &operation,
            object_database,
        ) {
            Ok(request_code) => request_code,
            Err(err) => {
                return Err(format!("Failed to generate code {}", err));
            }
        },
    };

    let mut object_file = match File::create(format!("output/src/paths/{}.rs", operation_id)) {
        Ok(file) => file,
        Err(err) => {
            return Err(format!(
                "Unable to create file {}.rs {}",
                operation_id,
                err.to_string()
            ));
        }
    };

    object_file.write(request_code.as_bytes()).unwrap();
    Ok(operation_id.clone())
}
