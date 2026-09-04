# OpenTelemetry Collector Extension Template (Rust)

A template project for building OpenTelemetry Collector extensions in Rust, designed to be loaded as a dynamic library (CDYLIB) via CGO from the Go-based OpenTelemetry Collector.

## Features

- **FFI Interface**: C-compatible FFI functions for integration with Go Collector
- **Configuration**: Serde-based configuration with JSON and YAML support
- **Async Runtime**: Tokio-based async implementation with proper lifecycle management
- **Logging**: Tracing-based structured logging with configurable levels
- **Error Handling**: thiserror/anyhow for robust error handling
- **Build Optimization**: Release profile optimized for size and performance
- **Build Info**: Automatic version, git commit, and build timestamp embedding

## Project Structure

```plaintext
├── Cargo.toml              # Project configuration and dependencies
├── build.rs                # Build script for version info
├── src/
│   ├── lib.rs              # Library code (extension implementation + FFI)
│   └── main.rs             # Binary for testing the extension
├── config/
│   └── config.yaml         # Example configuration
└── README.md               # This file
```

## Requirements

- Rust 1.75+
- OpenTelemetry Collector development environment (for integration testing)

## Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run the example binary
cargo run
```

The release build produces a shared library:

- Linux: `target/release/libotelcol_extension_template.so`
- macOS: `target/release/libotelcol_extension_template.dylib`
- Windows: `target/release/otelcol_extension_template.dll`

## Configuration

The extension accepts JSON configuration via the FFI `otelcol_extension_init` function. The configuration structure:

```json
{
  "endpoint": "http://localhost:8080/api/v1/collect",
  "interval_seconds": 30,
  "metadata": {
    "environment": "production",
    "team": "observability"
  }
}
```

### Configuration Options

| Field | Type | Default | Description |
| ----- | ---- | ------- | ----------- |
| `endpoint` | string | `http://localhost:8080` | Target endpoint for the extension |
| `interval_seconds` | integer | `60` | Interval for periodic tasks |
| `metadata` | map[string]string | `{}` | Additional metadata key-value pairs |

The `config/config.yaml` file shows how to configure this in the OpenTelemetry Collector's YAML configuration.

## FFI Functions

The library exports the following C-compatible functions:

| Function | Description |
| -------- | ----------- |
| `otelcol_extension_init(config_json)` | Initialize with JSON config string |
| `otelcol_extension_start()` | Start the extension background tasks |
| `otelcol_extension_stop()` | Stop the extension gracefully |
| `otelcol_extension_shutdown()` | Full shutdown and cleanup |
| `otelcol_extension_version()` | Get extension version string |
| `otelcol_extension_name()` | Get extension name |
| `otelcol_extension_build_timestamp()` | Get build timestamp |
| `otelcol_extension_git_sha()` | Get git commit hash |
| `otelcol_extension_init_logging(log_level)` | Initialize logging |

### Return Codes

All init/start/stop/shutdown functions return:

- `0` - Success
- `-1` - Null pointer or not initialized
- `-2` - Config parse error / already running / not running
- `-3` - Already initialized / runtime creation failed

## Using in OpenTelemetry Collector

### 1. Build as a shared library

```bash
cargo build --release
```

### 2. Create a Go wrapper (CGO)

Create a Go file to load the Rust library:

```go
// extension.go
package main

/*
#cgo LDFLAGS: -L${SRCDIR}/../target/release -lotelcol_extension_template
#include <stdlib.h>

extern int otelcol_extension_init(const char* config_json);
extern int otelcol_extension_start();
extern int otelcol_extension_stop();
extern int otelcol_extension_shutdown();
extern const char* otelcol_extension_name();
extern const char* otelcol_extension_version();
*/
import "C"
import (
    "fmt"
    "unsafe"
)

func main() {
    config := `{"endpoint":"http://localhost:8080","interval_seconds":30}`
    cConfig := C.CString(config)
    defer C.free(unsafe.Pointer(cConfig))
    
    if ret := C.otelcol_extension_init(cConfig); ret != 0 {
        fmt.Printf("Init failed: %d\n", ret)
        return
    }
    
    name := C.GoString(C.otelcol_extension_name())
    version := C.GoString(C.otelcol_extension_version())
    fmt.Printf("Loaded extension: %s v%s\n", name, version)
    
    if ret := C.otelcol_extension_start(); ret != 0 {
        fmt.Printf("Start failed: %d\n", ret)
        return
    }
    
    // ... run collector ...
    
    C.otelcol_extension_stop()
    C.otelcol_extension_shutdown()
}
```

### 3. Configure the Collector

Add to your Collector's `config.yaml`:

```yaml
extensions:
  template:
    endpoint: "http://your-endpoint:8080"
    interval_seconds: 30

service:
  extensions: [template]
  pipelines:
    # ... your pipelines
```

## Development

### Adding New Features

1. Modify `src/lib.rs` for the extension logic
2. Update `TemplateExtensionConfig` for new configuration options
3. Add corresponding FFI functions if needed
4. Add tests in `src/lib.rs` or separate test files

### Testing

```bash
# Run unit tests
cargo test

# Run with logging
RUST_LOG=debug cargo run

# Test with specific log level
RUST_LOG=trace cargo test
```

## Extension Lifecycle

1. **Initialize**: Collector calls `otelcol_extension_init()` with JSON config
2. **Start**: Collector calls `otelcol_extension_start()` to begin background tasks
3. **Run**: Extension performs its work (periodic tasks, HTTP requests, etc.)
4. **Stop**: Collector calls `otelcol_extension_stop()` for graceful shutdown
5. **Shutdown**: Collector calls `otelcol_extension_shutdown()` for cleanup

## Build Information

The template automatically embeds build information using `vergen`:

- Version (from Cargo.toml)
- Git commit SHA
- Build timestamp
- Rustc version

Access via FFI functions or check the binary with:

```bash
strings target/release/libotelcol_extension_template.dylib | grep VERGEN
```

## License

Apache-2.0
