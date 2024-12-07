use std::{
    collections::HashMap,
    fmt::format,
    fs::{self, File},
    io::Write,
};

use log::{error, info, trace};
use oas3::{
    spec::{ObjectOrReference, ObjectSchema, SchemaTypeSet},
    Spec,
};

use crate::utils::{config::Config, name_mapping::NameMapping};

pub type ObjectDatabase = HashMap<String, ObjectDefinition>;

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

#[derive(Clone, Debug, PartialEq)]
pub enum ObjectDefinition {
    Struct(StructDefinition),
    Enum(EnumDefinition),
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumValue {
    pub name: String,
    pub value_type: TypeDefinition,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumDefinition {
    pub name: String,
    pub used_modules: Vec<ModuleInfo>,
    pub values: HashMap<String, EnumValue>,
}

impl EnumDefinition {
    pub fn get_required_modules(&self) -> Vec<&ModuleInfo> {
        let mut required_modules = self.used_modules.iter().collect::<Vec<&ModuleInfo>>();
        required_modules.append(
            &mut self
                .values
                .iter()
                .filter_map(|(_, enum_value)| enum_value.value_type.module.as_ref())
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
        definition_str += format!("pub enum {} {{\n\n", self.name).as_str();

        for (_, enum_value) in &self.values {
            definition_str +=
                format!("{}({}),\n", enum_value.name, enum_value.value_type.name).as_str()
        }

        definition_str += "}";
        definition_str
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructDefinition {
    pub used_modules: Vec<ModuleInfo>,
    pub name: String,
    pub properties: HashMap<String, PropertyDefinition>,
    pub local_objects: HashMap<String, Box<ObjectDefinition>>,
}

pub fn get_object_name(object_definition: &ObjectDefinition) -> &String {
    match object_definition {
        ObjectDefinition::Struct(struct_definition) => &struct_definition.name,
        ObjectDefinition::Enum(enum_definition) => &enum_definition.name,
    }
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

pub fn get_components_base_path() -> Vec<String> {
    vec![
        String::from("#"),
        String::from("components"),
        String::from("schemas"),
    ]
}

pub fn generate_components(spec: &Spec, config: &Config) -> Result<ObjectDatabase, String> {
    let components = match spec.components {
        Some(ref components) => components,
        None => return Ok(ObjectDatabase::new()),
    };

    let mut object_database = ObjectDatabase::new();

    for (component_name, object_ref) in &components.schemas {
        if config.ignore.component_ignored(&component_name) {
            info!("\"{}\" ignored", component_name);
            continue;
        }

        info!("Generating component \"{}\"", component_name);

        let resolved_object = match object_ref.resolve(spec) {
            Ok(object) => object,
            Err(err) => {
                error!(
                    "Unable to parse component {} {}",
                    component_name,
                    err.to_string()
                );
                continue;
            }
        };

        let definition_path = get_components_base_path();
        let object_name = match resolved_object.title {
            Some(ref title) => config
                .name_mapping
                .name_to_struct_name(&definition_path, &title),
            None => config
                .name_mapping
                .name_to_struct_name(&definition_path, &component_name),
        };

        if object_database.contains_key(&object_name) {
            info!(
                "Component \"{}\" already found in database and will be skipped",
                object_name
            );
            continue;
        }

        let _ = match generate_object(
            spec,
            &mut object_database,
            definition_path,
            &object_name,
            &resolved_object,
            &config.name_mapping,
        ) {
            Ok(struct_definition) => {
                let object_name = get_object_name(&struct_definition);

                match object_database.contains_key(object_name) {
                    true => {
                        error!("ObjectDatabase already contains an object {}", object_name);
                        None
                    }
                    _ => {
                        trace!("Adding component/struct {} to database", object_name);
                        object_database.insert(object_name.clone(), struct_definition)
                    }
                }
            }
            Err(err) => {
                error!("{} {}\n", component_name, err);
                None
            }
        };
    }

    Ok(object_database)
}

pub fn write_object_database(
    output_dir: &str,
    object_database: &ObjectDatabase,
    name_mapping: &NameMapping,
) -> Result<(), String> {
    fs::create_dir_all(format!("{}/src/objects/", output_dir))
        .expect("Creating objects dir failed");

    for (_, object_definition) in object_database {
        let object_name = match object_definition {
            ObjectDefinition::Struct(object_definition) => &object_definition.name,
            ObjectDefinition::Enum(enum_definition) => &enum_definition.name,
        };

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

        match object_definition {
            ObjectDefinition::Struct(struct_definition) => {
                object_file
                    .write(modules_to_string(&struct_definition.get_required_modules()).as_bytes())
                    .expect("Failed to write imports");
                object_file.write("\n".as_bytes()).unwrap();

                object_file
                    .write(struct_definition.to_string(true).as_bytes())
                    .unwrap();
            }
            ObjectDefinition::Enum(enum_definition) => {
                object_file
                    .write(modules_to_string(&enum_definition.get_required_modules()).as_bytes())
                    .expect("Failed to write imports");
                object_file.write("\n".as_bytes()).unwrap();

                object_file
                    .write(enum_definition.to_string(true).as_bytes())
                    .unwrap();
            }
        }
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

pub fn generate_object(
    spec: &Spec,
    object_database: &mut ObjectDatabase,
    definition_path: Vec<String>,
    name: &str,
    object_schema: &ObjectSchema,
    name_mapping: &NameMapping,
) -> Result<ObjectDefinition, String> {
    match object_schema.any_of.len() {
        0 => generate_struct(
            spec,
            object_database,
            definition_path,
            name,
            object_schema,
            name_mapping,
        ),
        _ => generate_enum(
            spec,
            object_database,
            definition_path,
            name,
            object_schema,
            name_mapping,
        ),
    }
}

fn oas3_type_to_string(oas3_type: &oas3::spec::SchemaType) -> String {
    match oas3_type {
        oas3::spec::SchemaType::Boolean => String::from("Boolean"),
        oas3::spec::SchemaType::Integer => String::from("Integer"),
        oas3::spec::SchemaType::Number => String::from("Number"),
        oas3::spec::SchemaType::String => String::from("String"),
        oas3::spec::SchemaType::Array => String::from("Array"),
        oas3::spec::SchemaType::Object => String::from("Object"),
        oas3::spec::SchemaType::Null => String::from("Null"),
    }
}

pub fn get_object_or_ref_struct_name(
    spec: &Spec,
    definition_path: &Vec<String>,
    name_mapping: &NameMapping,
    object_or_reference: &ObjectOrReference<ObjectSchema>,
) -> Result<(Vec<String>, String), String> {
    let object_schema = match object_or_reference {
        ObjectOrReference::Ref { ref_path } => {
            let ref_definition_path = match get_base_path_to_ref(ref_path) {
                Ok(ref_path) => ref_path,
                Err(err) => return Err(err),
            };

            match object_or_reference.resolve(spec) {
                Ok(object_schema) => match object_schema.title {
                    Some(ref ref_title) => {
                        return Ok((
                            ref_definition_path.clone(),
                            name_mapping.name_to_struct_name(&ref_definition_path, ref_title),
                        ));
                    }
                    None => {
                        let path_name = match ref_path.split("/").last() {
                            Some(last_name) => last_name,
                            None => {
                                return Err(format!(
                                    "Unable to retrieve name from ref path {}",
                                    ref_path
                                ))
                            }
                        };

                        return Ok((
                            ref_definition_path.clone(),
                            name_mapping.name_to_struct_name(&ref_definition_path, path_name),
                        ));
                    }
                },

                Err(err) => return Err(format!("Failed to resolve object {}", err.to_string())),
            }
        }
        ObjectOrReference::Object(object_schema) => object_schema,
    };

    if let Some(ref title) = object_schema.title {
        return Ok((
            definition_path.clone(),
            name_mapping.name_to_struct_name(definition_path, &title),
        ));
    }

    if let Some(ref schema_type) = object_schema.schema_type {
        let type_name = match schema_type {
            SchemaTypeSet::Single(single_type) => oas3_type_to_string(single_type),
            SchemaTypeSet::Multiple(multiple_types) => multiple_types
                .iter()
                .map(oas3_type_to_string)
                .collect::<Vec<String>>()
                .join(""),
        };

        return Ok((
            definition_path.clone(),
            name_mapping.name_to_struct_name(definition_path, &type_name),
        ));
    }

    Err(format!("Unable to determine object name"))
}

pub fn get_base_path_to_ref(ref_path: &str) -> Result<Vec<String>, String> {
    let mut path_segments = ref_path
        .split("/")
        .map(|segment| segment.to_owned())
        .collect::<Vec<String>>();
    if path_segments.len() < 4 {
        return Err(format!("Expected 4 path segments in {}", ref_path));
    }
    // Remove component name
    path_segments.pop();
    Ok(path_segments)
}

pub fn generate_enum(
    spec: &Spec,
    object_database: &mut ObjectDatabase,
    mut definition_path: Vec<String>,
    name: &str,
    object_schema: &ObjectSchema,
    name_mapping: &NameMapping,
) -> Result<ObjectDefinition, String> {
    trace!("Generating enum");
    let mut enum_definition = EnumDefinition {
        name: name_mapping
            .name_to_struct_name(&definition_path, name)
            .to_owned(),
        values: HashMap::new(),
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
    definition_path.push(enum_definition.name.clone());

    for any_object_ref in &object_schema.any_of {
        trace!("Generating enum value");
        let (any_object_definition_path, any_object) = match any_object_ref {
            ObjectOrReference::Ref { ref_path } => match any_object_ref.resolve(spec) {
                Err(err) => {
                    error!("{} {}", name, err);
                    continue;
                }
                Ok(object_schema) => {
                    let ref_definition_path = match get_base_path_to_ref(ref_path) {
                        Ok(base_path) => base_path,
                        Err(err) => {
                            error!("Unable to retrieve ref path {}", err);
                            continue;
                        }
                    };
                    (ref_definition_path, object_schema)
                }
            },
            ObjectOrReference::Object(object_schema) => {
                (definition_path.clone(), object_schema.clone())
            }
        };

        let object_type_enum_name = match get_object_or_ref_struct_name(
            spec,
            &any_object_definition_path,
            name_mapping,
            any_object_ref,
        ) {
            Ok((_, object_type_struct_name)) => name_mapping.name_to_struct_name(
                &any_object_definition_path,
                &format!("{}Value", object_type_struct_name),
            ),
            Err(err) => {
                return Err(format!(
                    "{} Anonymous enum value are not supported \"{}\"",
                    name, err
                ))
            }
        };

        enum_definition.values.insert(
            object_type_enum_name.clone(),
            match get_type_from_schema(
                spec,
                object_database,
                any_object_definition_path.clone(),
                &any_object,
                Some(&object_type_enum_name),
                name_mapping,
            ) {
                Ok(type_definition) => EnumValue {
                    name: object_type_enum_name,
                    value_type: type_definition,
                },
                Err(err) => {
                    info!("{} {}", name, err);
                    continue;
                }
            },
        );
    }
    Ok(ObjectDefinition::Enum(enum_definition))
}

pub fn generate_struct(
    spec: &Spec,
    object_database: &mut ObjectDatabase,
    mut definition_path: Vec<String>,
    name: &str,
    object_schema: &ObjectSchema,
    name_mapping: &NameMapping,
) -> Result<ObjectDefinition, String> {
    trace!("Generating struct");
    let mut struct_definition = StructDefinition {
        name: name_mapping
            .name_to_struct_name(&definition_path, name)
            .to_owned(),
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
        local_objects: HashMap::new(),
    };
    definition_path.push(struct_definition.name.clone());

    for (property_name, property_ref) in &object_schema.properties {
        let property_required = object_schema
            .required
            .iter()
            .any(|property| property == property_name);

        let property_definition = match get_or_create_property(
            spec,
            definition_path.clone(),
            property_name,
            property_ref,
            property_required,
            object_database,
            name_mapping,
        ) {
            Err(err) => {
                info!("{} {}", name, err);
                continue;
            }
            Ok(property_definition) => property_definition,
        };
        struct_definition
            .properties
            .insert(property_definition.name.clone(), property_definition);
    }

    Ok(ObjectDefinition::Struct(struct_definition))
}

fn get_or_create_property(
    spec: &Spec,
    definition_path: Vec<String>,
    property_name: &String,
    property_ref: &ObjectOrReference<ObjectSchema>,
    required: bool,
    object_database: &mut ObjectDatabase,
    name_mapping: &NameMapping,
) -> Result<PropertyDefinition, String> {
    trace!("Creating property {}", property_name);
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

    let (property_type_definition_path, property_type_name) =
        match get_object_or_ref_struct_name(spec, &definition_path, name_mapping, property_ref) {
            Ok(type_naming_data) => type_naming_data,
            Err(err) => {
                return Err(format!(
                    "Unable to determine property name of {} {}",
                    property_name, err
                ))
            }
        };

    match get_type_from_schema(
        spec,
        object_database,
        property_type_definition_path,
        &property,
        Some(&property_type_name),
        name_mapping,
    ) {
        Ok(property_type_definition) => Ok(PropertyDefinition {
            type_name: property_type_definition.name,
            module: property_type_definition.module,
            name: name_mapping.name_to_property_name(&definition_path, property_name),
            real_name: property_name.clone(),
            required: required,
        }),
        Err(err) => Err(err),
    }
}

pub fn get_type_from_schema(
    spec: &Spec,
    object_database: &mut ObjectDatabase,
    definition_path: Vec<String>,
    object_schema: &ObjectSchema,
    object_variable_fallback_name: Option<&str>,
    name_mapping: &NameMapping,
) -> Result<TypeDefinition, String> {
    if let Some(ref schema_type) = object_schema.schema_type {
        return get_type_from_schema_type(
            spec,
            object_database,
            definition_path,
            schema_type,
            object_schema,
            object_variable_fallback_name,
            name_mapping,
        );
    }

    if object_schema.any_of.len() > 0 {
        return get_type_from_any_type(
            spec,
            object_database,
            definition_path,
            object_schema,
            object_variable_fallback_name,
            name_mapping,
        );
    }

    let empty_object_name = match object_variable_fallback_name {
        Some(empty_object_name) => empty_object_name,
        None => return Err("Cannot create empty object without name".to_owned()),
    };

    // empty type
    match get_or_create_object(
        spec,
        object_database,
        definition_path,
        empty_object_name,
        object_schema,
        name_mapping,
    ) {
        Ok(object_definition) => {
            let object_name = get_object_name(&object_definition);
            Ok(TypeDefinition {
                name: object_name.clone(),
                module: Some(ModuleInfo {
                    path: format!(
                        "crate::objects::{}",
                        name_mapping.name_to_module_name(&object_name)
                    ),
                    name: object_name.clone(),
                }),
            })
        }
        Err(err) => Err(err),
    }
}

pub fn get_type_from_any_type(
    spec: &Spec,
    object_database: &mut ObjectDatabase,
    definition_path: Vec<String>,
    object_schema: &ObjectSchema,
    object_variable_fallback_name: Option<&str>,
    name_mapping: &NameMapping,
) -> Result<TypeDefinition, String> {
    let object_variable_name = match object_schema.title {
        Some(ref title) => &name_mapping.name_to_struct_name(&definition_path, &title),
        None => match object_variable_fallback_name {
            Some(title_fallback) => title_fallback,
            None => {
                return Err(format!(
                    "Cannot fetch type because no title or title_fallback was given"
                ))
            }
        },
    };

    trace!("Generating any_type {}", object_variable_name);

    let object_definition = match get_or_create_object(
        spec,
        object_database,
        definition_path,
        &object_variable_name,
        &object_schema,
        name_mapping,
    ) {
        Ok(object_definition) => object_definition,
        Err(err) => {
            return Err(format!(
                "Failed to generated struct {} {}",
                object_variable_name, err
            ));
        }
    };

    let object_name = get_object_name(&object_definition);

    Ok(TypeDefinition {
        name: object_name.clone(),
        module: Some(ModuleInfo {
            path: format!(
                "crate::objects::{}",
                name_mapping.name_to_module_name(&object_name)
            ),
            name: object_name.clone(),
        }),
    })
}

pub fn get_type_from_schema_type(
    spec: &Spec,
    object_database: &mut ObjectDatabase,
    definition_path: Vec<String>,
    schema_type: &SchemaTypeSet,
    object_schema: &ObjectSchema,
    object_variable_fallback_name: Option<&str>,
    name_mapping: &NameMapping,
) -> Result<TypeDefinition, String> {
    let single_type = match schema_type {
        oas3::spec::SchemaTypeSet::Single(single_type) => single_type,
        _ => return Err(format!("MultiType is not supported")),
    };

    let object_variable_name = match object_schema.title {
        Some(ref title) => title,
        None => match object_variable_fallback_name {
            Some(title_fallback) => title_fallback,
            None => {
                return Err(format!(
                    "Cannot fetch type because no title or title_fallback was given {:#?}",
                    object_schema
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

            let (item_type_definition_path, item_type_name) = match get_object_or_ref_struct_name(
                spec,
                &definition_path,
                name_mapping,
                &item_object_ref,
            ) {
                Ok(definition_path_and_name) => definition_path_and_name,
                Err(err) => return Err(format!("Unable to determine ArrayItem type name {}", err)),
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
                object_database,
                item_type_definition_path,
                &item_object,
                Some(&item_type_name),
                name_mapping,
            ) {
                Ok(mut type_definition) => {
                    type_definition.name = format!("Vec<{}>", type_definition.name);
                    return Ok(type_definition);
                }
                Err(err) => Err(err),
            }
        }
        oas3::spec::SchemaType::Object => {
            let object_definition = match get_or_create_object(
                spec,
                object_database,
                definition_path,
                &object_variable_name,
                &object_schema,
                name_mapping,
            ) {
                Ok(object_definition) => object_definition,
                Err(err) => {
                    return Err(format!(
                        "Failed to generated struct {} {}",
                        object_variable_name, err
                    ));
                }
            };

            let object_name = get_object_name(&object_definition);

            Ok(TypeDefinition {
                name: object_name.clone(),
                module: Some(ModuleInfo {
                    path: format!(
                        "crate::objects::{}",
                        name_mapping.name_to_module_name(&object_name)
                    ),
                    name: object_name.clone(),
                }),
            })
        }
        _ => Err(format!("Type {:?} not supported", single_type)),
    }
}

fn get_or_create_object(
    spec: &Spec,
    object_database: &mut ObjectDatabase,
    definition_path: Vec<String>,
    name: &str,
    property_ref: &ObjectSchema,
    name_mapping: &NameMapping,
) -> Result<ObjectDefinition, String> {
    let struct_in_database_opt = match object_database
        .get(&name_mapping.name_to_struct_name(&definition_path, name))
    {
        Some(struct_in_database) => Some(struct_in_database),
        None => {
            // create shallow hull which will be filled in later
            // the hull is needed to reference for cyclic dependencies where we would
            // otherwise create the same object every time we want to resolve the current one
            let struct_name = name_mapping.name_to_struct_name(&definition_path, name);
            if object_database.contains_key(&struct_name) {
                return Err(format!(
                    "ObjectDatabase already contains an object {}",
                    struct_name
                ));
            }

            trace!("Adding struct {} to database", struct_name);

            object_database.insert(
                struct_name.clone(),
                ObjectDefinition::Struct(StructDefinition {
                    used_modules: vec![],
                    name: struct_name.clone(),
                    properties: HashMap::new(),
                    local_objects: HashMap::new(),
                }),
            );

            match generate_object(
                spec,
                object_database,
                definition_path,
                &struct_name,
                property_ref,
                name_mapping,
            ) {
                Ok(created_struct) => {
                    let name = match created_struct {
                        ObjectDefinition::Struct(ref struct_definition) => {
                            struct_definition.name.clone()
                        }
                        ObjectDefinition::Enum(ref enum_definition) => enum_definition.name.clone(),
                    };
                    trace!("Updating struct {} in database", name);
                    object_database.insert(name.clone(), created_struct);
                    object_database.get(&name)
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
