use askama::Template;

use crate::parser::component::object_definition::types::{
    to_unique_list, EnumDefinition, EnumValue, ModuleInfo, PrimitveDefinition, PropertyDefinition,
    StructDefinition,
};

pub struct PrimitiveDefinitionTemplate {
    pub name: String,
    pub type_name: String,
}

impl From<&PrimitveDefinition> for PrimitiveDefinitionTemplate {
    fn from(primitive_definition: &PrimitveDefinition) -> Self {
        PrimitiveDefinitionTemplate {
            name: primitive_definition.name.clone(),
            type_name: primitive_definition.primitive_type.name.clone(),
        }
    }
}

impl From<&PrimitveDefinition> for BaseTemplate {
    fn from(primitive_definition: &PrimitveDefinition) -> Self {
        BaseTemplate {
            struct_definitions: vec![],
            enum_definitions: vec![],
            primitive_definitions: vec![PrimitiveDefinitionTemplate {
                name: primitive_definition.name.clone(),
                type_name: primitive_definition.primitive_type.name.clone(),
            }],
            module_imports: to_unique_list(
                &primitive_definition
                    .primitive_type
                    .module
                    .as_ref()
                    .map_or(vec![], |module| vec![module.clone()]),
            ),
        }
    }
}

pub struct EnumValueTemplate {
    pub name: String,
    pub value_type: String,
}

impl From<&EnumValue> for EnumValueTemplate {
    fn from(enum_value: &EnumValue) -> Self {
        EnumValueTemplate {
            name: enum_value.name.clone(),
            value_type: enum_value.value_type.name.clone(),
        }
    }
}

pub struct EnumDefinitionTemplate {
    pub serializable: bool,
    pub name: String,
    pub values: Vec<EnumValueTemplate>,
}

impl EnumDefinitionTemplate {
    pub fn serializable(mut self, serializable: bool) -> Self {
        self.serializable = serializable;
        self
    }
}

impl From<&EnumDefinition> for EnumDefinitionTemplate {
    fn from(enum_definition: &EnumDefinition) -> Self {
        EnumDefinitionTemplate {
            serializable: true,
            name: enum_definition.name.clone(),
            values: enum_definition
                .values
                .iter()
                .map(|(_, value)| value.into())
                .collect(),
        }
    }
}

impl From<&EnumDefinition> for BaseTemplate {
    fn from(enum_definition: &EnumDefinition) -> Self {
        BaseTemplate {
            struct_definitions: vec![],
            enum_definitions: vec![EnumDefinitionTemplate::from(enum_definition)],
            primitive_definitions: vec![],
            module_imports: to_unique_list(
                &enum_definition
                    .get_required_modules()
                    .iter()
                    .map(|&module| module.clone())
                    .collect(),
            ),
        }
    }
}

pub struct PropertyTemplate {
    pub real_name: String,
    pub name: String,
    pub type_name: String,
    pub required: bool,
}

impl From<&PropertyDefinition> for PropertyTemplate {
    fn from(property: &PropertyDefinition) -> Self {
        PropertyTemplate {
            real_name: property.real_name.clone(),
            name: property.name.clone(),
            type_name: property.type_name.clone(),
            required: property.required,
        }
    }
}

pub struct StructDefinitionTemplate {
    pub serializable: bool,
    pub name: String,
    pub properties: Vec<PropertyDefinition>,
}

impl StructDefinitionTemplate {
    pub fn serializable(mut self, serializable: bool) -> Self {
        self.serializable = serializable;
        self
    }
}

impl From<&StructDefinition> for StructDefinitionTemplate {
    fn from(struct_definition: &StructDefinition) -> Self {
        StructDefinitionTemplate {
            serializable: true,
            name: struct_definition.name.clone(),
            properties: struct_definition
                .properties
                .iter()
                .map(|(_, property)| property.clone())
                .collect(),
        }
    }
}

impl From<&StructDefinition> for BaseTemplate {
    fn from(struct_definition: &StructDefinition) -> Self {
        BaseTemplate {
            struct_definitions: vec![StructDefinitionTemplate::from(struct_definition)],
            enum_definitions: vec![],
            primitive_definitions: vec![],
            module_imports: to_unique_list(
                &struct_definition
                    .get_required_modules()
                    .iter()
                    .map(|&module| module.clone())
                    .collect(),
            ),
        }
    }
}

#[derive(Template)]
#[template(path = "rust_reqwest_async/base.rs.jinja", ext = "rs")]
pub struct BaseTemplate {
    pub module_imports: Vec<ModuleInfo>,
    pub struct_definitions: Vec<StructDefinitionTemplate>,
    pub enum_definitions: Vec<EnumDefinitionTemplate>,
    pub primitive_definitions: Vec<PrimitiveDefinitionTemplate>,
}
