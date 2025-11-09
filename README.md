# Rust MCP Server Template

<img width="1702" height="831" alt="image" src="https://github.com/user-attachments/assets/034efa15-8125-4c9b-a727-90c1fc9f42bb" />


A production-ready Model Context Protocol (MCP) server template built with Rust and Actix Web. This template provides a solid foundation for building high-performance MCP servers with support for both STDIO and HTTP transport modes.

## Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Architecture](#architecture)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Creating Tools](#creating-tools)
- [API Reference](#api-reference)
- [Performance Tuning](#performance-tuning)
- [Deployment](#deployment)
- [Development](#development)
- [MCP Protocol](#mcp-protocol)

## Overview

This template implements a complete MCP server following the Model Context Protocol specification. It supports both STDIO (for MCP Inspector and local development) and HTTP transport modes, making it suitable for both development and production deployments.

The server is optimized for high-traffic scenarios with connection pooling, resource limits, and efficient JSON serialization. It includes built-in monitoring, health checks, and tools discovery endpoints.

## Features

### Core Functionality

- **Full MCP Protocol Implementation**: Complete JSON-RPC 2.0 support with initialize, tools/list, and tools/call methods
- **Dual Transport Modes**: STDIO for MCP Inspector compatibility and HTTP for production deployments
- **Modular Tool System**: Clean separation of tools into individual modules for easy maintenance
- **Configuration Management**: Tool-specific configuration via YAML files
- **Error Handling**: Comprehensive error handling with proper JSON-RPC error responses

### Performance Optimizations

- **Connection Pooling**: Automatic connection management for high concurrency
- **Resource Limits**: Configurable limits for connections, timeouts, and rates
- **Optimized Serialization**: Efficient JSON handling with minimal allocations
- **Buffered I/O**: Optimized stdio mode with 8KB buffers for throughput
- **CPU Optimization**: Automatic worker thread scaling based on CPU cores

### Production Features

- **Health Checks**: Built-in `/health` endpoint for monitoring
- **Metrics Endpoint**: Request counter and server statistics at `/metrics`
- **Tools Discovery**: Server-Sent Events (SSE) endpoint at `/sse` for real-time tool discovery
- **Security Headers**: XSS protection, frame options, and content type validation
- **Graceful Shutdown**: Proper cleanup on server termination

## Architecture

### Project Structure

```
.
├── src/
│   ├── main.rs              # Application entry point and transport mode selection
│   ├── core/
│   │   ├── mod.rs           # Core module exports
│   │   ├── server.rs        # MCP server implementation, HTTP/STDIO handlers
│   │   └── utils.rs         # Configuration loading and utility functions
│   └── tools/
│       ├── mod.rs           # Tool module exports
│       └── echo.rs          # Example echo tool implementation
├── Cargo.toml               # Rust dependencies and build configuration
├── kmcp.yaml                # Tool configuration file
├── Dockerfile               # Multi-stage Docker build for production
└── README.md                # This file
```

### Component Overview

**main.rs**: Entry point that parses environment variables and selects the appropriate transport mode (STDIO or HTTP).

**core/server.rs**: Contains the MCP server implementation including:
- JSON-RPC request/response structures
- Tool registry for managing available tools
- HTTP server setup with Actix Web
- STDIO server implementation for line-based communication
- Request handlers for initialize, tools/list, and tools/call

**core/utils.rs**: Utility functions for:
- Loading configuration from YAML files
- Accessing tool-specific configuration
- Environment variable management

**tools/**: Directory containing tool implementations. Each tool is a separate module that exports a `register` function.

## Quick Start

### Prerequisites

- Rust 1.70 or later (edition 2021)
- Cargo (Rust package manager)

### Installation

1. Clone or use this template as a starting point for your project.

2. Update `Cargo.toml` with your project details:
   ```toml
   [package]
   name = "your-mcp-server"
   version = "0.1.0"
   authors = ["Your Name <your.email@example.com>"]
   ```

3. Build the project:
   ```bash
   cargo build --release
   ```

### Running the Server

#### STDIO Mode (Default)

STDIO mode is used for MCP Inspector and local development. The server reads JSON-RPC requests from stdin and writes responses to stdout.

```bash
# Run in STDIO mode
cargo run

# Or run the release binary
./target/release/mcp-server
```

#### HTTP Mode

HTTP mode is used for production deployments and web integrations.

```bash
# Run in HTTP mode
MCP_TRANSPORT_MODE=http cargo run

# With custom host and port
MCP_TRANSPORT_MODE=http HOST=0.0.0.0 PORT=8080 cargo run
```

### Using with MCP Inspector

1. Build the release binary:
   ```bash
   cargo build --release
   ```

2. In MCP Inspector, configure:
   - **Transport Type**: `STDIO`
   - **Command**: Full path to the binary (e.g., `/path/to/target/release/mcp-server`)
   - **Arguments**: (leave empty)
   - **Environment Variables**: (optional)
     - `SERVER_NAME=mcp-server`
     - `SERVER_VERSION=0.1.0`

3. Click **Connect** to establish the connection.

### Testing the Server

#### Health Check

```bash
curl http://localhost:3000/health
```

Expected response:
```json
{"status":"ok","service":"mcp-server"}
```

#### MCP Initialize

```bash
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'
```

#### List Tools

```bash
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
```

#### Call a Tool

```bash
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc":"2.0",
    "id":3,
    "method":"tools/call",
    "params":{
      "name":"echo",
      "arguments":{"message":"Hello, MCP!"}
    }
  }'
```

## Configuration

### Environment Variables

The server can be configured using the following environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `SERVER_NAME` | Server name for MCP protocol | `mcp-server` |
| `SERVER_VERSION` | Server version string | `0.1.0` |
| `MCP_TRANSPORT_MODE` | Transport mode: `stdio` or `http` | `stdio` |
| `HOST` | Bind address for HTTP mode | `0.0.0.0` |
| `PORT` | Port number for HTTP mode | `3000` |
| `WORKER_THREADS` | Number of worker threads (HTTP mode) | CPU count (max 16) |

### Tool Configuration

Tool-specific configuration is managed in `kmcp.yaml`:

```yaml
name: mcp-server
framework: actix-web-rust
version: 0.1.0
description: Model Context Protocol server built with Rust

tools:
  echo:
    prefix: "Echo: "
  
  weather:
    api_key_env: "WEATHER_API_KEY"
    base_url: "https://api.openweathermap.org/data/2.5"
    timeout: 30
```

Access configuration in your tools:

```rust
use crate::core::utils;

let config = utils::get_tool_config("weather");
let api_key = utils::get_env_var(
    config.get("api_key_env")
        .and_then(|v| v.as_str())
        .unwrap_or("WEATHER_API_KEY"),
    ""
);
```

## Creating Tools

### Tool Structure

Each tool is implemented as a separate Rust module in `src/tools/`. The module must export a `register` function that adds the tool to the registry.

### Example Tool Implementation

```rust
// src/tools/weather.rs
use crate::core::server::{MCPTool, ToolRegistry, ToolHandler};
use crate::core::utils;
use serde_json::Value;

/// Register the weather tool with the tool registry.
/// This function is called during server initialization.
pub fn register(registry: &mut ToolRegistry) {
    // Define the tool metadata
    let tool = MCPTool {
        name: "weather".to_string(),
        description: "Get current weather information for a location.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "City name or location identifier"
                },
                "units": {
                    "type": "string",
                    "enum": ["celsius", "fahrenheit"],
                    "default": "celsius",
                    "description": "Temperature units"
                }
            },
            "required": ["location"]
        }),
    };
    
    // Implement the tool handler
    let handler: ToolHandler = Box::new(|args: Value| -> Result<Value, String> {
        // Extract and validate parameters
        let location = args.get("location")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: location".to_string())?;
        
        let units = args.get("units")
            .and_then(|v| v.as_str())
            .unwrap_or("celsius");
        
        // Load tool configuration
        let config = utils::get_tool_config("weather");
        let api_key = utils::get_env_var(
            config.get("api_key_env")
                .and_then(|v| v.as_str())
                .unwrap_or("WEATHER_API_KEY"),
            ""
        );
        
        if api_key.is_empty() {
            return Err("Weather API key not configured".to_string());
        }
        
        // TODO: Implement actual API call
        // For now, return a placeholder response
        Ok(serde_json::json!({
            "location": location,
            "temperature": 22,
            "units": units,
            "condition": "sunny"
        }))
    });
    
    // Register the tool
    registry.register(tool, handler);
}
```

### Registering Tools

1. Add the tool module to `src/tools/mod.rs`:
   ```rust
   pub mod echo;
   pub mod weather;  // Add your new tool
   ```

2. Register the tool in `src/core/server.rs`:
   ```rust
   pub fn initialize_tools() -> Arc<ToolRegistry> {
       let mut registry = ToolRegistry::new();
       
       tools::echo::register(&mut registry);
       tools::weather::register(&mut registry);  // Register your tool
       
       Arc::new(registry)
   }
   ```

### Tool Handler Best Practices

1. **Parameter Validation**: Always validate required parameters and return clear error messages.
2. **Error Handling**: Use `Result<Value, String>` to return errors. The error string will be sent to the client.
3. **Configuration**: Use `utils::get_tool_config()` to access tool-specific settings.
4. **Environment Variables**: Use `utils::get_env_var()` for sensitive data like API keys.
5. **Performance**: Minimize allocations and use efficient data structures for high-traffic scenarios.

## API Reference

### HTTP Endpoints

#### GET /health

Health check endpoint for monitoring and load balancers.

**Response:**
```json
{
  "status": "ok",
  "service": "mcp-server"
}
```

#### GET /metrics

Server metrics and request statistics.

**Response:**
```json
{
  "requests_total": 1234,
  "status": "ok"
}
```

#### GET /sse

Server-Sent Events endpoint for tools discovery. Returns a stream of tool information.

**Response Format:**
```
data: {"tools":[...],"count":1}

```

**JavaScript Example:**
```javascript
const eventSource = new EventSource('http://localhost:3000/sse');
eventSource.onmessage = (e) => {
    const data = JSON.parse(e.data);
    console.log('Available tools:', data.tools);
};
```

#### POST /mcp

Main MCP JSON-RPC endpoint. Accepts JSON-RPC 2.0 requests.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "echo",
    "arguments": {
      "message": "Hello"
    }
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{\"result\":\"Hello\"}"
      }
    ],
    "isError": false
  }
}
```

## Performance Tuning

### Build Optimizations

The project includes aggressive release optimizations in `Cargo.toml`:

- **opt-level = 3**: Maximum optimization
- **lto = "thin"**: Link-time optimization for better performance
- **codegen-units = 1**: Better inlining opportunities
- **panic = "abort"**: Smaller binary size
- **strip = true**: Remove debug symbols

Build with:
```bash
cargo build --release
```

### HTTP Server Configuration

The HTTP server is configured with the following limits (in `src/core/server.rs`):

- **Max Connections**: 10,000 concurrent connections
- **Connection Rate**: 1,000 connections per second
- **Keep-Alive**: 30 seconds
- **Request Timeout**: 30 seconds
- **Disconnect Timeout**: 2 seconds

### Worker Threads

Worker threads are automatically set based on CPU count (capped at 16). Override with:

```bash
WORKER_THREADS=8 MCP_TRANSPORT_MODE=http cargo run --release
```

### Resource Usage

Typical resource usage:

- **Memory**: 10-50MB base (depends on number of tools)
- **CPU**: Scales with worker threads (1 per CPU core recommended)
- **Network**: Handles 10,000+ concurrent connections efficiently

### Monitoring

Monitor server performance using the metrics endpoint:

```bash
# Watch metrics in real-time
watch -n 1 'curl -s http://localhost:3000/metrics | jq'
```

## Deployment

### Docker

#### Build Image

```bash
docker build -t mcp-server:latest .
```

#### Run Container

```bash
# HTTP mode (default)
docker run -p 3000:3000 mcp-server:latest

# Stdio mode
docker run -i mcp-server:latest

# With custom configuration
docker run -p 3000:3000 \
  -e WORKER_THREADS=8 \
  -e PORT=8080 \
  -e SERVER_NAME=my-mcp-server \
  mcp-server:latest
```

#### Docker Compose

```yaml
version: '3.8'
services:
  mcp-server:
    build: .
    ports:
      - "3000:3000"
    environment:
      - MCP_TRANSPORT_MODE=http
      - WORKER_THREADS=8
      - PORT=3000
      - SERVER_NAME=mcp-server
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 5s
    restart: unless-stopped
```

### Systemd Service

Create `/etc/systemd/system/mcp-server.service`:

```ini
[Unit]
Description=MCP Server
After=network.target

[Service]
Type=simple
User=mcpuser
WorkingDirectory=/opt/mcp-server
ExecStart=/opt/mcp-server/mcp-server
Environment="MCP_TRANSPORT_MODE=http"
Environment="PORT=3000"
Environment="WORKER_THREADS=8"
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable mcp-server
sudo systemctl start mcp-server
```

### Kubernetes

Example deployment:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: mcp-server
spec:
  replicas: 3
  selector:
    matchLabels:
      app: mcp-server
  template:
    metadata:
      labels:
        app: mcp-server
    spec:
      containers:
      - name: mcp-server
        image: mcp-server:latest
        ports:
        - containerPort: 3000
        env:
        - name: MCP_TRANSPORT_MODE
          value: "http"
        - name: PORT
          value: "3000"
        - name: WORKER_THREADS
          value: "8"
        livenessProbe:
          httpGet:
            path: /health
            port: 3000
          initialDelaySeconds: 10
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /health
            port: 3000
          initialDelaySeconds: 5
          periodSeconds: 10
```

## Development

### Adding Dependencies

Add dependencies to `Cargo.toml`:

```toml
[dependencies]
your-crate = "1.0"
```

Then build:
```bash
cargo build
```

### Code Quality

Format code:
```bash
cargo fmt
```

Lint code:
```bash
cargo clippy
```

Run with stricter lints:
```bash
cargo clippy -- -W clippy::all -W clippy::pedantic
```

### Testing

Add tests in your tool modules:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tool_handler() {
        let args = serde_json::json!({"message": "test"});
        // Test your handler
    }
}
```

Run tests:
```bash
cargo test
```

### Debugging

Run in debug mode with logging:
```bash
RUST_LOG=debug cargo run
```

## MCP Protocol

### Protocol Version

This server implements MCP protocol version `2024-11-05`.

### Supported Methods

#### initialize

Initializes the MCP connection. Returns server capabilities and information.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {}
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2024-11-05",
    "capabilities": {
      "tools": {}
    },
    "serverInfo": {
      "name": "mcp-server",
      "version": "0.1.0"
    }
  }
}
```

#### tools/list

Lists all available tools.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/list",
  "params": {}
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "tools": [
      {
        "name": "echo",
        "description": "Echo a message back to the client.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "message": {
              "type": "string",
              "description": "The message to echo"
            }
          },
          "required": ["message"]
        }
      }
    ]
  }
}
```

#### tools/call

Calls a tool with the provided arguments.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "echo",
    "arguments": {
      "message": "Hello, MCP!"
    }
  }
}
```

**Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{\"result\":\"Hello, MCP!\"}"
      }
    ],
    "isError": false
  }
}
```

**Response (Error):**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Error: Missing required parameter: message"
      }
    ],
    "isError": true
  }
}
```

### Error Codes

The server uses standard JSON-RPC 2.0 error codes:

- `-32700`: Parse error (invalid JSON)
- `-32600`: Invalid Request (malformed JSON-RPC)
- `-32601`: Method not found
- `-32602`: Invalid params
- `-32603`: Internal error

## License

MIT License - see LICENSE file for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## Support

For issues and questions, please open an issue on the GitHub repository.
