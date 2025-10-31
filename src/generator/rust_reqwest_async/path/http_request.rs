use std::collections::HashMap;

use askama::Template;
use log::{trace, warn};
use oas3::{
    spec::{Operation, ParameterIn},
    Spec,
};

use crate::{
    generator::rust_reqwest_async::{
        path::utils::ResponseEntity,
        templates::{
            EnumDefinitionTemplate, PrimitiveDefinitionTemplate, StructDefinitionTemplate,
        },
    },
    parser::component::{
        object_definition::{
            oas3_type_to_string,
            types::{
                to_unique_list, EnumDefinition, EnumValue, ModuleInfo, ObjectDatabase,
                PropertyDefinition, StructDefinition, TypeDefinition,
            },
        },
        type_definition::get_type_from_schema,
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
    name_mapping: NameMapping,
    // Request
    operation_definition_path: Vec<String>,
    response_enum_definition_path: Vec<String>,
    response_type_name: String,
    function_visibility: String,
    function_name: String,
    function_parameters: Vec<FunctionParameter>,
    path_format_string: String,
    path_parameter_arguments: String,
    request_body_content_types_count: usize,
    request_media_type: String,
    request_content_variable_name: Option<String>,
    request_method: String,
    has_response_any_multi_content_type: bool,

    query_parameters_mutable: bool,
    query_parameters: Vec<QueryParameter>,

    responses: HashMap<String, ResponseEntity>,
    multi_request_type_functions: Vec<MultiRequestTypeFunction>,

    media_type_enum_name: fn(&Vec<String>, &NameMapping, &TransferMediaType) -> String,
}

impl HttpRequestTemplate {
    fn media_type_enum_name(
        &self,
        operation_definition_path: &Vec<String>,
        name_mapping: &NameMapping,
        transfer_media_type: &TransferMediaType,
    ) -> String {
        (self.media_type_enum_name)(operation_definition_path, name_mapping, transfer_media_type)
    }
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

    let multi_request_type_functions = match request_body {
        Some(ref request_entity) => match generate_multi_request_type_functions(
            &operation_definition_path,
            name_mapping,
            &function_name,
            &path_parameter_code,
            &mut module_imports,
            &query_parameter_code,
            request_entity,
        ) {
            functions => Some(functions),
        },

        None => None,
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

    let mut request_content_variable_name = None;

    if !multi_content_request_body {
        if let Some(request_body) = &request_body {
            for (_, transfer_media_type) in &request_body.content {
                match transfer_media_type {
                    TransferMediaType::ApplicationJson(ref type_definition_opt) => {
                        match type_definition_opt {
                            Some(ref type_definition) => {
                                let variable_name = name_mapping
                                    .name_to_property_name(&operation_definition_path, "content");
                                if let Some(ref module) = type_definition.module {
                                    if !module_imports.contains(module) {
                                        module_imports.push(module.clone());
                                    }
                                }
                                function_parameters.push(FunctionParameter {
                                    name: variable_name.clone(),
                                    type_name: type_definition.name.clone(),
                                    reference: false,
                                });
                                request_content_variable_name = Some(variable_name);
                            }
                            None => trace!("Empty request body not added to function params"),
                        }
                    }
                    TransferMediaType::TextPlain => {
                        let variable_name = name_mapping
                            .name_to_property_name(&operation_definition_path, "content");
                        function_parameters.push(FunctionParameter {
                            name: variable_name.clone(),
                            type_name: oas3_type_to_string(&oas3::spec::SchemaType::String),
                            reference: true,
                        });
                        request_content_variable_name = Some(variable_name);
                    }
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

    module_imports.extend(
        path_parameter_code
            .parameters_struct
            .get_required_modules()
            .iter()
            .map(|&module| module.clone()),
    );
    module_imports.extend(
        query_parameter_code
            .query_struct
            .get_required_modules()
            .iter()
            .map(|&module| module.clone()),
    );

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
            name: query_parameter_code.query_struct_variable_name.clone(),
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
        request_body_content_types_count: request_body_content_types_count,
        request_content_variable_name: request_content_variable_name,
        request_method: method.as_str().to_lowercase(),
        has_response_any_multi_content_type: has_response_any_multi_content_type,
        query_parameters_mutable: query_struct
            .properties
            .iter()
            .filter(|(_, property)| !property.required || property.type_name.starts_with("Vec<"))
            .collect::<Vec<(&String, &PropertyDefinition)>>()
            .len()
            > 0,
        query_parameters: query_struct
            .properties
            .iter()
            .map(|(_, property)| QueryParameter {
                real_name: property.real_name.clone(),
                name: property.name.clone(),
                struct_name: query_parameter_code.query_struct_variable_name.clone(),
                is_required: property.required,
                is_array: property.type_name.starts_with("Vec<"),
            })
            .collect(),
        responses: response_entities,
        multi_request_type_functions: multi_request_type_functions.unwrap_or(vec![]),
        media_type_enum_name: media_type_enum_name,
        name_mapping: name_mapping.clone(),
        operation_definition_path: operation_definition_path.clone(),
        response_enum_definition_path: response_enum_definition_path.clone(),
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

    Ok(QueryParametersCode {
        query_struct_variable_name,
        query_struct,
    })
}

struct MultiRequestTypeFunction {
    function_name: String,
    function_parameters: Vec<FunctionParameter>,
    request_media_type: String,
    request_content_variable_name: Option<String>,
}

fn generate_multi_request_type_functions(
    definition_path: &Vec<String>,
    name_mapping: &NameMapping,
    function_name: &str,
    path_parameter_code: &PathParameterCode,
    module_imports: &mut Vec<ModuleInfo>,
    query_parameter_code: &QueryParametersCode,
    request_entity: &RequestEntity,
) -> Vec<MultiRequestTypeFunction> {
    let mut function_definitions: Vec<MultiRequestTypeFunction> = vec![];
    if request_entity.content.len() < 2 {
        return function_definitions;
    }

    for (_, transfer_media_type) in &request_entity.content {
        let content_function_name = name_mapping.name_to_property_name(
            &definition_path,
            &format!(
                "{}{}",
                function_name,
                media_type_enum_name(&definition_path, name_mapping, &transfer_media_type)
            ),
        );
        let mut function_parameters: Vec<FunctionParameter> = vec![
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
        ];

        if path_parameter_code.parameters_struct.properties.len() > 0 {
            function_parameters.push(FunctionParameter {
                name: path_parameter_code.parameters_struct_variable_name.clone(),
                type_name: path_parameter_code.parameters_struct.name.clone(),
                reference: false,
            });
        }

        let query_struct = &query_parameter_code.query_struct;
        if query_struct.properties.len() > 0 {
            function_parameters.push(FunctionParameter {
                name: query_parameter_code.query_struct_variable_name.clone(),
                type_name: query_struct.name.clone(),
                reference: false,
            });
        }

        let mut request_content_variable_name = None;
        match transfer_media_type {
            TransferMediaType::ApplicationJson(ref type_definition_opt) => {
                match type_definition_opt {
                    Some(ref type_definition) => {
                        let variable_name =
                            name_mapping.name_to_property_name(definition_path, "content");
                        if let Some(ref module) = type_definition.module {
                            if !module_imports.contains(module) {
                                module_imports.push(module.clone());
                            }
                        }
                        function_parameters.push(FunctionParameter {
                            name: variable_name.clone(),
                            type_name: type_definition.name.clone(),
                            reference: false,
                        });
                        request_content_variable_name = Some(variable_name);
                    }
                    None => trace!("Empty request body not added to function params"),
                }
            }
            TransferMediaType::TextPlain => {
                let variable_name = name_mapping.name_to_property_name(definition_path, "content");
                function_parameters.push(FunctionParameter {
                    name: variable_name.clone(),
                    type_name: oas3_type_to_string(&oas3::spec::SchemaType::String),
                    reference: true,
                });

                request_content_variable_name = Some(variable_name);
            }
        }

        function_definitions.push(MultiRequestTypeFunction {
            function_name: content_function_name,
            function_parameters: function_parameters,
            request_content_variable_name: request_content_variable_name,
            request_media_type: match transfer_media_type {
                TransferMediaType::ApplicationJson(_) => "application/json".to_owned(),
                TransferMediaType::TextPlain => "text/plain".to_owned(),
            },
        });
    }

    function_definitions
}
