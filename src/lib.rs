use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::sync::Mutex;
use std::time::Duration;
use std::ffi::CString;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::oneshot;
use tracing::{info, warn, error};

/// Configuration for the template extension
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TemplateExtensionConfig {
    #[serde(default = "default_endpoint")]
    pub endpoint: String,

    #[serde(default = "default_interval")]
    pub interval_seconds: u64,

    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

fn default_endpoint() -> String {
    "http://localhost:8080".to_string()
}

fn default_interval() -> u64 {
    60
}

impl Default for TemplateExtensionConfig {
    fn default() -> Self {
        Self {
            endpoint: default_endpoint(),
            interval_seconds: default_interval(),
            metadata: HashMap::new(),
        }
    }
}

/// Errors that can occur in the template extension
#[derive(Debug, Error)]
pub enum TemplateExtensionError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Failed to start extension: {0}")]
    Start(String),

    #[error("Failed to stop extension: {0}")]
    Stop(String),

    #[error("Extension not initialized")]
    NotInitialized,

    #[error("Extension already running")]
    AlreadyRunning,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Extension trait for the template extension (for testing and internal use)
#[async_trait::async_trait]
pub trait Extension: Send + Sync {
    async fn start(&mut self) -> Result<(), TemplateExtensionError>;
    async fn stop(&mut self) -> Result<(), TemplateExtensionError>;
    fn name(&self) -> &str;
    fn version(&self) -> &str;
}

/// Opaque handle for the extension instance
pub struct ExtensionHandle {
    pub runtime: Option<tokio::runtime::Runtime>,
    pub shutdown_tx: Option<oneshot::Sender<()>>,
    pub config: TemplateExtensionConfig,
    pub is_running: bool,
}

impl ExtensionHandle {
    pub fn new(config: TemplateExtensionConfig) -> Self {
        Self {
            runtime: None,
            shutdown_tx: None,
            config,
            is_running: false,
        }
    }
}

/// Global extension instance (single instance for simplicity)
static EXTENSION_INSTANCE: Mutex<Option<ExtensionHandle>> = Mutex::new(None);

/// Initialize the extension with JSON configuration
/// Returns 0 on success, negative error code on failure
///
/// # Safety
/// This function dereferences a raw pointer. The caller must ensure that:
/// - `config_json` is a valid pointer to a null-terminated C string, or is null
/// - The pointed-to string is valid UTF-8
#[no_mangle]
pub unsafe extern "C" fn otelcol_extension_init(config_json: *const c_char) -> c_int {
    let config_str = unsafe {
        if config_json.is_null() {
            "{}"
        } else {
            match CStr::from_ptr(config_json).to_str() {
                Ok(s) => s,
                Err(_) => return -1,
            }
        }
    };

    let config: TemplateExtensionConfig = match serde_json::from_str(config_str) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to parse config: {}", e);
            return -2;
        }
    };

    info!("Initializing extension with config: {:?}", config);

    let mut instance = EXTENSION_INSTANCE.lock().unwrap();
    if instance.is_some() {
        warn!("Extension already initialized");
        return -3;
    }

    *instance = Some(ExtensionHandle::new(config));
    0
}

/// Start the extension
/// Returns 0 on success, negative error code on failure
#[no_mangle]
pub extern "C" fn otelcol_extension_start() -> c_int {
    let mut instance = EXTENSION_INSTANCE.lock().unwrap();
    let handle = match instance.as_mut() {
        Some(h) => h,
        None => return -1,
    };

    if handle.is_running {
        warn!("Extension already running");
        return -2;
    }

    // Create Tokio runtime
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            error!("Failed to create Tokio runtime: {}", e);
            return -3;
        }
    };

    let (tx, mut rx) = oneshot::channel();
    let config = handle.config.clone();
    let endpoint = config.endpoint.clone();
    let interval = config.interval_seconds;
    let metadata = config.metadata.clone();

    runtime.spawn(async move {
        let mut interval_timer = tokio::time::interval(Duration::from_secs(interval));

        loop {
            tokio::select! {
                _ = interval_timer.tick() => {
                    info!(
                        endpoint = %endpoint,
                        metadata = ?metadata,
                        "Template extension periodic task executed"
                    );
                    
                    // Example: Send metrics/data to endpoint
                    if let Err(e) = send_heartbeat(&endpoint, &metadata).await {
                        error!("Failed to send heartbeat: {}", e);
                    }
                }
                _ = &mut rx => {
                    info!("Extension received shutdown signal");
                    break;
                }
            }
        }
    });

    handle.runtime = Some(runtime);
    handle.shutdown_tx = Some(tx);
    handle.is_running = true;

    info!("Extension started successfully");
    0
}

