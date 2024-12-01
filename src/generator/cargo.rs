use crate::utils::config::ProjectMetadata;

pub fn generate_cargo_content(project_metadata: &ProjectMetadata) -> Result<String, String> {
    Ok(format!(
        "[package]
name = \"{name}\"
version = \"{version}\"
edition = \"2021\"

[dependencies]
reqwest = {{ version = \"0.12.9\", features = [\"json\"] }}
serde = {{ version = \"1.0.215\", features = [\"derive\"] }}
serde_json = \"1.0.132\"
tungstenite = \"0.24.0\"
",
        name = project_metadata.name,
        version = project_metadata.version,
    ))
}
