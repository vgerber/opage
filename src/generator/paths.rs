use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, File},
    io::Write,
};

use oas3::{
    spec::{ObjectOrReference, Operation, ParameterIn, RequestBody, Response},
    Spec,
};
use reqwest::StatusCode;

use crate::{
    generator::component::{get_type_from_schema, PropertyDefinition},
    utils::name_mapper::NameMapper,
};

use super::component::{ModuleInfo, StructDatabase, StructDefinition, TypeDefinition};

pub fn generate_paths(spec: &Spec, struct_database: &mut StructDatabase, name_mapper: &NameMapper) {
    let paths = match spec.paths {
        Some(ref paths) => paths,
        None => return (),
    };

    fs::create_dir_all("output/src/paths").expect("Creating objects dir failed");

    let mut mod_file = match File::create(format!("output/src/paths/mod.rs")) {
        Ok(file) => file,
        Err(err) => {
            println!("Unable to create file mod.rs {}", err.to_string());
            return;
        }
    };

    for (name, path_item) in paths {
        // if name != "/cells/{cell}/motion-groups/{motion_group}" {
        //     continue;
        // }
        println!("{}", name);
        // println!("{:#?}", path_item);

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
                struct_database,
                name_mapper,
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
    struct_database: &mut StructDatabase,
    name_mapper: &NameMapper,
) -> Result<String, String> {
    let operation_id = match operation.operation_id {
        Some(ref operation_id) => &name_mapper.name_to_module_name(operation_id),
        None => {
            return Err(format!("{} get has no id", path));
        }
    };

    let request_code = match generate_operation(
        spec,
        name_mapper,
        method,
        &path,
        &operation,
        struct_database,
    ) {
        Ok(request_code) => request_code,
        Err(err) => {
            return Err(format!("Failed to generate code {}", err));
        }
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

fn is_path_parameter(path_component: &str) -> bool {
    path_component.starts_with("{") && path_component.ends_with("}")
}

#[derive(Clone, Debug)]
enum TransferMediaType {
    ApplicationJson(TypeDefinition),
}

#[derive(Clone, Debug)]
struct ResponseEntity {
    canonical_status_code: String,
    content: Option<TransferMediaType>,
}

#[derive(Clone, Debug)]
struct RequestEntity {
    content: TransferMediaType,
}

type ResponseEntities = HashMap<String, ResponseEntity>;

fn generate_request_body(
    spec: &Spec,
    struct_database: &mut StructDatabase,
    name_mapper: &NameMapper,
    request_body: &ObjectOrReference<RequestBody>,
    function_name: &str,
) -> Result<RequestEntity, String> {
    let request = match request_body.resolve(spec) {
        Ok(request) => request,
        Err(err) => {
            return Err(format!(
                "Failed to resolve request body {}",
                err.to_string()
            ))
        }
    };

    if request.content.len() > 1 {
        println!("Only a single json object is supported");
    }

    let json_data = match request.content.get("application/json") {
        Some(json_data) => json_data,
        None => return Err("No json payload found".to_string()),
    };
    let json_object_definition_opt = match json_data.schema {
        Some(ref object_or_ref) => match object_or_ref {
            ObjectOrReference::Ref { ref_path } => match ref_path.split("/").last() {
                Some(object_name) => Some(TypeDefinition {
                    module: Some(ModuleInfo {
                        path: format!(
                            "crate::objects::{}",
                            name_mapper.name_to_module_name(object_name)
                        ),
                        name: name_mapper.name_to_struct_name(object_name).to_owned(),
                    }),
                    name: name_mapper.name_to_struct_name(object_name).to_owned(),
                }),
                None => None,
            },
            ObjectOrReference::Object(object_schema) => match get_type_from_schema(
                spec,
                struct_database,
                object_schema,
                Some(&format!(
                    "{}RequestBody",
                    name_mapper.name_to_struct_name(&function_name).to_owned(),
                )),
                name_mapper,
            ) {
                Ok(type_definition) => Some(type_definition),
                Err(err) => return Err(err),
            },
        },
        None => return Err(format!("Failed to parse response json data",)),
    };

    let json_object_definition = match json_object_definition_opt {
        Some(json_object_definition) => json_object_definition,
        None => return Err(format!("JsonObjectName not found")),
    };

    Ok(RequestEntity {
        content: TransferMediaType::ApplicationJson(json_object_definition),
    })
}

fn generate_responses(
    spec: &Spec,
    struct_database: &mut StructDatabase,
    name_mapper: &NameMapper,
    responses: &BTreeMap<String, Response>,
    function_name: &str,
) -> Result<ResponseEntities, String> {
    let mut response_entities = ResponseEntities::new();
    for (response_key, response) in responses {
        if response_key == "default" {
            continue;
        }

        let canonical_status_code = match StatusCode::from_bytes(response_key.as_bytes()) {
            Ok(status_code) => match status_code.canonical_reason() {
                Some(canonical_status_code) => canonical_status_code,
                None => {
                    return Err(format!(
                        "Failed to get canonical status code {}",
                        response_key
                    ))
                }
            },
            Err(err) => {
                return Err(format!(
                    "Failed to parse status code {} {}",
                    response_key,
                    err.to_string()
                ))
            }
        };

        if response.content.len() > 1 {
            println!("Only a single json object is supported");
        }

        if response.content.len() == 0 {
            response_entities.insert(
                response_key.clone(),
                ResponseEntity {
                    canonical_status_code: canonical_status_code.to_owned(),
                    content: None,
                },
            );
            continue;
        }

        let json_data = match response.content.get("application/json") {
            Some(json_data) => json_data,
            None => continue,
        };
        let json_object_definition_opt = match json_data.schema {
            Some(ref object_or_ref) => match object_or_ref {
                ObjectOrReference::Ref { ref_path } => match ref_path.split("/").last() {
                    Some(object_name) => Some(TypeDefinition {
                        module: Some(ModuleInfo {
                            path: format!(
                                "crate::objects::{}",
                                name_mapper.name_to_module_name(object_name)
                            ),
                            name: name_mapper.name_to_struct_name(object_name).to_owned(),
                        }),
                        name: name_mapper.name_to_struct_name(object_name).to_owned(),
                    }),
                    None => None,
                },
                ObjectOrReference::Object(object_schema) => match get_type_from_schema(
                    spec,
                    struct_database,
                    object_schema,
                    Some(&format!(
                        "{}{}",
                        name_mapper.name_to_struct_name(&function_name).to_owned(),
                        canonical_status_code
                    )),
                    name_mapper,
                ) {
                    Ok(type_definition) => Some(type_definition),
                    Err(err) => return Err(err),
                },
            },
            None => return Err(format!("Failed to parse response json data",)),
        };

        let json_object_definition = match json_object_definition_opt {
            Some(json_object_definition) => json_object_definition,
            None => return Err(format!("JsonObjectName not found")),
        };

        response_entities.insert(
            response_key.clone(),
            ResponseEntity {
                canonical_status_code: canonical_status_code.to_owned(),
                content: Some(TransferMediaType::ApplicationJson(json_object_definition)),
            },
        );
    }
    Ok(response_entities)
}

fn use_module_to_string(module: &ModuleInfo) -> String {
    if module.path.is_empty() {
        return format!("use {};", module.name);
    }
    format!("use {}::{};", module.path, module.name)
}

fn generate_operation(
    spec: &Spec,
    name_mapper: &NameMapper,
    method: &reqwest::Method,
    path: &str,
    operation: &Operation,
    struct_database: &mut StructDatabase,
) -> Result<String, String> {
    let function_name = match operation.operation_id {
        Some(ref operation_id) => name_mapper.name_to_module_name(operation_id),
        None => return Err("No operation_id found".to_owned()),
    };

    let response_entities = match generate_responses(
        spec,
        struct_database,
        name_mapper,
        &operation.responses(spec),
        &function_name,
    ) {
        Ok(response_entities) => response_entities,
        Err(err) => return Err(err),
    };

    let path_parameters_ordered = path
        .split("/")
        .filter(|&path_component| is_path_parameter(&path_component))
        .map(|path_component| path_component.replace("{", "").replace("}", ""))
        .map(|path_component| PropertyDefinition {
            module: None,
            name: name_mapper.name_to_property_name(&path_component),
            real_name: path_component,
            required: true,
            type_name: "&str".to_owned(),
        })
        .collect::<Vec<PropertyDefinition>>();
    let path_struct_definition = StructDefinition {
        name: format!(
            "{}PathParameters",
            name_mapper.name_to_struct_name(&function_name)
        ),
        used_modules: vec![],
        properties: path_parameters_ordered
            .iter()
            .map(|path_component| {
                (
                    path_component.name.clone(),
                    PropertyDefinition {
                        module: None,
                        name: path_component.name.clone(),
                        real_name: path_component.real_name.clone(),
                        required: path_component.required,
                        type_name: "String".to_owned(),
                    },
                )
            })
            .collect::<HashMap<String, PropertyDefinition>>(),
    };

    let path_format_string = path
        .split("/")
        .map(|path_component| {
            return match is_path_parameter(path_component) {
                true => String::from("{}"),
                _ => path_component.to_owned(),
            };
        })
        .collect::<Vec<String>>()
        .join("/");

    let response_enum_name = format!(
        "{}ResponseType",
        name_mapper.name_to_struct_name(&function_name)
    );

    let mut request_source_code = String::new();

    let mut function_parameters = vec![];
    
    if !path_struct_definition.properties.is_empty() {
        function_parameters.push(format!(
            "{}: &{}",
            name_mapper.name_to_property_name(&path_struct_definition.name),
            path_struct_definition.name
        ));
    }

    let mut module_imports = vec![ModuleInfo {
        name: "reqwest".to_owned(),
        path: String::new(),
    }];

    // Response types
    for (_, entity) in &response_entities {
        match entity.content {
            Some(ref content) => match content {
                TransferMediaType::ApplicationJson(ref type_definition) => {
                    match type_definition.module {
                        Some(ref module_info) => {
                            if module_imports.contains(module_info) {
                                continue;
                            }
                            module_imports.push(module_info.clone());
                        }
                        _ => (),
                    }
                }
            },
            None => (),
        }
    }

    let mut response_enum_source_code = format!("pub enum {} {{\n", response_enum_name);

    for (_, entity) in &response_entities {
        match entity.content {
            Some(ref content) => match content {
                TransferMediaType::ApplicationJson(ref type_definition) => {
                    response_enum_source_code += &format!(
                        "{}({}),\n",
                        name_mapper.name_to_struct_name(&entity.canonical_status_code),
                        type_definition.name,
                    )
                }
            },
            None => {
                response_enum_source_code += &format!(
                    "{},\n",
                    name_mapper.name_to_struct_name(&entity.canonical_status_code),
                )
            }
        }
    }

    response_enum_source_code += "UndefinedResponse(reqwest::Response),\n";
    response_enum_source_code += "}\n";

    // Query params
    let mut query_struct = StructDefinition {
        name: format!(
            "{}QueryParameters",
            name_mapper.name_to_struct_name(&function_name)
        ),
        properties: HashMap::new(),
        used_modules: vec![],
    };

    for parameter_ref in &operation.parameters {
        let parameter = match parameter_ref.resolve(spec) {
            Ok(parameter) => parameter,
            Err(err) => return Err(format!("Failed to resolve parameter {}", err.to_string())),
        };
        if parameter.location != ParameterIn::Query {
            continue;
        }

        let parameter_type = match parameter.schema {
            Some(schema) => match schema.resolve(spec) {
                Ok(object_schema) => get_type_from_schema(
                    spec,
                    struct_database,
                    &object_schema,
                    Some(&parameter.name),
                    name_mapper,
                ),
                Err(err) => {
                    return Err(format!(
                        "Failed to resolve parameter {} {}",
                        parameter.name,
                        err.to_string()
                    ))
                }
            },
            None => return Err(format!("Parameter {} has no schema", parameter.name)),
        };

        let _ = match parameter_type {
            Ok(parameter_type) => query_struct.properties.insert(
                name_mapper.name_to_property_name(&parameter.name),
                PropertyDefinition {
                    name: name_mapper.name_to_property_name(&parameter.name),
                    module: parameter_type.module,
                    real_name: parameter.name,
                    required: match parameter.required {
                        Some(required) => required,
                        None => false,
                    },
                    type_name: parameter_type.name,
                },
            ),
            Err(err) => return Err(err),
        };
    }

    let mut query_struct_source_code = String::new();
    if query_struct.properties.len() > 0 {
        function_parameters.push(format!(
            "{}: {}",
            name_mapper.name_to_property_name(&query_struct.name),
            query_struct.name
        ));
        query_struct_source_code += &query_struct.to_string(false);
        query_struct_source_code += "\n\n";
    }

    // Request Body
    let request_body = match operation.request_body {
        Some(ref request_body) => {
            match generate_request_body(
                spec,
                struct_database,
                name_mapper,
                request_body,
                &function_name,
            ) {
                Ok(request_body) => Some(request_body),
                Err(err) => {
                    return Err(format!(
                        "Failed to generated request body {}",
                        err.to_string()
                    ))
                }
            }
        }
        None => None,
    };

    if let Some(ref request_body) = request_body {
        match request_body.content {
            TransferMediaType::ApplicationJson(ref type_definition) => {
                if let Some(ref module) = type_definition.module {
                    if !module_imports.contains(module) {
                        module_imports.push(module.clone());
                    }
                }
                function_parameters.push(format!(
                    "{}: {}",
                    name_mapper.name_to_property_name(&type_definition.name),
                    type_definition.name
                ))
            }
        }
    }

    request_source_code += &module_imports
        .iter()
        .map(use_module_to_string)
        .collect::<Vec<String>>()
        .join("\n");
    request_source_code += "\n\n";
    request_source_code += &response_enum_source_code;
    request_source_code += "\n";
    if !path_struct_definition.properties.is_empty() {
        request_source_code += &path_struct_definition.to_string(false);
        request_source_code += "\n";
    }
    
    request_source_code += &query_struct_source_code;

    // Function signature
    request_source_code += &format!(
        "pub async fn {}(client: reqwest::Client, {}) -> Result<{}, reqwest::Error> {{\n",
        function_name,
        function_parameters.join(", "),
        response_enum_name,
    );

    request_source_code += &format!(
        "let {} query_parameters: Vec<(&str, String)> = vec![{}];\n",
        match query_struct
            .properties
            .iter()
            .filter(|(_, property)| !property.required || property.type_name.starts_with("Vec<"))
            .collect::<Vec<(&String, &PropertyDefinition)>>()
            .len()
        {
            0 => "",
            _ => "mut",
        },
        query_struct
            .properties
            .iter()
            .filter(|(_, property)| property.required && !property.type_name.starts_with("Vec<"))
            .map(|(_, property)| format!(
                "(\"{}\",{}.{}.to_string())",
                property.real_name,
                name_mapper.name_to_property_name(&query_struct.name),
                property.name
            ))
            .collect::<Vec<String>>()
            .join(",")
    );

    query_struct
        .properties
        .values()
        .filter(|&property| property.required && property.type_name.starts_with("Vec<"))
        .for_each(|vector_property|
    {
        request_source_code += &format!(
                "{}.{}.iter().for_each(|query_parameter_item| query_parameters.push((\"{}\", query_parameter_item.to_string())));\n",
                name_mapper.name_to_property_name(&query_struct.name),
                name_mapper.name_to_property_name(&vector_property.name),
                vector_property.real_name
            );
        
    });

    for optional_property in query_struct
        .properties
        .values()
        .filter(|&property| !property.required)
        .collect::<Vec<&PropertyDefinition>>()
    {
        request_source_code += &format!(
            "if let Some(query_parameter) = {}.{} {{\n",
            name_mapper.name_to_property_name(&query_struct.name),
            optional_property.name
        );
        if optional_property.type_name.starts_with("Vec<") {
            request_source_code += &format!(
                "query_parameter.iter().for_each(|query_parameter_item| query_parameters.push((\"{}\", query_parameter_item.to_string())));\n",
                optional_property.real_name
            );
        } else {
            request_source_code += &format!(
                "query_parameters.push((\"{}\", query_parameter.to_string()));\n",
                optional_property.real_name
            );
        }
        request_source_code += "}\n"
    }

    let body_build = match request_body {
        Some(request_body) => match request_body.content {
            TransferMediaType::ApplicationJson(type_definition) => {
                format!(
                    ".json(&{})",
                    name_mapper.name_to_property_name(&type_definition.name)
                )
            }
        },
        None => String::new(),
    };

    request_source_code += &format!(
        "    let response = match client.{}(format!(\"{}\", {})).query(&query_parameters){}.send().await\n",
        method.as_str().to_lowercase(),
        path_format_string,
        path_parameters_ordered.iter().map(|parameter| format!("{}.{}", name_mapper.name_to_property_name(&path_struct_definition.name), name_mapper.name_to_property_name(&parameter.name))).collect::<Vec<String>>().join(","),
        body_build
    );
    request_source_code += "    {\n";
    request_source_code += "        Ok(response) => response,\n";
    request_source_code += "        Err(err) => return Err(err),\n";
    request_source_code += "    };\n";

    request_source_code += "    match response.status().as_str() {\n";

    for (response_key, entity) in &response_entities {
        match entity.content {
            Some(ref content) => match content {
                TransferMediaType::ApplicationJson(ref type_definition) => {
                    request_source_code += &format!(
                        "\"{}\" => match response.json::<{}>().await {{\n",
                        response_key, type_definition.name
                    );

                    request_source_code += &format!(
                        "Ok({}) => Ok({}::{}({})),\n",
                        name_mapper.name_to_property_name(&type_definition.name),
                        response_enum_name,
                        name_mapper.name_to_struct_name(&entity.canonical_status_code),
                        name_mapper.name_to_property_name(&type_definition.name)
                    );
                    request_source_code += "Err(parsing_error) => Err(parsing_error)\n";
                    request_source_code += "}"
                }
            },
            None => {
                request_source_code += &format!(
                    "\"{}\" => Ok({}::{}),\n",
                    response_key,
                    response_enum_name,
                    name_mapper.name_to_struct_name(&entity.canonical_status_code),
                )
            }
        }
    }

    request_source_code += &format!(
        "_ => Ok({}::UndefinedResponse(response))\n",
        response_enum_name
    );

    request_source_code += "}";

    request_source_code += "}";
    Ok(request_source_code)
}
