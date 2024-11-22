use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
};

use oas3::{
    spec::{ObjectOrReference, ObjectSchema},
    Spec,
};

use crate::utils::name_mapper::NameMapper;

pub type StructDatabase = HashMap<String, StructDefinition>;

#[derive(Clone, Debug, PartialEq)]
pub struct ModuleInfo {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeDefinition {
    pub name: String,
    pub module: Option<ModuleInfo>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PropertyDefinition {
    pub name: String,
    pub real_name: String,
    pub type_name: String,
    pub module: Option<ModuleInfo>,
    pub required: bool,
}

#[derive(Clone, PartialEq)]
pub struct StructDefinition {
    pub used_modules: Vec<ModuleInfo>,
    pub name: String,
    pub properties: HashMap<String, PropertyDefinition>,
}

pub fn modules_to_string(modules: &Vec<&ModuleInfo>) -> String {
    let mut module_import_string = String::new();
    let mut unique_modules: Vec<&ModuleInfo> = vec![];
    for module in modules {
        if unique_modules.contains(&module) {
            continue;
        }
        unique_modules.push(&module);
        module_import_string += format!("use {}::{};\n", module.path, module.name).as_str();
    }
    module_import_string
}

impl StructDefinition {
    pub fn get_required_modules(&self) -> Vec<&ModuleInfo> {
        let mut required_modules = self.used_modules.iter().collect::<Vec<&ModuleInfo>>();
        required_modules.append(
            &mut self
                .properties
                .iter()
                .filter_map(|(_, property)| property.module.as_ref())
                .collect::<Vec<&ModuleInfo>>(),
        );
        required_modules
    }

