use std::collections::HashMap;

use askama::Template;
use log::{trace, warn};
use oas3::{
    spec::{Operation, ParameterIn},
    Spec,
};

use crate::{
    generator::rust_reqwest_async::{
        component::{
            object_definition::{
                oas3_type_to_string,
                types::{
                    to_unique_list, EnumDefinition, EnumValue, ModuleInfo, ObjectDatabase,
                    PropertyDefinition, StructDefinition, TypeDefinition,
                },
            },
            type_definition::get_type_from_schema,
        },
        templates::{
            EnumDefinitionTemplate, PrimitiveDefinitionTemplate, StructDefinitionTemplate,
        },
    },
    utils::name_mapping::NameMapping,
};

use super::utils::{
    generate_request_body, generate_responses, is_path_parameter, RequestEntity, TransferMediaType,
};

#[derive(Debug)]
struct QueryParameter {
    is_required: bool,
    is_array: bool,
    real_name: String,
    name: String,
    struct_name: String,
}

#[derive(Debug)]
struct FunctionParameter {
    name: String,
    type_name: String,
    reference: bool,
}

#[derive(Template)]
#[template(path = "rust_reqwest_async/http.rs.jinja", ext = "rs")]
struct HttpRequestTemplate {
    // Base
    module_imports: Vec<ModuleInfo>,
    struct_definitions: Vec<StructDefinitionTemplate>,
    enum_definitions: Vec<EnumDefinitionTemplate>,
    primitive_definitions: Vec<PrimitiveDefinitionTemplate>,
    // Request
    response_type_name: String,
    function_visibility: String,
    function_name: String,
    function_parameters: Vec<FunctionParameter>,
    path_format_string: String,
    path_parameter_arguments: String,
    request_query_parameters_code: String,
    request_body_content_types_count: usize,
    request_media_type: String,
    request_content_variable_name: String,
    request_method: String,
    has_response_any_multi_content_type: bool,

    query_parameters_mutable: bool,
    query_parameters: Vec<QueryParameter>,

