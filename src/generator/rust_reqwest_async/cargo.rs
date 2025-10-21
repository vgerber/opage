use askama::Template;

use crate::utils::config::ProjectMetadata;

#[derive(Template)]
#[template(path = "rust_reqwest_async/cargo.template.txt", ext = "txt")]
struct CargoTomlTemplate {
    name: String,
    version: String,
}

pub fn generate_cargo_content(project_metadata: &ProjectMetadata) -> Result<String, String> {
    let template = CargoTomlTemplate {
        name: project_metadata.name.clone(),
        version: project_metadata.version.clone(),
    };
    template.render().map_err(|e| e.to_string())
}
