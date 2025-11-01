use std::{
    fs::{self, File},
    io::Write,
};

use askama::Template;
use log::error;

use crate::{
    generator::rust_reqwest_async::templates::BaseTemplate,
    parser::component::object_definition::{
        get_object_name,
        types::{ObjectDatabase, ObjectDefinition},
    },
    utils::name_mapping::NameMapping,
};

pub fn write_object_database(
    output_dir: &str,
    object_database: &ObjectDatabase,
    name_mapping: &NameMapping,
) -> Result<(), String> {
    fs::create_dir_all(format!("{}/src/objects/", output_dir))
        .expect("Creating objects dir failed");

    for (_, object_definition) in object_database {
        let object_name = get_object_name(object_definition);

        let module_name = name_mapping.name_to_module_name(object_name);

        let mut object_file =
            match File::create(format!("{}/src/objects/{}.rs", output_dir, module_name)) {
                Ok(file) => file,
                Err(err) => {
                    error!(
                        "Unable to create file {}.rs {}",
                        module_name,
                        err.to_string()
                    );
                    continue;
                }
            };

        let template: BaseTemplate = match object_definition {
            ObjectDefinition::Struct(struct_definition) => struct_definition.into(),
            ObjectDefinition::Enum(enum_definition) => enum_definition.into(),
            ObjectDefinition::Primitive(primitive_definition) => primitive_definition.into(),
        };

        let rendered_template = match template.render() {
            Ok(rendered_template) => rendered_template,
            Err(err) => {
                error!(
                    "Failed to render object template {} {}",
                    object_name,
                    err.to_string()
                );
                continue;
            }
        };

        object_file
            .write(rendered_template.as_bytes())
            .map_err(|err| {
                format!(
                    "Failed to write to object file {}.rs {}",
                    module_name,
                    err.to_string()
                )
            })?;
    }

    let mut object_mod_file = match File::create(format!("{}/src/objects/mod.rs", output_dir)) {
        Ok(file) => file,
        Err(err) => {
            return Err(format!(
                "Unable to create file {} {}",
                format!("{}/src/objects/mod.rs", output_dir),
                err.to_string()
            ))
        }
    };

    for (struct_name, _) in object_database {
        match object_mod_file.write(
            format!(
                "pub mod {};\n",
                name_mapping.name_to_module_name(struct_name)
            )
            .to_string()
            .as_bytes(),
        ) {
            Ok(_) => (),
            Err(err) => return Err(format!("Failed to write to mod {}", err.to_string())),
        }
    }
    Ok(())
}
