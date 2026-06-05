use serde::Deserialize;

fn default_wasm_file() -> String {
    "plugin.wasm".to_string()
}

/// One input field exposed by an uploader for guided setup.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UploaderFieldSchema {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub default: Option<String>,
}

/// Declares one uploader exposed by a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PluginUploaderManifest {
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub picgo_aliases: Vec<String>,
    #[serde(default)]
    pub fields: Vec<UploaderFieldSchema>,
}

impl PluginUploaderManifest {
    pub fn display_name(&self) -> &str {
        self.display_name
            .as_deref()
            .unwrap_or(self.type_name.as_str())
    }
}

/// Placeholder capability declaration for future stages such as transformers
/// and hooks. These are parsed now so the manifest shape can evolve without
/// forcing another top-level format reset.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FutureCapabilityManifest {
    pub name: String,
}

/// Top-level plugin manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default = "default_wasm_file", rename = "wasm")]
    pub wasm_file: String,
    #[serde(default)]
    pub uploaders: Vec<PluginUploaderManifest>,
    #[serde(default)]
    pub transformers: Vec<FutureCapabilityManifest>,
    #[serde(default)]
    pub hooks: Vec<FutureCapabilityManifest>,
}

impl PluginManifest {
    pub fn from_toml(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_with_uploader_fields() {
        let manifest = PluginManifest::from_toml(
            r#"
id = "demo"
name = "Demo"
version = "0.1.0"
wasm = "plugin.wasm"

[[uploaders]]
type = "demo-uploader"
display_name = "Demo Uploader"
picgo_aliases = ["demo-picgo"]

[[uploaders.fields]]
key = "token"
label = "API Token"
required = true
secret = true
"#,
        )
        .unwrap();

        assert_eq!(manifest.id, "demo");
        assert_eq!(manifest.uploaders.len(), 1);
        assert_eq!(manifest.uploaders[0].type_name, "demo-uploader");
        assert_eq!(manifest.uploaders[0].fields[0].key, "token");
    }
}
