use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine as Base64Engine;
use serde::{Deserialize, Serialize};
use toml::Value as TomlValue;
use wasmtime::{Engine, Instance, Memory, Module, Store, TypedFunc};

use zpic_config::UploaderConfigItem;
use zpic_core::error::{Result, ZpicError};
use zpic_core::upload::{UploadOutput, UploadRequest, Uploader};

use crate::registry::{plugin_uploader_descriptor, UploaderDescriptor, UploaderRunner};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmUploadRequest {
    pub uploader_type: String,
    pub target_key: String,
    pub dry_run: bool,
    pub source_path: String,
    pub file_name: String,
    pub mime: String,
    pub size: u64,
    pub alt: Option<String>,
    pub config: BTreeMap<String, serde_json::Value>,
    pub bytes_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmUploadResponse {
    pub url: Option<String>,
    pub key: Option<String>,
    pub markdown: Option<String>,
    pub mime: Option<String>,
    pub size: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub uploader: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfigValidationRequest {
    pub uploader_type: String,
    pub config: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfigValidationResponse {
    #[serde(default = "default_validation_ok")]
    pub ok: bool,
    pub error: Option<String>,
}

fn default_validation_ok() -> bool {
    true
}

pub struct WasmPluginRunner {
    plugin_id: String,
    wasm_path: PathBuf,
    engine: Engine,
}

impl WasmPluginRunner {
    pub fn new(plugin_id: impl Into<String>, wasm_path: PathBuf) -> Result<Self> {
        let config = wasmtime::Config::new();
        let engine = Engine::new(&config).map_err(|e| ZpicError::Internal(e.to_string()))?;
        Ok(Self {
            plugin_id: plugin_id.into(),
            wasm_path,
            engine,
        })
    }

    pub fn into_descriptor(
        self,
        type_name: String,
        display_name: String,
        aliases: Vec<String>,
        fields: Vec<crate::manifest::UploaderFieldSchema>,
    ) -> UploaderDescriptor {
        plugin_uploader_descriptor(type_name, display_name, aliases, fields, Arc::new(self))
    }

    fn module(&self) -> Result<Module> {
        Module::from_file(&self.engine, &self.wasm_path)
            .map_err(|e| ZpicError::UploadFailed(format!("plugin `{}` load failed: {e}", self.plugin_id)))
    }

    fn call_export(&self, export_name: &str, request: &[u8]) -> Result<Vec<u8>> {
        let module = self.module()?;
        let mut store = Store::new(&self.engine, ());
        let instance =
            Instance::new(&mut store, &module, &[]).map_err(|e| {
                ZpicError::UploadFailed(format!(
                    "plugin `{}` instantiation failed: {e}",
                    self.plugin_id
                ))
            })?;
        let memory = instance.get_memory(&mut store, "memory").ok_or_else(|| {
            ZpicError::UploadFailed(format!(
                "plugin `{}` missing exported memory",
                self.plugin_id
            ))
        })?;
        let alloc: TypedFunc<i32, i32> = instance
            .get_typed_func(&mut store, "zpic_alloc")
            .map_err(|e| {
                ZpicError::UploadFailed(format!(
                    "plugin `{}` missing `zpic_alloc`: {e}",
                    self.plugin_id
                ))
            })?;
        let free: Option<TypedFunc<(i32, i32), ()>> =
            instance.get_typed_func(&mut store, "zpic_free").ok();
        let call: TypedFunc<(i32, i32), i64> = instance
            .get_typed_func(&mut store, export_name)
            .map_err(|e| {
                ZpicError::UploadFailed(format!(
                    "plugin `{}` missing `{}` export: {e}",
                    self.plugin_id, export_name
                ))
            })?;

        let ptr = alloc
            .call(&mut store, request.len() as i32)
            .map_err(|e| ZpicError::UploadFailed(format!("plugin `{}` alloc failed: {e}", self.plugin_id)))?;
        memory
            .write(&mut store, ptr as usize, request)
            .map_err(|e| ZpicError::UploadFailed(format!("plugin `{}` memory write failed: {e}", self.plugin_id)))?;

        let packed = call
            .call(&mut store, (ptr, request.len() as i32))
            .map_err(|e| ZpicError::UploadFailed(format!("plugin `{}` call failed: {e}", self.plugin_id)))?;

        if let Some(free) = free.as_ref() {
            let _ = free.call(&mut store, (ptr, request.len() as i32));
        }

        let (resp_ptr, resp_len) = unpack_ptr_len(packed)?;
        let bytes = read_bytes(&memory, &mut store, resp_ptr, resp_len)?;
        if let Some(free) = free.as_ref() {
            let _ = free.call(&mut store, (resp_ptr, resp_len));
        }
        Ok(bytes)
    }

    fn validate_config_fields(
        &self,
        uploader_type: &str,
        fields: &BTreeMap<String, TomlValue>,
    ) -> Result<()> {
        let request = WasmConfigValidationRequest {
            uploader_type: uploader_type.to_string(),
            config: to_json_map(fields)?,
        };
        let bytes = serde_json::to_vec(&request).map_err(|e| ZpicError::Internal(e.to_string()))?;
        match self.call_export("zpic_validate_config", &bytes) {
            Ok(response_bytes) => {
                let response: WasmConfigValidationResponse = serde_json::from_slice(&response_bytes)
                    .map_err(|e| ZpicError::UploadFailed(format!("plugin `{}` returned invalid validation JSON: {e}", self.plugin_id)))?;
                if response.ok {
                    Ok(())
                } else {
                    Err(ZpicError::ConfigInvalid(
                        response
                            .error
                            .unwrap_or_else(|| format!("plugin `{}` rejected the config", self.plugin_id)),
                    ))
                }
            }
            Err(err) if err.to_string().contains("missing `zpic_validate_config`") => Ok(()),
            Err(err) => Err(err),
        }
    }

    fn upload_impl(
        &self,
        uploader_type: &str,
        fields: &BTreeMap<String, TomlValue>,
        request: UploadRequest,
    ) -> Result<UploadOutput> {
        let payload = WasmUploadRequest {
            uploader_type: uploader_type.to_string(),
            target_key: request.context.target_key.clone(),
            dry_run: request.context.dry_run,
            source_path: request.input.source_path.to_string_lossy().into_owned(),
            file_name: request.input.file_name.clone(),
            mime: request.input.mime.clone(),
            size: request.input.size,
            alt: request.input.alt.clone(),
            config: to_json_map(fields)?,
            bytes_base64: base64::engine::general_purpose::STANDARD.encode(&request.input.bytes),
        };
        let request_bytes =
            serde_json::to_vec(&payload).map_err(|e| ZpicError::Internal(e.to_string()))?;
        let response_bytes = self.call_export("zpic_upload", &request_bytes)?;
        let response: WasmUploadResponse = serde_json::from_slice(&response_bytes).map_err(|e| {
            ZpicError::UploadFailed(format!(
                "plugin `{}` returned invalid upload JSON: {e}",
                self.plugin_id
            ))
        })?;

        if let Some(error) = response.error {
            return Err(ZpicError::UploadFailed(error));
        }

        let url = response.url.ok_or_else(|| {
            ZpicError::UploadFailed(format!(
                "plugin `{}` did not return a URL",
                self.plugin_id
            ))
        })?;
        let key = response
            .key
            .unwrap_or_else(|| request.context.target_key.clone());
        let markdown = response
            .markdown
            .unwrap_or_else(|| format!("![{}]({})", request.input.file_name, url));

        Ok(UploadOutput {
            source: request.input.source_path.to_string_lossy().into_owned(),
            url,
            key,
            markdown,
            mime: response.mime.unwrap_or(request.input.mime),
            size: response.size.unwrap_or(request.input.size),
            width: response.width,
            height: response.height,
            uploader: response
                .uploader
                .unwrap_or_else(|| uploader_type.to_string()),
        })
    }
}

#[async_trait]
impl UploaderRunner for WasmPluginRunner {
    fn instantiate(&self, uploader_type: &str, item: &UploaderConfigItem) -> Result<Box<dyn Uploader>> {
        self.validate(uploader_type, item)?;
        Ok(Box::new(WasmPluginUploader {
            uploader_type: uploader_type.to_string(),
            fields: item.fields.clone(),
            runner: Arc::new(Self {
                plugin_id: self.plugin_id.clone(),
                wasm_path: self.wasm_path.clone(),
                engine: self.engine.clone(),
            }),
        }))
    }

    fn validate(&self, uploader_type: &str, item: &UploaderConfigItem) -> Result<()> {
        self.validate_config_fields(uploader_type, &item.fields)
    }
}

struct WasmPluginUploader {
    uploader_type: String,
    fields: BTreeMap<String, TomlValue>,
    runner: Arc<WasmPluginRunner>,
}

#[async_trait]
impl Uploader for WasmPluginUploader {
    fn name(&self) -> &str {
        self.uploader_type.as_str()
    }

    async fn upload(&self, request: UploadRequest) -> Result<UploadOutput> {
        self.runner
            .upload_impl(&self.uploader_type, &self.fields, request)
    }
}

fn unpack_ptr_len(packed: i64) -> Result<(i32, i32)> {
    let raw = packed as u64;
    let ptr = (raw & 0xffff_ffff) as i32;
    let len = (raw >> 32) as i32;
    if ptr < 0 || len < 0 {
        return Err(ZpicError::UploadFailed(
            "plugin returned an invalid pointer/length pair".into(),
        ));
    }
    Ok((ptr, len))
}

fn read_bytes(memory: &Memory, store: &mut Store<()>, ptr: i32, len: i32) -> Result<Vec<u8>> {
    let mut buf = vec![0_u8; len as usize];
    memory
        .read(store, ptr as usize, &mut buf)
        .map_err(|e| ZpicError::UploadFailed(format!("plugin memory read failed: {e}")))?;
    Ok(buf)
}

fn to_json_map(fields: &BTreeMap<String, TomlValue>) -> Result<BTreeMap<String, serde_json::Value>> {
    let value = serde_json::to_value(fields).map_err(|e| ZpicError::Internal(e.to_string()))?;
    match value {
        serde_json::Value::Object(map) => Ok(map.into_iter().collect()),
        _ => Err(ZpicError::Internal(
            "uploader config fields did not serialize to an object".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use bytes::Bytes;
    use tempfile::tempdir;
    use zpic_core::config::ZpicConfig;
    use zpic_core::upload::{UploadContext, UploadInput};

    #[derive(Debug)]
    struct StubConfig;

    impl ZpicConfig for StubConfig {
        fn source(&self) -> &str {
            "test"
        }
    }

    fn write_plugin(dir: &std::path::Path, body: &str) -> PathBuf {
        let path = dir.join("plugin.wasm");
        fs::write(&path, body).unwrap();
        path
    }

    #[tokio::test]
    async fn wasm_runner_uploads_via_fixture_module() {
        let dir = tempdir().unwrap();
        let wasm = write_plugin(
            dir.path(),
            r#"(module
  (memory (export "memory") 1)
  (global $heap (mut i32) (i32.const 2048))
  (func (export "zpic_alloc") (param $len i32) (result i32)
    (local $ptr i32)
    global.get $heap
    local.set $ptr
    global.get $heap
    local.get $len
    i32.add
    global.set $heap
    local.get $ptr)
  (func (export "zpic_upload") (param $ptr i32) (param $len i32) (result i64)
    i64.const 665719930880)
  (data (i32.const 0) "{\"url\":\"https://plugins.example/uploaded.png\",\"key\":\"plugin/key.png\",\"markdown\":\"![plugin](https://plugins.example/uploaded.png)\",\"uploader\":\"plugin-demo\"}")
)"#,
        );
        let runner = WasmPluginRunner::new("demo", wasm).unwrap();
        let uploader = runner
            .instantiate(
                "plugin-demo",
                &UploaderConfigItem {
                    id: "id".into(),
                    config_name: "Default".into(),
                    created_at: 0,
                    updated_at: 0,
                    fields: BTreeMap::new(),
                },
            )
            .unwrap();
        let request = UploadRequest {
            context: UploadContext::new("images/plugin.png".into(), Arc::new(StubConfig)),
            input: UploadInput::new(
                PathBuf::from("cover.png"),
                "cover.png",
                "image/png",
                Bytes::from_static(b"png"),
            ),
        };
        let output = uploader.upload(request).await.unwrap();
        assert_eq!(output.url, "https://plugins.example/uploaded.png");
        assert_eq!(output.key, "plugin/key.png");
        assert_eq!(output.uploader, "plugin-demo");
    }
}