    pub fn to_string(&self, serializable: bool) -> String {
        let mut definition_str = String::new();

        definition_str += match serializable {
            true => "#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]\n",
            _ => "",
        };
        definition_str += format!("pub struct {} {{\n\n", self.name).as_str();

        for (_, property) in &self.properties {
            if property.name != property.real_name && serializable {
                definition_str +=
                    format!("#[serde(alias = \"{}\")]\n", property.real_name).as_str();
            }

            match property.required {
                true => {
                    definition_str +=
                        format!("pub {}: {},\n", property.name, property.type_name).as_str()
                }
                false => {
                    definition_str +=
                        format!("pub {}: Option<{}>,\n", property.name, property.type_name).as_str()
                }
            }
        }

        definition_str += "}";
        definition_str
    }
}

pub fn generate_components(
    spec: &Spec,
    name_mapper: &NameMapper,
) -> Result<StructDatabase, String> {
    let components = match spec.components {
        Some(ref components) => components,
        None => return Ok(StructDatabase::new()),
    };

    let mut struct_database = StructDatabase::new();

    for (name, object_ref) in &components.schemas {
        let resolved_object = match object_ref.resolve(spec) {
            Ok(object) => object,
            Err(err) => {
                println!("Unable to parse component {} {}", name, err.to_string());
                continue;
            }
        };

        let _ = match generate_object(
            spec,
            &mut struct_database,
            &name,
            &resolved_object,
            name_mapper,
        ) {
            Ok(struct_definition) => {
                struct_database.insert(struct_definition.name.clone(), struct_definition)
            }
            Err(err) => {
                println!("{}", err);
                None
            }
        };
    }

    Ok(struct_database)
}

pub fn write_struct_database(
    struct_database: &StructDatabase,
    name_mapper: &NameMapper,
) -> Result<(), String> {
    for (_, struct_definition) in struct_database {
        let module_name = name_mapper.name_to_module_name(&struct_definition.name);
        fs::create_dir_all("output/objects").expect("Creating objects dir failed");
        let mut object_file = match File::create(format!("output/src/objects/{}.rs", module_name)) {
            Ok(file) => file,
            Err(err) => {
                println!(
                    "Unable to create file {}.rs {}",
                    module_name,
                    err.to_string()
                );
                continue;
            }
        };

        object_file
            .write(modules_to_string(&struct_definition.get_required_modules()).as_bytes())
            .expect("Failed to write imports");
        object_file.write("\n".as_bytes()).unwrap();

        object_file
            .write(struct_definition.to_string(true).as_bytes())
            .unwrap();
    }

    let mut object_mod_file = match File::create("output/src/objects/mod.rs") {
        Ok(file) => file,
        Err(err) => return Err(format!("Unable to create file mod.rs {}", err.to_string())),
    };

    for (struct_name, _) in struct_database {
        match object_mod_file.write(
            format!(
                "pub mod {};\n",
                name_mapper.name_to_module_name(struct_name)
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

pub fn generate_object(
    spec: &Spec,
    struct_database: &mut StructDatabase,
    name: &str,
    object_schema: &ObjectSchema,
    name_mapper: &NameMapper,
) -> Result<StructDefinition, String> {
    let mut struct_definition = StructDefinition {
        name: name_mapper.name_to_struct_name(name).to_owned(),
        properties: HashMap::new(),
        used_modules: vec![
            ModuleInfo {
                name: "Serialize".to_owned(),
                path: "serde".to_owned(),
            },
            ModuleInfo {
                name: "Deserialize".to_owned(),
                path: "serde".to_owned(),
            },
        ],
    };

    for (property_name, property_ref) in &object_schema.properties {
        let property_required = object_schema
            .required
            .iter()
            .any(|property| property == property_name);

        let property_definition = match get_or_create_property(
            spec,
            property_name,
            property_ref,
            property_required,
            struct_database,
            name_mapper,
        ) {
            Err(err) => {
                println!("{}", err);
                continue;
            }
            Ok(property_definition) => property_definition,
        };
        struct_definition
            .properties
            .insert(property_definition.name.clone(), property_definition);
    }

    Ok(struct_definition)
}

fn get_or_create_property(
    spec: &Spec,
    property_name: &String,
    property_ref: &ObjectOrReference<ObjectSchema>,
    required: bool,
    struct_database: &mut StructDatabase,
    name_mapper: &NameMapper,
) -> Result<PropertyDefinition, String> {
    let property = match property_ref.resolve(spec) {
        Ok(property) => property,
        Err(err) => {
            return Err(format!(
                "Failed to resolve {} {}",
                property_name,
                err.to_string()
            ))
        }
    };

    match get_type_from_schema(
        spec,
        struct_database,
        &property,
        Some(&property_name),
        name_mapper,
    ) {
        Ok(property_type_definition) => Ok(PropertyDefinition {
            type_name: property_type_definition.name,
            module: property_type_definition.module,
            name: name_mapper.name_to_property_name(property_name),
            real_name: property_name.clone(),
            required: required,
        }),
        Err(err) => Err(err),
    }
}

pub fn get_type_from_schema(
    spec: &Spec,
    struct_database: &mut StructDatabase,
    object_schema: &ObjectSchema,
    object_variable_fallback_name: Option<&str>,
    name_mapper: &NameMapper,
) -> Result<TypeDefinition, String> {
    let schema_type = match object_schema.schema_type.as_ref() {
        Some(schema_type) => schema_type,
        None => {
            return Err(format!("Object has no schema type"));
        }
    };

    let single_type = match schema_type {
        oas3::spec::SchemaTypeSet::Single(single_type) => single_type,
        _ => return Err(format!("MultiType is not supported")),
    };

    let mut object_variable_name = match object_schema.title {
        Some(ref title) => title,
        None => match object_variable_fallback_name {
            Some(title_fallback) => title_fallback,
            None => {
                return Err(format!(
                    "Cannot fetch type because no title or title_fallback was given"
                ))
            }
        },
    };

    match single_type {
        oas3::spec::SchemaType::Boolean => Ok(TypeDefinition {
            name: "bool".to_owned(),
            module: None,
        }),
        oas3::spec::SchemaType::String => Ok(TypeDefinition {
            name: "String".to_owned(),
            module: None,
        }),
        oas3::spec::SchemaType::Number => Ok(TypeDefinition {
            name: "f64".to_owned(),
            module: None,
        }),
        oas3::spec::SchemaType::Integer => Ok(TypeDefinition {
            name: "i32".to_owned(),
            module: None,
        }),
        oas3::spec::SchemaType::Array => {
            let item_object_ref = match object_schema.items {
                Some(ref item_object) => item_object,
                None => return Err(format!("Array has no item type")),
            };

            object_variable_name = match **item_object_ref {
                ObjectOrReference::Ref { ref ref_path } => {
                    let path_segments = ref_path.split("/");
                    match path_segments.last() {
                        Some(ref_name) => ref_name,
                        None => return Err(format!("ArrayItem ref has no _ref defined")),
                    }
                }
                _ => object_variable_name,
            };

            let item_object = match item_object_ref.resolve(spec) {
                Ok(item_object) => item_object,
                Err(err) => {
                    return Err(format!(
                        "Failed to resolve ArrayItem\n{:#?}\n{}",
                        item_object_ref,
                        err.to_string()
                    ))
                }
            };

            match get_type_from_schema(
                spec,
                struct_database,
                &item_object,
                Some(&object_variable_name),
                name_mapper,
            ) {
                Ok(mut type_definition) => {
                    type_definition.name = format!("Vec<{}>", type_definition.name);
                    return Ok(type_definition);
                }
                Err(err) => Err(err),
            }
        }
        oas3::spec::SchemaType::Object => {
            let struct_definition = match get_or_create_object(
                spec,
                struct_database,
                &object_variable_name,
                &object_schema,
                name_mapper,
            ) {
                Ok(struct_definition) => struct_definition,
                Err(err) => {
                    return Err(format!(
                        "Failed to generated struct {} {}",
                        object_variable_name, err
                    ));
                }
            };

            Ok(TypeDefinition {
                name: struct_definition.name.clone(),
                module: Some(ModuleInfo {
                    path: format!(
                        "crate::objects::{}",
                        name_mapper.name_to_module_name(&struct_definition.name)
                    ),
                    name: struct_definition.name.clone(),
                }),
            })
        }
        _ => Err(format!("Type {:?} not supported", single_type)),
    }
}

fn get_or_create_object(
    spec: &Spec,
    struct_database: &mut StructDatabase,
    name: &str,
    property_ref: &ObjectSchema,
    name_mapper: &NameMapper,
) -> Result<StructDefinition, String> {
    if name_mapper.name_to_struct_name(name) == "ChildGeometries" {
        println!("{:#?}", property_ref);
        panic!("ChildGeometries!")
    }

    let struct_in_database_opt = match struct_database.get(&name_mapper.name_to_struct_name(name)) {
        Some(struct_in_database) => Some(struct_in_database),
        None => {
            // create shallow hull which will be filled in later
            // the hull is needed to reference for cyclic dependencies where we would
            // otherwise create the same object every time we want to resolve the current one
            let struct_name = name_mapper.name_to_struct_name(name);
            struct_database.insert(
                struct_name.clone(),
                StructDefinition {
                    used_modules: vec![],
                    name: struct_name.clone(),
                    properties: HashMap::new(),
                },
            );

            match generate_object(
                spec,
                struct_database,
                &struct_name,
                property_ref,
                name_mapper,
            ) {
                Ok(created_struct) => {
                    let name = created_struct.name.clone();
                    struct_database.insert(name.clone(), created_struct);
                    struct_database.get(&name)
                }
                Err(_) => None,
            }
        }
    };
    match struct_in_database_opt {
        Some(struct_in_database) => Ok(struct_in_database.clone()),
        None => Err(format!("Struct {} not found", name)),
    }
}
