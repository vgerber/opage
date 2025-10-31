use std::collections::HashMap;

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
    Primitive(PrimitiveDefinition),
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

pub type ObjectDatabase = HashMap<String, ObjectDefinition>;

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
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructDefinition {
    pub used_modules: Vec<ModuleInfo>,
    pub name: String,
    pub properties: HashMap<String, PropertyDefinition>,
    pub local_objects: HashMap<String, Box<ObjectDefinition>>,
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrimitiveDefinition {
    pub name: String,
    pub primitive_type: TypeDefinition,
}

pub fn to_unique_list(modules: &Vec<ModuleInfo>) -> Vec<ModuleInfo> {
    let mut unique_modules: Vec<ModuleInfo> = vec![];
    for module in modules {
        if !unique_modules.iter().any(|unique_module| {
            unique_module.name == module.name && unique_module.path == module.path
        }) {
            unique_modules.push(module.clone());
        }
    }
    unique_modules
}
