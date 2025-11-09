/// MCP Server Entry Point
///
/// This is the main entry point for the MCP server. It parses environment variables
/// to determine the transport mode (STDIO or HTTP) and server configuration, then
/// starts the appropriate server implementation.
///
/// Environment Variables:
/// - SERVER_NAME: Name of the server (default: "mcp-server")
/// - SERVER_VERSION: Version string (default: "0.1.0")
/// - MCP_TRANSPORT_MODE: "stdio", "http", or "both" (default: "both")
/// - HOST: Bind address for HTTP mode (default: "0.0.0.0")
/// - PORT: Port number for HTTP mode (default: 3000)

mod core;
mod tools;

use std::env;
use crate::core::server;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Load server metadata from environment variables with defaults
    let name = env::var("SERVER_NAME").unwrap_or_else(|_| "mcp-server".to_string());
    let version = env::var("SERVER_VERSION").unwrap_or_else(|_| "0.1.0".to_string());
    
    // Determine transport mode from environment variable
    // Default to "both" to support both STDIO (MCP Inspector) and HTTP simultaneously
    let transport = env::var("MCP_TRANSPORT_MODE")
        .unwrap_or_else(|_| "both".to_string());
    
    match transport.as_str() {
        "stdio" => {
            // STDIO mode only: Read from stdin, write to stdout
            // Used for MCP Inspector and local development
            server::run_server_stdio(name, version).await
        }
        "http" => {
            // HTTP mode only: Run as HTTP server with Actix Web
            // Used for production deployments and web integrations
            let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
            let port = env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse::<u16>()
                .unwrap_or(3000);
            server::run_server_http(name, version, host, port).await
        }
        "both" => {
            // Both modes: Run STDIO and HTTP concurrently
            // This allows MCP Inspector to use STDIO while HTTP endpoints are available
            let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
            let port = env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse::<u16>()
                .unwrap_or(3000);
            
            let name_clone = name.clone();
            let version_clone = version.clone();
            
            // Spawn STDIO server in a background task
            let stdio_handle = tokio::spawn(async move {
                if let Err(e) = server::run_server_stdio(name_clone, version_clone).await {
                    eprintln!("STDIO server error: {}", e);
                }
            });
            
            // Run HTTP server in the foreground
            let http_result = server::run_server_http(name, version, host, port).await;
            
            // If HTTP server exits, abort STDIO task
            stdio_handle.abort();
            
            http_result
        }
        _ => {
            eprintln!("Error: Invalid transport mode '{}'. Must be 'stdio', 'http', or 'both'", transport);
            std::process::exit(1);
        }
    }
}