/// Stop the extension
/// Returns 0 on success, negative error code on failure
#[no_mangle]
pub extern "C" fn otelcol_extension_stop() -> c_int {
    let mut instance = EXTENSION_INSTANCE.lock().unwrap();
    let handle = match instance.as_mut() {
        Some(h) => h,
        None => return -1,
    };

    if !handle.is_running {
        warn!("Extension not running");
        return -2;
    }

    // Send shutdown signal
    if let Some(tx) = handle.shutdown_tx.take() {
        let _ = tx.send(());
    }

    // Shutdown runtime
    if let Some(rt) = handle.runtime.take() {
        rt.shutdown_timeout(Duration::from_secs(5));
    }

    handle.is_running = false;
    info!("Extension stopped successfully");
    0
}

/// Shutdown and cleanup the extension
/// Returns 0 on success, negative error code on failure
#[no_mangle]
pub extern "C" fn otelcol_extension_shutdown() -> c_int {
    let mut instance = EXTENSION_INSTANCE.lock().unwrap();
    
    if let Some(mut handle) = instance.take() {
        if handle.is_running {
            // Force stop if still running
            if let Some(tx) = handle.shutdown_tx.take() {
                let _ = tx.send(());
            }
            if let Some(rt) = handle.runtime.take() {
                rt.shutdown_timeout(Duration::from_secs(5));
            }
        }
    }
    
    info!("Extension shutdown complete");
    0
}

/// Get extension version
#[no_mangle]
pub extern "C" fn otelcol_extension_version() -> *const c_char {
    static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
    VERSION.as_ptr() as *const c_char
}

/// Get extension name
#[no_mangle]
pub extern "C" fn otelcol_extension_name() -> *const c_char {
    static NAME: &str = concat!(env!("CARGO_PKG_NAME"), "\0");
    NAME.as_ptr() as *const c_char
}

/// Get build timestamp
#[no_mangle]
pub extern "C" fn otelcol_extension_build_timestamp() -> *const c_char {
    const BUILD_TIMESTAMP: &str = match option_env!("VERGEN_BUILD_TIMESTAMP") {
        Some(v) => v,
        None => "unknown",
    };
    // Add null terminator for C string
    let cstr = CString::new(BUILD_TIMESTAMP).unwrap_or_default();
    cstr.into_raw()
}

/// Get git commit hash
#[no_mangle]
pub extern "C" fn otelcol_extension_git_sha() -> *const c_char {
    const GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {
        Some(v) => v,
        None => "unknown",
    };
    let cstr = CString::new(GIT_SHA).unwrap_or_default();
    cstr.into_raw()
}

/// Example async function to send heartbeat
async fn send_heartbeat(endpoint: &str, metadata: &HashMap<String, String>) -> Result<(), reqwest::Error> {
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "metadata": metadata,
        "type": "heartbeat"
    });

    let _response = client
        .post(endpoint)
        .json(&payload)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    Ok(())
}

/// Initialize tracing/logging
///
/// # Safety
/// This function dereferences a raw pointer. The caller must ensure that:
/// - `log_level` is a valid pointer to a null-terminated C string, or is null
/// - The pointed-to string is valid UTF-8
#[no_mangle]
pub unsafe extern "C" fn otelcol_extension_init_logging(log_level: *const c_char) -> c_int {
    let level_str = unsafe {
        if log_level.is_null() {
            "info"
        } else {
            CStr::from_ptr(log_level).to_str().unwrap_or("info")
        }
    };

    let filter = match level_str {
        "trace" => "trace",
        "debug" => "debug",
        "info" => "info",
        "warn" => "warn",
        "error" => "error",
        _ => "info",
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    info!("Logging initialized at level: {}", filter);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TemplateExtensionConfig::default();
        assert_eq!(config.endpoint, "http://localhost:8080");
        assert_eq!(config.interval_seconds, 60);
    }

    #[test]
    fn test_config_serialization() {
        let config = TemplateExtensionConfig {
            endpoint: "http://test:8080".to_string(),
            interval_seconds: 30,
            metadata: {
                let mut m = HashMap::new();
                m.insert("key".to_string(), "value".to_string());
                m
            },
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: TemplateExtensionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.endpoint, parsed.endpoint);
        assert_eq!(config.interval_seconds, parsed.interval_seconds);
    }

    #[test]
    fn test_config_yaml_roundtrip() {
        let config = TemplateExtensionConfig {
            endpoint: "http://yaml:8080".to_string(),
            interval_seconds: 45,
            metadata: HashMap::new(),
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        let parsed: TemplateExtensionConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.endpoint, parsed.endpoint);
        assert_eq!(config.interval_seconds, parsed.interval_seconds);
    }
}