    response_parse_code: String,
    multi_request_type_source_code: String,
}

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
    let path_parameter_code = match generate_path_parameter_code(
        &operation_definition_path,
        name_mapping,
        &function_name,
        path,
    ) {
        Ok(path_parameter_code) => path_parameter_code,
        Err(err) => return Err(err),
    };

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

    // Generated enums for multi content type responses
    let mut response_enums: Vec<EnumDefinition> = vec![];
    for (_, entity) in &response_entities {
        if entity.content.len() < 2 {
            continue;
        }

        let response_code_enum_name = name_mapping.name_to_struct_name(
            &response_enum_definition_path,
            &format!("{}Value", entity.canonical_status_code),
        );

        let mut response_enum = EnumDefinition {
            name: response_code_enum_name.clone(),
            used_modules: vec![],
            values: HashMap::new(),
        };
        let mut enum_definition_path = operation_definition_path.clone();
        enum_definition_path.push(response_code_enum_name);

        for (_, transfer_media_type) in &entity.content {
            let transfer_media_type_name =
                media_type_enum_name(&enum_definition_path, name_mapping, transfer_media_type);
            let enum_value = &match transfer_media_type {
                TransferMediaType::ApplicationJson(type_definition) => match type_definition {
                    Some(type_definition) => EnumValue {
                        name: transfer_media_type_name,
                        value_type: type_definition.clone(),
                    },
                    None => EnumValue {
                        name: transfer_media_type_name,
                        value_type: TypeDefinition {
                            name: "".to_string(),
                            module: None,
                        },
                    },
                },
                TransferMediaType::TextPlain => EnumValue {
                    name: transfer_media_type_name,
                    value_type: TypeDefinition {
                        name: oas3_type_to_string(&oas3::spec::SchemaType::String),
                        module: None,
                    },
                },
            };

            response_enum
                .values
                .insert(enum_value.name.clone(), enum_value.clone());
        }

        response_enums.push(response_enum);
    }

    let mut response_enum = EnumDefinition {
        name: response_enum_name.clone(),
        used_modules: vec![],
        values: HashMap::new(),
    };

    for (status_code, entity) in &response_entities {
        let response_enum_name = name_mapping.name_to_struct_name(
            &response_enum_definition_path,
            &format!("{}", entity.canonical_status_code),
        );

        let enum_value = &match entity.content.len() {
            0 => continue,
            1 => match entity.content.values().next() {
                Some(transfer_media_type) => match transfer_media_type {
                    TransferMediaType::ApplicationJson(type_definition) => match type_definition {
                        Some(type_definition) => EnumValue {
                            name: response_enum_name,
                            value_type: type_definition.clone(),
                        },

                        None => EnumValue {
                            name: response_enum_name,
                            value_type: TypeDefinition {
                                name: "".to_string(),
                                module: None,
                            },
                        },
                    },
                    TransferMediaType::TextPlain => EnumValue {
                        name: response_enum_name,
                        value_type: TypeDefinition {
                            name: oas3_type_to_string(&oas3::spec::SchemaType::String),
                            module: None,
                        },
                    },
                },
                None => {
                    return Err(format!(
                        "Failed to retrieve first response media type of status {}",
                        status_code
                    ))
                }
            },
            _ => EnumValue {
                name: response_enum_name,
                value_type: TypeDefinition {
                    name: name_mapping.name_to_struct_name(
                        &response_enum_definition_path,
                        &format!("{}Value", entity.canonical_status_code),
                    ),
                    module: None,
                },
            },
        };

        response_enum
            .values
            .insert(status_code.to_string(), enum_value.clone());
    }

    response_enum.values.insert(
        "UndefinedResponse".to_string(),
        EnumValue {
            name: "UndefinedResponse".to_owned(),
            value_type: TypeDefinition {
                name: "reqwest::Response".to_owned(),
                module: Some(ModuleInfo {
                    name: "reqwest".to_owned(),
                    path: String::new(),
                }),
            },
        },
    );
    response_enums.push(response_enum);

    // Query params
    let query_parameter_code = match generate_query_parameter_code(
        spec,
        operation,
        &operation_definition_path,
        name_mapping,
        object_database,
        &function_name,
    ) {
        Ok(query_parameter_code) => query_parameter_code,
        Err(err) => return Err(err),
    };

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

    let request_body_content_types_count = match request_body {
        Some(ref request_body) => request_body.content.len(),
        None => 0,
    };

    let multi_content_request_body = request_body_content_types_count > 1;

    let multi_request_type_source_code = match request_body {
        Some(ref request_entity) => match generate_multi_request_type_functions(
            &operation_definition_path,
            name_mapping,
            &function_name,
            &path_parameter_code,
            &mut module_imports,
            &query_parameter_code,
            &response_enum_name,
            method,
            request_entity,
        ) {
            Some(request_code) => request_code,
            None => String::new(),
        },

        None => String::new(),
    };

    let mut function_parameters: Vec<FunctionParameter> = match multi_content_request_body {
        true => vec![FunctionParameter {
            name: "request_builder".to_owned(),
            type_name: "reqwest::RequestBuilder".to_owned(),
            reference: false,
        }],
        false => vec![
            FunctionParameter {
                name: "client".to_owned(),
                type_name: "reqwest::Client".to_owned(),
                reference: true,
            },
            FunctionParameter {
                name: "server".to_owned(),
                type_name: "str".to_owned(),
                reference: true,
            },
        ],
    };

    let request_content_variable_name = match multi_content_request_body {
        true => String::new(),
        false => name_mapping.name_to_property_name(&operation_definition_path, "content"),
    };

    if !multi_content_request_body {
        if let Some(request_body) = &request_body {
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
                                function_parameters.push(FunctionParameter {
                                    name: request_content_variable_name.clone(),
                                    type_name: type_definition.name.clone(),
                                    reference: false,
                                });
                            }
                            None => trace!("Empty request body not added to function params"),
                        }
                    }
                    TransferMediaType::TextPlain => function_parameters.push(FunctionParameter {
                        name: request_content_variable_name.clone(),
                        type_name: oas3_type_to_string(&oas3::spec::SchemaType::String),
                        reference: true,
                    }),
                }
            }
        }
    }

    trace!("Generating source code");
    let struct_definition_templates = vec![
        Into::<StructDefinitionTemplate>::into(&path_parameter_code.parameters_struct)
            .serializable(false),
        Into::<StructDefinitionTemplate>::into(&query_parameter_code.query_struct)
            .serializable(false),
    ];

    if !multi_content_request_body && path_parameter_code.parameters_struct.properties.len() > 0 {
        function_parameters.push(FunctionParameter {
            name: path_parameter_code.parameters_struct_variable_name.clone(),
            type_name: path_parameter_code.parameters_struct.name.clone(),
            reference: false,
        });
    }

    let query_struct = &query_parameter_code.query_struct;
    if query_struct.properties.len() > 0 {
        function_parameters.push(FunctionParameter {
            name: query_parameter_code.query_struct_variable_name,
            type_name: query_struct.name.clone(),
            reference: false,
        });
    }

    let function_visibility = match multi_content_request_body {
        true => "",
        false => "pub",
    };

    let request_media_type = match request_body {
        Some(request_body) => {
            if request_body.content.len() > 1 {
                warn!("Multiple request body content types not supported yet");
            }
            let mut media_type = String::new();
            for (_, transfer_media_type) in request_body.content {
                media_type = match transfer_media_type {
                    TransferMediaType::ApplicationJson(_) => "application/json".to_owned(),
                    TransferMediaType::TextPlain => "text/plain".to_owned(),
                };
                // TODO: multiple request types not supported
                break;
            }
            media_type
        }
        None => String::new(),
    };

    let mut response_parse_code = "    match response.status().as_u16() {\n".to_string();

    for (response_key, entity) in &response_entities {
        if entity.content.len() > 1 {
            // Multi content type response
            response_parse_code += &format!("{} => match content_type {{\n", response_key);

            for (content_type, transfer_media_type) in &entity.content {
                match transfer_media_type {
                    TransferMediaType::ApplicationJson(ref type_definition) => {
                        match type_definition {
                            Some(type_definition) => {
                                response_parse_code += &format!(
                                    "\"{}\" => match response.json::<{}>().await {{\n",
                                    content_type, type_definition.name
                                );

                                response_parse_code += &format!(
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
                                response_parse_code += "Err(parsing_error) => Err(parsing_error)\n";
                                response_parse_code += "}\n"
                            }
                            None => {
                                response_parse_code += &format!(
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
                        response_parse_code +=
                            &format!("\"{}\" => match response.text().await {{\n", content_type);

                        response_parse_code += &format!(
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
                        response_parse_code += "Err(parsing_error) => Err(parsing_error)\n";
                        response_parse_code += "}\n"
                    }
                }
            }

            response_parse_code += &format!(
                "_ => Ok({}::UndefinedResponse(response))\n",
                response_enum_name
            );

            // Close content_type match
            response_parse_code += "}\n"
        } else {
            // Single content type response
            for (_, transfer_media_type) in &entity.content {
                match transfer_media_type {
                    TransferMediaType::ApplicationJson(ref type_definition) => {
                        match type_definition {
                            Some(type_definition) => {
                                response_parse_code += &format!(
                                    "{} => match response.json::<{}>().await {{\n",
                                    response_key, type_definition.name
                                );

                                response_parse_code += &format!(
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
                                response_parse_code += "Err(parsing_error) => Err(parsing_error)\n";
                                response_parse_code += "}\n"
                            }
                            None => {
                                response_parse_code += &format!(
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
                        response_parse_code +=
                            &format!("{} => match response.text().await {{\n", response_key);

                        response_parse_code += &format!(
                            "Ok(response_text) => Ok({}::{}(response_text)),\n",
                            response_enum_name,
                            name_mapping.name_to_struct_name(
                                &operation_definition_path,
                                &entity.canonical_status_code
                            )
                        );
                        response_parse_code += "Err(parsing_error) => Err(parsing_error)\n";
                        response_parse_code += "}\n"
                    }
                }
            }
        }
    }

    response_parse_code += &format!(
        "_ => Ok({}::UndefinedResponse(response))\n",
        response_enum_name
    );

    // Close match status code
    response_parse_code += "}\n";

    let template = HttpRequestTemplate {
        module_imports: to_unique_list(&module_imports),
        struct_definitions: struct_definition_templates,
        enum_definitions: response_enums
            .iter()
            .map(|enum_def| Into::<EnumDefinitionTemplate>::into(enum_def).serializable(false))
            .collect(),
        primitive_definitions: vec![],
        response_type_name: response_enum_name,
        function_visibility: function_visibility.to_owned(),
        function_name: function_name,
        function_parameters: function_parameters,
        path_format_string: path_parameter_code.path_format_string,
        path_parameter_arguments: path_parameter_code
            .parameters_struct
            .properties
            .keys()
            .map(|property_name| {
                format!(
                    "{}.{}",
                    path_parameter_code.parameters_struct_variable_name, property_name
                )
            })
            .collect::<Vec<String>>()
            .join(", "),
        request_media_type: request_media_type,
        request_query_parameters_code: query_parameter_code.unroll_query_parameters_code,
        request_body_content_types_count: request_body_content_types_count,
        request_content_variable_name: request_content_variable_name,
        request_method: method.as_str().to_lowercase(),
        has_response_any_multi_content_type: has_response_any_multi_content_type,
        query_parameters_mutable: query_struct
            .properties
            .iter()
            .any(|(_, param)| !param.required),
        query_parameters: vec![],
        response_parse_code: response_parse_code,
        multi_request_type_source_code: multi_request_type_source_code,
    };

    template.render().map_err(|err| err.to_string())
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

struct PathParameterCode {
    pub parameters_struct_variable_name: String,
    pub parameters_struct: StructDefinition,
    pub path_format_string: String,
}

fn generate_path_parameter_code(
    definition_path: &Vec<String>,
    name_mapping: &NameMapping,
    function_name: &str,
    path: &str,
) -> Result<PathParameterCode, String> {
    trace!("Generating path parameters");
    let path_parameters_struct_name = name_mapping.name_to_struct_name(
        &definition_path,
        &format!("{}PathParameters", function_name),
    );

    let mut path_parameters_definition_path = definition_path.clone();
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

    Ok(PathParameterCode {
        parameters_struct_variable_name: name_mapping
            .name_to_property_name(definition_path, "path_parameters"),
        parameters_struct: path_struct_definition,
        path_format_string: path_format_string,
    })
}

struct QueryParametersCode {
    pub query_struct: StructDefinition,
    pub query_struct_variable_name: String,
    pub unroll_query_parameters_code: String,
}

fn generate_query_parameter_code(
    spec: &Spec,
    operation: &Operation,
    definition_path: &Vec<String>,
    name_mapping: &NameMapping,
    object_database: &mut ObjectDatabase,
    function_name: &str,
) -> Result<QueryParametersCode, String> {
    trace!("Generating query params");
    let mut query_struct = StructDefinition {
        name: name_mapping.name_to_struct_name(
            &definition_path,
            &format!("{}QueryParameters", &function_name),
        ),
        properties: HashMap::new(),
        used_modules: vec![],
        local_objects: HashMap::new(),
    };

    let query_struct_variable_name =
        name_mapping.name_to_property_name(&definition_path, "query_parameters");

    let mut query_parameters_definition_path = definition_path.clone();
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

    let mut unroll_query_parameters_code = String::new();
    unroll_query_parameters_code += &format!(
        "let {} request_query_parameters: Vec<(&str, String)> = vec![{}];\n",
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
                property.real_name, query_struct_variable_name, property.name
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
        unroll_query_parameters_code += &format!(
                "{}.{}.iter().for_each(|query_parameter_item| request_query_parameters.push((\"{}\", query_parameter_item.to_string())));\n",
                &query_struct_variable_name,
                name_mapping.name_to_property_name(&definition_path, &vector_property.name),
                vector_property.real_name
            );
    });

    for optional_property in query_struct
        .properties
        .values()
        .filter(|&property| !property.required)
        .collect::<Vec<&PropertyDefinition>>()
    {
        unroll_query_parameters_code += &format!(
            "if let Some(ref query_parameter) = {}.{} {{\n",
            query_struct_variable_name, optional_property.name
        );
        if optional_property.type_name.starts_with("Vec<") {
            unroll_query_parameters_code += &format!(
                "query_parameter.iter().for_each(|query_parameter_item| request_query_parameters.push((\"{}\", query_parameter_item.to_string())));\n",
                optional_property.real_name
            );
        } else {
            unroll_query_parameters_code += &format!(
                "request_query_parameters.push((\"{}\", query_parameter.to_string()));\n",
                optional_property.real_name
            );
        }
        unroll_query_parameters_code += "}\n"
    }

    Ok(QueryParametersCode {
        query_struct_variable_name,
        query_struct,
        unroll_query_parameters_code,
    })
}

fn generate_multi_request_type_functions(
    definition_path: &Vec<String>,
    name_mapping: &NameMapping,
    function_name: &str,
    path_parameter_code: &PathParameterCode,
    module_imports: &mut Vec<ModuleInfo>,
    query_parameter_code: &QueryParametersCode,
    response_enum_name: &str,
    method: &reqwest::Method,
    request_entity: &RequestEntity,
) -> Option<String> {
    if request_entity.content.len() < 2 {
        return None;
    }

    let mut request_source_code = String::new();

    for (_, transfer_media_type) in &request_entity.content {
        let content_function_name = name_mapping.name_to_property_name(
            &definition_path,
            &format!(
                "{}{}",
                function_name,
                media_type_enum_name(&definition_path, name_mapping, &transfer_media_type)
            ),
        );
        let mut function_parameters = vec![
            "client: &reqwest::Client".to_owned(),
            "server: &str".to_owned(),
        ];

        if path_parameter_code.parameters_struct.properties.len() > 0 {
            function_parameters.push(format!(
                "{}: &{}",
                path_parameter_code.parameters_struct_variable_name,
                path_parameter_code.parameters_struct.name
            ));
        }

        let query_struct = &query_parameter_code.query_struct;
        if query_struct.properties.len() > 0 {
            function_parameters.push(format!(
                "{}: &{}",
                query_parameter_code.query_struct_variable_name, query_struct.name
            ));
        }

        let request_content_variable_name =
            name_mapping.name_to_property_name(definition_path, "content");
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
                            request_content_variable_name, type_definition.name
                        ))
                    }
                    None => trace!("Empty request body not added to function params"),
                }
            }
            TransferMediaType::TextPlain => function_parameters.push(format!(
                "{}: &{}",
                request_content_variable_name,
                oas3_type_to_string(&oas3::spec::SchemaType::String)
            )),
        }

        request_source_code += &format!(
            "pub async fn {}({}) -> Result<{}, reqwest::Error> {{\n",
            content_function_name,
            function_parameters.join(", "),
            response_enum_name,
        );

        // PRE request processing
        match transfer_media_type {
            TransferMediaType::TextPlain => {
                request_source_code +=
                    &format!("let body = {}.to_owned();\n", request_content_variable_name)
            }
            _ => (),
        }

        // Request attach
        let request_body = match transfer_media_type {
            TransferMediaType::ApplicationJson(type_definition) => match type_definition {
                Some(_) => {
                    format!(".json(&{})", request_content_variable_name)
                }
                None => ".json(&serde_json::json!({}))".to_owned(),
            },
            TransferMediaType::TextPlain => ".body(body)".to_owned(),
        };

        request_source_code += &format!(
            "let request_builder = client.{}(format!(\"{{server}}{}\", {})){};\n",
            method.as_str().to_lowercase(),
            path_parameter_code.path_format_string,
            path_parameter_code
                .parameters_struct
                .properties
                .iter()
                .map(|(_, parameter)| format!(
                    "{}.{}",
                    path_parameter_code.parameters_struct_variable_name,
                    name_mapping.name_to_property_name(&definition_path, &parameter.name)
                ))
                .collect::<Vec<String>>()
                .join(","),
            request_body
        );

        let request_function_call_parameters = match query_struct.properties.len() {
            0 => vec!["request_builder".to_owned()],
            _ => vec![
                "request_builder".to_owned(),
                query_parameter_code.query_struct_variable_name.clone(),
            ],
        };

        request_source_code += &format!(
            "{}({}).await",
            function_name,
            request_function_call_parameters.join(",")
        );
        request_source_code += "}\n";
    }

    Some(request_source_code)
}
