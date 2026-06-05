use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;

use zpic_config::UploaderConfigItem;
use zpic_core::error::{Result, ZpicError};
use zpic_core::upload::Uploader;

use crate::manifest::UploaderFieldSchema;

type InstantiateFn = fn(&str, &UploaderConfigItem) -> Result<Box<dyn Uploader>>;
type ValidateFn = fn(&str, &UploaderConfigItem) -> Result<()>;

#[async_trait]
pub trait UploaderRunner: Send + Sync {
    fn instantiate(&self, uploader_type: &str, item: &UploaderConfigItem) -> Result<Box<dyn Uploader>>;

    fn validate(&self, uploader_type: &str, item: &UploaderConfigItem) -> Result<()>;
}

#[derive(Clone)]
struct FunctionUploaderRunner {
    instantiate_fn: InstantiateFn,
    validate_fn: ValidateFn,
}

#[async_trait]
impl UploaderRunner for FunctionUploaderRunner {
    fn instantiate(
        &self,
        uploader_type: &str,
        item: &UploaderConfigItem,
    ) -> Result<Box<dyn Uploader>> {
        (self.instantiate_fn)(uploader_type, item)
    }

    fn validate(&self, uploader_type: &str, item: &UploaderConfigItem) -> Result<()> {
        (self.validate_fn)(uploader_type, item)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploaderDescriptorKind {
    Builtin,
    Plugin,
}

#[derive(Clone)]
pub struct UploaderDescriptor {
    pub kind: UploaderDescriptorKind,
    pub type_name: String,
    pub display_name: String,
    pub aliases: Vec<String>,
    pub fields: Vec<UploaderFieldSchema>,
    runner: Arc<dyn UploaderRunner>,
}

impl UploaderDescriptor {
    pub fn instantiate(&self, uploader_type: &str, item: &UploaderConfigItem) -> Result<Box<dyn Uploader>> {
        self.runner.instantiate(uploader_type, item)
    }

    pub fn validate(&self, uploader_type: &str, item: &UploaderConfigItem) -> Result<()> {
        self.runner.validate(uploader_type, item)
    }
}

pub fn builtin_uploader_descriptor(
    type_name: impl Into<String>,
    display_name: impl Into<String>,
    aliases: Vec<String>,
    fields: Vec<UploaderFieldSchema>,
    instantiate_fn: InstantiateFn,
    validate_fn: ValidateFn,
) -> UploaderDescriptor {
    UploaderDescriptor {
        kind: UploaderDescriptorKind::Builtin,
        type_name: type_name.into(),
        display_name: display_name.into(),
        aliases,
        fields,
        runner: Arc::new(FunctionUploaderRunner {
            instantiate_fn,
            validate_fn,
        }),
    }
}

pub fn plugin_uploader_descriptor(
    type_name: impl Into<String>,
    display_name: impl Into<String>,
    aliases: Vec<String>,
    fields: Vec<UploaderFieldSchema>,
    runner: Arc<dyn UploaderRunner>,
) -> UploaderDescriptor {
    UploaderDescriptor {
        kind: UploaderDescriptorKind::Plugin,
        type_name: type_name.into(),
        display_name: display_name.into(),
        aliases,
        fields,
        runner,
    }
}

pub struct UploaderRegistry {
    descriptors: BTreeMap<String, UploaderDescriptor>,
    aliases: BTreeMap<String, String>,
}

impl Default for UploaderRegistry {
    fn default() -> Self {
        Self {
            descriptors: BTreeMap::new(),
            aliases: BTreeMap::new(),
        }
    }
}

impl UploaderRegistry {
    pub fn register(&mut self, descriptor: UploaderDescriptor) -> Result<()> {
        let canonical = descriptor.type_name.trim().to_ascii_lowercase();
        if canonical.is_empty() {
            return Err(ZpicError::ConfigInvalid(
                "uploader descriptor type name can not be empty".into(),
            ));
        }
        if self.descriptors.contains_key(&canonical) || self.aliases.contains_key(&canonical) {
            return Err(ZpicError::ConfigInvalid(format!(
                "duplicate uploader descriptor registration for `{}`",
                descriptor.type_name
            )));
        }

        for alias in &descriptor.aliases {
            let normalized = alias.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                continue;
            }
            if self.aliases.contains_key(&normalized) || self.descriptors.contains_key(&normalized)
            {
                return Err(ZpicError::ConfigInvalid(format!(
                    "duplicate uploader alias registration for `{}`",
                    alias
                )));
            }
        }

        for alias in &descriptor.aliases {
            let normalized = alias.trim().to_ascii_lowercase();
            if !normalized.is_empty() {
                self.aliases.insert(normalized, canonical.clone());
            }
        }

        self.descriptors.insert(canonical, descriptor);
        Ok(())
    }

    pub fn resolve(&self, token: &str) -> Option<&UploaderDescriptor> {
        let normalized = token.trim().to_ascii_lowercase();
        if let Some(descriptor) = self.descriptors.get(&normalized) {
            return Some(descriptor);
        }
        let canonical = self.aliases.get(&normalized)?;
        self.descriptors.get(canonical)
    }

    pub fn canonical_type(&self, token: &str) -> Option<&str> {
        self.resolve(token).map(|descriptor| descriptor.type_name.as_str())
    }

    pub fn descriptors(&self) -> Vec<&UploaderDescriptor> {
        let mut out: Vec<&UploaderDescriptor> = self.descriptors.values().collect();
        out.sort_by(|a, b| descriptor_sort_key(a).cmp(&descriptor_sort_key(b)));
        out
    }
}

fn descriptor_sort_key(descriptor: &UploaderDescriptor) -> (u8, u8, &str) {
    let builtin_rank = match descriptor.type_name.as_str() {
        "local" => 0,
        "github" => 1,
        "s3" => 2,
        _ => 10,
    };
    let kind_rank = match descriptor.kind {
        UploaderDescriptorKind::Builtin => 0,
        UploaderDescriptorKind::Plugin => 1,
    };
    (kind_rank, builtin_rank, descriptor.type_name.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use zpic_core::upload::{UploadOutput, UploadRequest};

    #[derive(Debug)]
    struct StubUploader;

    #[async_trait]
    impl Uploader for StubUploader {
        fn name(&self) -> &str {
            "stub"
        }

        async fn upload(&self, _request: UploadRequest) -> Result<UploadOutput> {
            Err(ZpicError::Internal("unused".into()))
        }
    }

    fn instantiate(_: &str, _: &UploaderConfigItem) -> Result<Box<dyn Uploader>> {
        Ok(Box::new(StubUploader))
    }

    fn validate(_: &str, _: &UploaderConfigItem) -> Result<()> {
        Ok(())
    }

    #[test]
    fn resolves_aliases_case_insensitively() {
        let mut registry = UploaderRegistry::default();
        registry
            .register(builtin_uploader_descriptor(
                "demo",
                "Demo",
                vec!["DemoAlias".into()],
                vec![],
                instantiate,
                validate,
            ))
            .unwrap();

        let descriptor = registry.resolve("demoalias").unwrap();
        assert_eq!(descriptor.type_name, "demo");
    }

    #[test]
    fn rejects_duplicate_aliases() {
        let mut registry = UploaderRegistry::default();
        registry
            .register(builtin_uploader_descriptor(
                "one",
                "One",
                vec!["same".into()],
                vec![],
                instantiate,
                validate,
            ))
            .unwrap();

        let err = registry
            .register(builtin_uploader_descriptor(
                "two",
                "Two",
                vec!["same".into()],
                vec![],
                instantiate,
                validate,
            ))
            .unwrap_err();
        assert!(err.to_string().contains("duplicate uploader alias"));
    }

    #[test]
    fn instantiates_registered_uploader() {
        let mut registry = UploaderRegistry::default();
        registry
            .register(builtin_uploader_descriptor(
                "demo",
                "Demo",
                vec![],
                vec![],
                instantiate,
                validate,
            ))
            .unwrap();

        let item = UploaderConfigItem {
            id: "id".into(),
            config_name: "Default".into(),
            created_at: 0,
            updated_at: 0,
            fields: BTreeMap::new(),
        };
        let uploader = registry.resolve("demo").unwrap().instantiate("demo", &item).unwrap();
        assert_eq!(uploader.name(), "stub");
    }
}
