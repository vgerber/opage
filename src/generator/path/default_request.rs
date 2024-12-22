use std::collections::HashMap;

use log::trace;
use oas3::{
    spec::{Operation, ParameterIn},
    Spec,
};

use crate::{
    generator::component::{
        object_definition::{
            oas3_type_to_string,
            types::{ModuleInfo, ObjectDatabase, PropertyDefinition, StructDefinition},
        },
        type_definition::get_type_from_schema,
    },
    utils::name_mapping::NameMapping,
};

use super::utils::{
    generate_request_body, generate_responses, is_path_parameter, use_module_to_string,
    TransferMediaType,
};

pub fn generate_operation(
    spec: &Spec,
    name_mapping: &NameMapping,
    method: &reqwest::Method,
    path: &str,
    operation: &Operation,
    object_database: &mut ObjectDatabase,
) -> Result<String, String> {
    trace!("Generating {} {}", method.as_str(), path);
    let operation_definition_path: Vec<String> = vec![path.to_owned()];
    let function_name = match operation.operation_id {
        Some(ref operation_id) => name_mapping.name_to_module_name(operation_id),
        None => return Err("No operation_id found".to_owned()),
    };

    let response_entities = match generate_responses(
        spec,
        object_database,
        &operation_definition_path,
        name_mapping,
        &operation.responses(spec),
        &function_name,
    ) {
        Ok(response_entities) => response_entities,
        Err(err) => return Err(err),
    };

    // Path parameters
    trace!("Generating path parameters");
    let path_parameters_struct_name = name_mapping.name_to_struct_name(
        &operation_definition_path,
        &format!("{}PathParameters", function_name),
    );

    let mut path_parameters_definition_path = operation_definition_path.clone();
    path_parameters_definition_path.push(path_parameters_struct_name.clone());

    let path_parameters_ordered = path
        .split("/")
        .filter(|&path_component| is_path_parameter(&path_component))
        .map(|path_component| path_component.replace("{", "").replace("}", ""))
        .map(|path_component| PropertyDefinition {
            module: None,
            name: name_mapping
                .name_to_property_name(&path_parameters_definition_path, &path_component),
            real_name: path_component,
            required: true,
            type_name: "&str".to_owned(),
        })
        .collect::<Vec<PropertyDefinition>>();
    let path_struct_definition = StructDefinition {
        name: path_parameters_struct_name,
        used_modules: vec![],
        local_objects: HashMap::new(),
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

    // Response enum
    trace!("Generating response enum");

    let has_response_any_multi_content_type = response_entities
        .iter()
        .map(|response| response.1.content.len())
        .filter(|content_type_length| content_type_length > &1)
        .collect::<Vec<usize>>()
        .len()
        > 0;

    let response_enum_name = name_mapping.name_to_struct_name(
        &operation_definition_path,
        &format!("{}ResponseType", &function_name),
    );
    let mut response_enum_definition_path = operation_definition_path.clone();
    response_enum_definition_path.push(response_enum_name.clone());

    let mut request_source_code = String::new();

    let mut function_parameters = vec![];

    if !path_struct_definition.properties.is_empty() {
        function_parameters.push(format!(
            "{}: &{}",
            name_mapping.name_to_property_name(
                &response_enum_definition_path,
                &path_struct_definition.name
            ),
            path_struct_definition.name
        ));
    }

    let mut module_imports = vec![ModuleInfo {
        name: "reqwest".to_owned(),
        path: String::new(),
    }];

    // Response types
    for (_, entity) in &response_entities {
        for (_, content) in &entity.content {
            match content {
                TransferMediaType::ApplicationJson(ref type_definition) => match type_definition {
                    Some(type_definition) => match type_definition.module {
                        Some(ref module_info) => {
                            if module_imports.contains(module_info) {
                                continue;
                            }
                            module_imports.push(module_info.clone());
                        }
                        _ => (),
                    },
                    None => (),
                },
                TransferMediaType::TextPlain => (),
            }
        }
    }

    let mut response_enum_source_code = String::new();

    // Generated enums for multi content type responses
    for (_, entity) in &response_entities {
        if entity.content.len() < 2 {
            continue;
        }

        let response_code_enum_name = name_mapping.name_to_struct_name(
            &response_enum_definition_path,
            &format!("{}Value", entity.canonical_status_code),
        );
        response_enum_source_code += &format!("pub enum {} {{\n", &response_code_enum_name);
        let mut enum_definition_path = operation_definition_path.clone();
        enum_definition_path.push(response_code_enum_name);

        for (_, transfer_media_type) in &entity.content {
            let transfer_media_type_name =
                media_type_enum_name(&enum_definition_path, name_mapping, transfer_media_type);
            response_enum_source_code += &match transfer_media_type {
                TransferMediaType::ApplicationJson(type_definiton) => match type_definiton {
                    Some(type_definition) => {
                        format!("{}({}),\n", transfer_media_type_name, type_definition.name)
                    }

                    None => format!("{},\n", transfer_media_type_name),
                },
                TransferMediaType::TextPlain => format!(
                    "{}({}),\n",
                    transfer_media_type_name,
                    oas3_type_to_string(&oas3::spec::SchemaType::String)
                ),
            }
        }
        response_enum_source_code += "}\n\n";
    }

    response_enum_source_code += &format!("pub enum {} {{\n", response_enum_name);

    for (status_code, entity) in &response_entities {
        let response_enum_name = name_mapping.name_to_struct_name(
            &response_enum_definition_path,
            &format!("{}", entity.canonical_status_code),
        );

        response_enum_source_code += &match entity.content.len() {
            0 => continue,
            1 => match entity.content.values().next() {
                Some(transfer_media_type) => match transfer_media_type {
                    TransferMediaType::ApplicationJson(type_definiton) => match type_definiton {
                        Some(type_definition) => {
                            format!("{}({}),\n", response_enum_name, type_definition.name)
                        }

                        None => format!("{},\n", response_enum_name),
                    },
                    TransferMediaType::TextPlain => format!(
                        "{}({}),\n",
                        response_enum_name,
                        oas3_type_to_string(&oas3::spec::SchemaType::String)
                    ),
                },
                None => {
                    return Err(format!(
                        "Failed to retrieve first response media type of status {}",
                        status_code
                    ))
                }
            },
            _ => format!(
                "{}({}),\n",
                response_enum_name,
                name_mapping.name_to_struct_name(
                    &response_enum_definition_path,
                    &format!("{}Value", entity.canonical_status_code)
                ),
            ),
        };
    }

    response_enum_source_code += "UndefinedResponse(reqwest::Response),\n";
    response_enum_source_code += "}\n";

    // Query params
    trace!("Generating query params");
    let mut query_struct = StructDefinition {
        name: name_mapping.name_to_struct_name(
            &operation_definition_path,
            &format!("{}QueryParameters", &function_name),
        ),
        properties: HashMap::new(),
        used_modules: vec![],
        local_objects: HashMap::new(),
    };

    let mut query_parameters_definition_path = operation_definition_path.clone();
    query_parameters_definition_path.push(query_struct.name.clone());

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
                    object_database,
                    query_parameters_definition_path.clone(),
                    &object_schema,
                    Some(&parameter.name),
                    name_mapping,
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
                name_mapping
                    .name_to_property_name(&query_parameters_definition_path, &parameter.name),
                PropertyDefinition {
                    name: name_mapping
                        .name_to_property_name(&query_parameters_definition_path, &parameter.name),
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
            "{}: &{}",
            name_mapping.name_to_property_name(&operation_definition_path, &query_struct.name),
            query_struct.name
        ));
        query_struct_source_code += &query_struct.to_string(false);
        query_struct_source_code += "\n\n";
    }

    // Request Body
    trace!("Generating request body");
    let request_body = match operation.request_body {
        Some(ref request_body) => {
            match generate_request_body(
                spec,
                object_database,
                &operation_definition_path,
                name_mapping,
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
        for (_, transfer_media_type) in &request_body.content {
            match transfer_media_type {
                TransferMediaType::ApplicationJson(ref type_definition_opt) => {
                    match type_definition_opt {
                        Some(ref type_definition) => {
                            if let Some(ref module) = type_definition.module {
                                if !module_imports.contains(module) {
                                    module_imports.push(module.clone());
                                }
                            }
                            function_parameters.push(format!(
                                "{}: {}",
                                name_mapping.name_to_property_name(
                                    &operation_definition_path,
                                    &type_definition.name
                                ),
                                type_definition.name
                            ))
                        }
                        None => trace!("Empty request body not added to function params"),
                    }
                }
                TransferMediaType::TextPlain => function_parameters.push(format!(
                    "request_string: &{}",
                    oas3_type_to_string(&oas3::spec::SchemaType::String)
                )),
            }
            // TODO add multi type support
            break;
        }
    }

    trace!("Generating source code");
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
        "pub async fn {}(client: &reqwest::Client, server: &str, {}) -> Result<{}, reqwest::Error> {{\n",
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
                name_mapping.name_to_property_name(&operation_definition_path, &query_struct.name),
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
                name_mapping.name_to_property_name(&operation_definition_path, &query_struct.name),
                name_mapping.name_to_property_name(&operation_definition_path, &vector_property.name),
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
            "if let Some(ref query_parameter) = {}.{} {{\n",
            name_mapping.name_to_property_name(&operation_definition_path, &query_struct.name),
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

    match request_body {
        Some(ref request_body) => {
            for (_, transfer_media_type) in &request_body.content {
                match transfer_media_type {
                    TransferMediaType::TextPlain => {
                        request_source_code += "let body = request_string.to_owned();\n"
                    }
                    _ => (),
                }

                // TODO: multiple request types not supported
                break;
            }
        }
        None => (),
    }

    let body_build = match request_body {
        Some(request_body) => {
            let mut body = String::new();
            for (_, transfer_media_type) in request_body.content {
                match transfer_media_type {
                    TransferMediaType::ApplicationJson(type_definition) => match type_definition {
                        Some(type_definition) => {
                            body = format!(
                                ".json(&{})",
                                name_mapping.name_to_property_name(
                                    &operation_definition_path,
                                    &type_definition.name
                                )
                            )
                        }
                        None => body = ".json(&serde_json::json!({}))".to_owned(),
                    },
                    TransferMediaType::TextPlain => body = ".body(body)".to_owned(),
                }

                // TODO: multiple request types not supported
                break;
            }
            body
        }
        None => String::new(),
    };

    request_source_code += &format!(
        "    let response = match client.{}(format!(\"{{server}}{}\", {})).query(&query_parameters){}.send().await\n",
        method.as_str().to_lowercase(),
        path_format_string,
        path_parameters_ordered.iter().map(|parameter| format!("{}.{}", name_mapping.name_to_property_name(&operation_definition_path, &path_struct_definition.name), name_mapping.name_to_property_name(&operation_definition_path, &parameter.name))).collect::<Vec<String>>().join(","),
        body_build
    );
    request_source_code += "    {\n";
    request_source_code += "        Ok(response) => response,\n";
    request_source_code += "        Err(err) => return Err(err),\n";
    request_source_code += "    };\n";

    if has_response_any_multi_content_type {
        request_source_code += "let content_type = match response\n";
        request_source_code += "    .headers()\n";
        request_source_code += "    .get(\"content-type\") {\n";
        request_source_code += "    Some(content_type) => match content_type.to_str()\n";
        request_source_code += "    {\n";
        request_source_code += "        Ok(content_type) => content_type,\n";
        request_source_code += "        Err(_) => \"text/plain\",\n";
        request_source_code += "    },\n";
        request_source_code += &format!(
            "    None => return Ok({}::UndefinedResponse(response))\n",
            response_enum_name
        );
        request_source_code += "    };\n\n";
    }

    request_source_code += "    match response.status().as_u16() {\n";

    for (response_key, entity) in &response_entities {
        if entity.content.len() > 1 {
            // Multi content type response
            request_source_code += &format!("{} => match content_type {{\n", response_key);

            for (content_type, transfer_media_type) in &entity.content {
                match transfer_media_type {
                    TransferMediaType::ApplicationJson(ref type_definition) => {
                        match type_definition {
                            Some(type_definition) => {
                                request_source_code += &format!(
                                    "\"{}\" => match response.json::<{}>().await {{\n",
                                    content_type, type_definition.name
                                );

                                request_source_code += &format!(
                                    "Ok({}) => Ok({}::{}({}::{}({}))),\n",
                                    name_mapping.name_to_property_name(
                                        &operation_definition_path,
                                        &type_definition.name
                                    ),
                                    response_enum_name,
                                    name_mapping.name_to_struct_name(
                                        &operation_definition_path,
                                        &entity.canonical_status_code
                                    ),
                                    name_mapping.name_to_struct_name(
                                        &response_enum_definition_path,
                                        &format!("{}Value", &entity.canonical_status_code)
                                    ),
                                    media_type_enum_name(
                                        &response_enum_definition_path,
                                        &name_mapping,
                                        &TransferMediaType::ApplicationJson(None)
                                    ),
                                    name_mapping.name_to_property_name(
                                        &operation_definition_path,
                                        &type_definition.name
                                    )
                                );
                                request_source_code += "Err(parsing_error) => Err(parsing_error)\n";
                                request_source_code += "}\n"
                            }
                            None => {
                                request_source_code += &format!(
                                    "\"{}\" => Ok({}::{}({}::{})),\n",
                                    content_type,
                                    response_enum_name,
                                    name_mapping.name_to_struct_name(
                                        &operation_definition_path,
                                        &entity.canonical_status_code
                                    ),
                                    name_mapping.name_to_struct_name(
                                        &response_enum_definition_path,
                                        &format!("{}Value", &entity.canonical_status_code)
                                    ),
                                    media_type_enum_name(
                                        &response_enum_definition_path,
                                        &name_mapping,
                                        &TransferMediaType::ApplicationJson(None)
                                    )
                                );
                            }
                        }
                    }
                    TransferMediaType::TextPlain => {
                        request_source_code +=
                            &format!("\"{}\" => match response.text().await {{\n", content_type);

                        request_source_code += &format!(
                            "Ok(response_text) => Ok({}::{}({}::{}(response_text))),\n",
                            response_enum_name,
                            name_mapping.name_to_struct_name(
                                &operation_definition_path,
                                &entity.canonical_status_code
                            ),
                            name_mapping.name_to_struct_name(
                                &response_enum_definition_path,
                                &format!("{}Value", &entity.canonical_status_code)
                            ),
                            media_type_enum_name(
                                &response_enum_definition_path,
                                &name_mapping,
                                &TransferMediaType::TextPlain
                            )
                        );
                        request_source_code += "Err(parsing_error) => Err(parsing_error)\n";
                        request_source_code += "}\n"
                    }
                }
            }

            request_source_code += &format!(
                "_ => Ok({}::UndefinedResponse(response))\n",
                response_enum_name
            );

            // Close content_type match
            request_source_code += "}\n"
        } else {
            // Single content type response
            for (_, transfer_media_type) in &entity.content {
                match transfer_media_type {
                    TransferMediaType::ApplicationJson(ref type_definition) => {
                        match type_definition {
                            Some(type_definition) => {
                                request_source_code += &format!(
                                    "{} => match response.json::<{}>().await {{\n",
                                    response_key, type_definition.name
                                );

                                request_source_code += &format!(
                                    "Ok({}) => Ok({}::{}({})),\n",
                                    name_mapping.name_to_property_name(
                                        &operation_definition_path,
                                        &type_definition.name
                                    ),
                                    response_enum_name,
                                    name_mapping.name_to_struct_name(
                                        &operation_definition_path,
                                        &entity.canonical_status_code
                                    ),
                                    name_mapping.name_to_property_name(
                                        &operation_definition_path,
                                        &type_definition.name
                                    )
                                );
                                request_source_code += "Err(parsing_error) => Err(parsing_error)\n";
                                request_source_code += "}\n"
                            }
                            None => {
                                request_source_code += &format!(
                                    "{} => Ok({}::{}),\n",
                                    response_key,
                                    response_enum_name,
                                    name_mapping.name_to_struct_name(
                                        &operation_definition_path,
                                        &entity.canonical_status_code
                                    )
                                );
                            }
                        }
                    }
                    TransferMediaType::TextPlain => {
                        request_source_code +=
                            &format!("{} => match response.text().await {{\n", response_key);

                        request_source_code += &format!(
                            "Ok(response_text) => Ok({}::{}(response_text)),\n",
                            response_enum_name,
                            name_mapping.name_to_struct_name(
                                &operation_definition_path,
                                &entity.canonical_status_code
                            )
                        );
                        request_source_code += "Err(parsing_error) => Err(parsing_error)\n";
                        request_source_code += "}\n"
                    }
                }
            }
        }
    }

    request_source_code += &format!(
        "_ => Ok({}::UndefinedResponse(response))\n",
        response_enum_name
    );

    // Close match status code
    request_source_code += "}\n";

    // function
    request_source_code += "}\n";
    Ok(request_source_code)
}

fn media_type_enum_name(
    definition_path: &Vec<String>,
    name_mapping: &NameMapping,
    transfer_media_type: &TransferMediaType,
) -> String {
    let name = match transfer_media_type {
        TransferMediaType::ApplicationJson(_) => "Json",
        TransferMediaType::TextPlain => "Text",
    };
    name_mapping.name_to_struct_name(definition_path, name)
}
