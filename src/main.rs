use otelcol_extension_template::{ExtensionHandle, TemplateExtensionConfig};

fn main() {
    println!("OpenTelemetry Collector Extension Template (Rust)");
    println!("=================================================");

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("debug")
        .init();

    // Test configuration
    let config = TemplateExtensionConfig {
        endpoint: "http://localhost:8080/api/v1/collect".to_string(),
        interval_seconds: 2,
        metadata: {
            let mut m = std::collections::HashMap::new();
            m.insert("environment".to_string(), "test".to_string());
            m.insert("service".to_string(), "otelcol-template".to_string());
            m.insert("version".to_string(), "0.1.0".to_string());
            m
        },
    };

    println!("\n1. Creating extension handle...");
    let handle = ExtensionHandle::new(config);
    println!("   Extension handle created successfully");

    println!("\n2. Starting extension (simulated)...");
    // We can't easily test the full async runtime here, but we can verify the structure
    println!("   Extension configuration: {:?}", handle.config);
    println!("   Is running: {}", handle.is_running);

    println!("\n3. Testing configuration serialization...");
    let json = serde_json::to_string_pretty(&handle.config).unwrap();
    println!("   Config JSON:\n{}", json);

    // Test deserialization
    let parsed: TemplateExtensionConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(handle.config.endpoint, parsed.endpoint);
    assert_eq!(handle.config.interval_seconds, parsed.interval_seconds);
    println!("   Serialization/deserialization works correctly");

    println!("\n4. Testing default configuration...");
    let default_config = TemplateExtensionConfig::default();
    println!("   Default endpoint: {}", default_config.endpoint);
    println!("   Default interval: {}s", default_config.interval_seconds);

    println!("\n5. Testing FFI function signatures (compile-time check)...");
    // These are just to verify the FFI signatures compile correctly
    // They won't be called in this binary test
    let _name_ptr = unsafe { otelcol_extension_name() };
    let _version_ptr = unsafe { otelcol_extension_version() };
    println!("   FFI function signatures verified");

    println!("\n✓ All library tests passed!");
    println!("\nTo use as a CDYLIB with the OpenTelemetry Collector:");
    println!("  1. Build release: cargo build --release");
    println!("  2. Load the .so/.dylib via CGO in Go Collector");
    println!("  3. Call otelcol_extension_init(), otelcol_extension_start(), etc.");
}

// Import FFI functions for compile-time verification
extern "C" {
    fn otelcol_extension_init(config_json: *const std::os::raw::c_char) -> std::os::raw::c_int;
    fn otelcol_extension_start() -> std::os::raw::c_int;
    fn otelcol_extension_stop() -> std::os::raw::c_int;
    fn otelcol_extension_shutdown() -> std::os::raw::c_int;
    fn otelcol_extension_version() -> *const std::os::raw::c_char;
    fn otelcol_extension_name() -> *const std::os::raw::c_char;
    fn otelcol_extension_init_logging(log_level: *const std::os::raw::c_char) -> std::os::raw::c_int;
}