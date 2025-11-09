/// MCP Server Implementation
///
/// This module contains the core MCP server implementation including:
/// - JSON-RPC 2.0 request/response structures
/// - Tool registry for managing available tools
/// - HTTP server setup with Actix Web
/// - STDIO server implementation for line-based communication
/// - Request handlers for MCP protocol methods

use actix_web::{
    web, App, HttpServer, HttpResponse, Result,
    middleware::{Compress, Logger, DefaultHeaders},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::tools;

/// Application state shared across all worker threads in HTTP mode.
///
/// This state is cloned for each worker thread and contains server metadata
/// that is used in MCP protocol responses.
#[derive(Clone)]
pub struct AppState {
    /// Server name as reported in MCP initialize responses
    pub server_name: String,
    /// Server version string as reported in MCP initialize responses
    pub server_version: String,
}

/// JSON-RPC 2.0 request structure for MCP protocol.
///
/// All MCP requests follow the JSON-RPC 2.0 specification. The jsonrpc field
/// must be "2.0", id is optional (None for notifications), method specifies
/// the MCP method to call, and params contains method-specific parameters.
#[derive(Deserialize, Debug)]
pub struct MCPRequest {
    /// JSON-RPC version identifier, must be "2.0"
    #[allow(dead_code)]
    jsonrpc: String,
    /// Request ID for correlating responses. None indicates a notification.
    id: Option<serde_json::Value>,
    /// MCP method name (e.g., "initialize", "tools/list", "tools/call")
    method: String,
    /// Method-specific parameters as JSON value
    params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response structure for MCP protocol.
///
/// Responses must include jsonrpc "2.0", the request id, and either a result
/// or an error. The error field is only present when an error occurred.
#[derive(Serialize, Debug)]
pub struct MCPResponse {
    /// JSON-RPC version identifier, always "2.0"
    jsonrpc: String,
    /// Request ID from the original request
    id: Option<serde_json::Value>,
    /// Response result, present when request succeeded
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    /// Error information, present when request failed
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<MCPError>,
}

/// JSON-RPC 2.0 error structure.
///
/// Errors follow the JSON-RPC 2.0 error format with a numeric code, message,
/// and optional additional data.
#[derive(Serialize, Debug)]
pub struct MCPError {
    /// JSON-RPC error code (e.g., -32601 for method not found)
    code: i32,
    /// Human-readable error message
    message: String,
    /// Optional additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

/// MCP tool definition structure.
///
/// Each tool must have a unique name, description, and JSON schema defining
/// its input parameters. This structure is serialized when listing tools.
#[derive(Serialize, Debug, Clone)]
pub struct MCPTool {
    /// Unique tool identifier (e.g., "echo", "weather")
    pub name: String,
    /// Human-readable description of what the tool does
    pub description: String,
    /// JSON Schema defining the tool's input parameters
    pub input_schema: serde_json::Value,
}

/// Tool handler function type definition.
///
/// Tool handlers are boxed closures that take JSON arguments and
/// return either a JSON result or an error string. The handler must be
/// Send + Sync to work across threads in the HTTP server.
pub type ToolHandler = Box<dyn Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync>;

/// Registry of available MCP tools.
///
/// The registry maintains a list of tool definitions for discovery and a
/// HashMap of tool names to their handler functions for execution.
pub struct ToolRegistry {
    /// List of all registered tools (for tools/list method)
    pub tools: Vec<MCPTool>,
    /// Map of tool names to their handler functions (for tools/call method)
    pub handlers: HashMap<String, ToolHandler>,
}

impl ToolRegistry {
    /// Create a new empty tool registry.
    ///
    /// Tools are registered using the register method during server initialization.
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            handlers: HashMap::new(),
        }
    }

    /// Register a tool with the registry.
    ///
    /// This method adds the tool definition to the tools list and stores
    /// the handler function in the handlers map for later execution.
    ///
    /// # Arguments
    /// * `tool` - Tool definition with name, description, and input schema
    /// * `handler` - Function that executes the tool when called
    pub fn register(&mut self, tool: MCPTool, handler: ToolHandler) {
        let name = tool.name.clone();
        self.tools.push(tool);
        self.handlers.insert(name, handler);
    }
}

/// Health check endpoint handler.
///
/// Returns a simple JSON response indicating the server is running.
/// Used by load balancers and monitoring systems to verify server availability.
async fn health() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "mcp-server"
    })))
}

/// MCP JSON-RPC request handler with metrics tracking.
///
/// This is the main handler for all MCP protocol requests in HTTP mode.
/// It increments a request counter for monitoring, routes requests to the
/// appropriate method handler, and returns JSON-RPC 2.0 compliant responses.
///
/// # Arguments
/// * `state` - Application state containing server metadata
/// * `registry` - Tool registry for accessing available tools
/// * `counter` - Atomic counter for tracking total requests
/// * `req` - JSON-RPC request from the client
async fn mcp_handler_optimized(
    state: web::Data<AppState>,
    registry: web::Data<Arc<ToolRegistry>>,
    counter: web::Data<std::sync::atomic::AtomicU64>,
    req: web::Json<MCPRequest>,
) -> Result<HttpResponse> {
    // Increment request counter using relaxed ordering for performance.
    // Relaxed ordering is sufficient here since we only need atomicity,
    // not synchronization with other operations.
    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    
    // Route request to appropriate method handler based on method name
    let response = match req.method.as_str() {
        "initialize" => handle_initialize(state, req.id.clone()),
        "tools/list" => handle_tools_list(registry, req.id.clone()),
        "tools/call" => handle_tools_call(registry, req.id.clone(), req.params.clone()).await,
        _ => {
            // Method not found - return JSON-RPC error
            MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id.clone(),
                result: None,
                error: Some(MCPError {
                    code: -32601, // Method not found
                    message: format!("Method not found: {}", req.method),
                    data: None,
                }),
            }
        }
    };
    
    Ok(HttpResponse::Ok().json(response))
}

/// Metrics endpoint handler for monitoring.
///
/// Returns the total number of requests processed since server start.
/// This endpoint can be used by monitoring systems to track server load.
///
/// # Arguments
/// * `counter` - Atomic counter tracking total requests
async fn metrics_handler(
    counter: web::Data<std::sync::atomic::AtomicU64>,
) -> Result<HttpResponse> {
    let count = counter.load(std::sync::atomic::Ordering::Relaxed);
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "requests_total": count,
        "status": "ok"
    })))
}

/// Server-Sent Events endpoint for tools discovery.
///
/// Returns a stream of tool information in SSE format. Clients can subscribe
/// to this endpoint to receive real-time updates about available tools.
/// The response includes all registered tools with their names, descriptions,
/// and input schemas.
///
/// # Arguments
/// * `registry` - Tool registry containing all registered tools
async fn sse_tools_discovery(
    registry: web::Data<Arc<ToolRegistry>>,
) -> Result<HttpResponse> {
    use actix_web::http::header;
    
    // Serialize all tools to JSON format matching MCP tools/list response
    let tools_json: Vec<serde_json::Value> = registry.tools.iter()
        .map(|tool| serde_json::json!({
            "name": tool.name,
            "description": tool.description,
            "inputSchema": tool.input_schema
        }))
        .collect();
    
    // Create SSE event data with tools list and count
    let tools_data = serde_json::json!({
        "tools": tools_json,
        "count": tools_json.len()
    });
    
    // Format as SSE event: "data: {json}\n\n"
    let sse_data = format!(
        "data: {}\n\n",
        serde_json::to_string(&tools_data).unwrap_or_else(|_| "{}".to_string())
    );
    
    // Return SSE response with appropriate headers
    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        // Disable caching to ensure clients always get fresh data
        .insert_header(header::CacheControl(vec![
            header::CacheDirective::NoCache,
            header::CacheDirective::NoStore,
            header::CacheDirective::MustRevalidate,
        ]))
        // Disable nginx buffering for real-time streaming
        .insert_header(("x-accel-buffering", "no"))
        .body(sse_data))
}

/// Handle MCP initialize method.
///
/// The initialize method is the first method called by MCP clients to establish
/// a connection. It returns the protocol version, server capabilities, and
/// server information.
///
/// # Arguments
/// * `state` - Application state containing server name and version
/// * `id` - Request ID from the client
fn handle_initialize(state: web::Data<AppState>, id: Option<serde_json::Value>) -> MCPResponse {
    MCPResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": state.server_name,
                "version": state.server_version
            }
        })),
        error: None,
    }
}

/// Handle MCP tools/list method.
///
/// Returns a list of all available tools with their names, descriptions,
/// and input schemas. This allows clients to discover what tools are available
/// before calling them.
///
/// # Arguments
/// * `registry` - Tool registry containing all registered tools
/// * `id` - Request ID from the client
fn handle_tools_list(registry: web::Data<Arc<ToolRegistry>>, id: Option<serde_json::Value>) -> MCPResponse {
    MCPResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(serde_json::json!({
            "tools": registry.tools
        })),
        error: None,
    }
}

/// Handle MCP tools/call method.
///
/// Executes a tool with the provided arguments. The tool name and arguments
/// are extracted from the params, the tool handler is looked up in the
/// registry, and the handler is executed. Results or errors are formatted
/// according to MCP protocol specifications.
///
/// # Arguments
/// * `registry` - Tool registry for looking up tool handlers
/// * `id` - Request ID from the client
/// * `params` - Method parameters containing tool name and arguments
async fn handle_tools_call(
    registry: web::Data<Arc<ToolRegistry>>,
    id: Option<serde_json::Value>,
    params: Option<serde_json::Value>,
) -> MCPResponse {
    // Extract tool call parameters from the request
    let tool_params: serde_json::Value = match params {
        Some(p) => p,
        None => {
            // Missing params - return invalid params error
            return MCPResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(MCPError {
                    code: -32602, // Invalid params
                    message: "Invalid params".to_string(),
                    data: None,
                }),
            };
        }
    };
    
    // Extract tool name from parameters
    let tool_name = tool_params.get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    
    // Extract tool arguments, defaulting to empty object if not provided
    let arguments = tool_params.get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));
    
    // Look up tool handler in registry
    if let Some(handler) = registry.handlers.get(tool_name) {
        // Execute tool handler with provided arguments
        match handler(arguments) {
            Ok(result) => {
                // Tool executed successfully - format as MCP content response
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "content": [
                            {
                                "type": "text",
                                "text": serde_json::to_string(&result).unwrap_or_default()
                            }
                        ],
                        "isError": false
                    })),
                    error: None,
                }
            }
            Err(e) => {
                // Tool execution failed - format as MCP error response
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "content": [
                            {
                                "type": "text",
                                "text": format!("Error: {}", e)
                            }
                        ],
                        "isError": true
                    })),
                    error: None,
                }
            }
        }
    } else {
        // Tool not found in registry
        MCPResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(MCPError {
                code: -32601, // Method not found
                message: format!("Unknown tool: {}", tool_name),
                data: None,
            }),
        }
    }
}

/// Initialize and register all tools.
///
/// This function is called during server startup to create the tool registry
/// and register all available tools. Add new tool registrations here when
/// implementing additional tools.
///
/// # Returns
/// An Arc-wrapped ToolRegistry containing all registered tools and handlers
pub fn initialize_tools() -> Arc<ToolRegistry> {
    let mut registry = ToolRegistry::new();
    
    // Register all available tools
    // Add new tool registrations here following this pattern:
    // tools::your_tool::register(&mut registry);
    tools::echo::register(&mut registry);
    
    Arc::new(registry)
}

/// Run the MCP server in HTTP mode.
///
/// Configures and starts an Actix Web HTTP server with optimized settings
/// for high-traffic production deployments. The server handles MCP protocol
/// requests over HTTP/JSON-RPC 2.0.
///
/// # Arguments
/// * `name` - Server name for MCP protocol responses
/// * `version` - Server version string
/// * `host` - Bind address (e.g., "0.0.0.0" for all interfaces)
/// * `port` - Port number to listen on
///
/// # Configuration
/// The server is configured with:
/// - Worker threads: Auto-detected from CPU count (max 16)
/// - Max connections: 10,000 concurrent connections
/// - Connection rate limit: 1,000 connections per second
/// - Keep-alive: 30 seconds
/// - Request timeout: 30 seconds
/// - Disconnect timeout: 2 seconds
/// - Shutdown timeout: 10 seconds
pub async fn run_server_http(name: String, version: String, host: String, port: u16) -> std::io::Result<()> {
    use std::time::Duration;
    use std::sync::atomic::AtomicU64;
    
    let bind_addr = format!("{}:{}", host, port);
    
    // Create application state shared across all worker threads
    let app_state = web::Data::new(AppState {
        server_name: name.clone(),
        server_version: version.clone(),
    });
    
    // Initialize tool registry and wrap in Arc for sharing across threads
    let tool_registry = web::Data::new(initialize_tools());
    
    // Create atomic request counter for metrics endpoint
    // Using AtomicU64 for lock-free counting across worker threads
    let request_count = web::Data::new(AtomicU64::new(0));
    let request_count_clone = request_count.clone();
    
    // Determine optimal worker thread count
    // Defaults to CPU count but capped at 16 to avoid excessive context switching
    // Can be overridden via WORKER_THREADS environment variable
    let workers = std::env::var("WORKER_THREADS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or_else(|| {
            (num_cpus::get()).min(16).max(1)
        });
    
    // Log server startup information to stderr (not stdout to avoid interfering with JSON-RPC)
    eprintln!("MCP Server Starting (HTTP mode)");
    eprintln!("  Name: {}", name);
    eprintln!("  Version: {}", version);
    eprintln!("  Bind Address: {}", bind_addr);
    eprintln!("  Worker Threads: {}", workers);
    eprintln!("  Max Connections: 10000");
    eprintln!("  MCP Protocol: JSON-RPC 2.0");
    
    // Create and configure HTTP server
    HttpServer::new(move || {
        App::new()
            // Share application state with all routes
            .app_data(app_state.clone())
            .app_data(tool_registry.clone())
            .app_data(request_count_clone.clone())
            // Enable compression for JSON responses (gzip/brotli)
            .wrap(Compress::default())
            // Add security headers to all responses
            .wrap(
                DefaultHeaders::new()
                    .add(("X-Content-Type-Options", "nosniff"))
                    .add(("X-Frame-Options", "DENY"))
                    .add(("X-XSS-Protection", "1; mode=block"))
            )
            // Configure request logging
            // Format: %r = request line, %s = status, %Dms = duration in milliseconds
            .wrap(Logger::new("%r %s %Dms"))
            // Register route handlers
            .route("/health", web::get().to(health))
            .route("/metrics", web::get().to(metrics_handler))
            .route("/sse", web::get().to(sse_tools_discovery))
            .route("/mcp", web::post().to(mcp_handler_optimized))
            .route("/", web::post().to(mcp_handler_optimized))
            .route("/", web::get().to(health))
    })
    .workers(workers)
    // Connection limits for high-traffic scenarios
    .max_connections(10000)
    .max_connection_rate(1000)
    // Timeout configurations to prevent resource exhaustion
    .keep_alive(Duration::from_secs(30))
    .client_request_timeout(Duration::from_secs(30))
    .client_disconnect_timeout(Duration::from_secs(2))
    // Graceful shutdown timeout
    .shutdown_timeout(10)
    .bind(&bind_addr)?
    .run()
    .await
}

/// Run the MCP server in STDIO mode.
///
/// Implements MCP protocol over standard input/output for compatibility with
/// MCP Inspector and local development. The server reads JSON-RPC requests
/// line-by-line from stdin and writes responses to stdout. All logging goes
/// to stderr to avoid interfering with the JSON-RPC protocol stream.
///
/// # Arguments
/// * `name` - Server name for MCP protocol responses
/// * `version` - Server version string
///
/// # Implementation Details
/// - Uses buffered I/O with 8KB buffers for optimal throughput
/// - Processes requests synchronously (one at a time)
/// - Skips notifications (requests without IDs)
/// - Flushes after each response for low latency
pub async fn run_server_stdio(name: String, version: String) -> std::io::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
    
    // Log startup information to stderr (not stdout to avoid interfering with JSON-RPC)
    eprintln!("MCP Server Starting (STDIO mode)");
    eprintln!("  Name: {}", name);
    eprintln!("  Version: {}", version);
    eprintln!("  MCP Protocol: JSON-RPC 2.0");
    
    // Initialize tool registry and application state
    let tool_registry = Arc::new(initialize_tools());
    let app_state = AppState {
        server_name: name,
        server_version: version,
    };
    
    // Set up buffered I/O for optimal performance
    // 8KB buffer size balances memory usage with I/O efficiency
    let stdin = tokio::io::stdin();
    let mut stdin = BufReader::with_capacity(8192, stdin).lines();
    let stdout = tokio::io::stdout();
    let mut stdout = BufWriter::with_capacity(8192, stdout);
    
    // Main request processing loop
    // Reads one line at a time from stdin, processes JSON-RPC requests
    while let Some(line) = stdin.next_line().await? {
        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }
        
        // Parse JSON-RPC request from input line
        let request: Result<MCPRequest, _> = serde_json::from_str(&line);
        match request {
            Ok(req) => {
                // Skip notifications (requests without ID)
                // Notifications are one-way messages that don't require responses
                if req.id.is_none() {
                    // Handle specific notifications if needed
                    if req.method == "notifications/initialized" {
                        // Client has finished initialization - acknowledge silently
                        continue;
                    }
                    continue;
                }
                
                // Process request and generate response based on method
                let response = match req.method.as_str() {
                    "initialize" => handle_initialize_stdio(&app_state, req.id.clone()),
                    "tools/list" => handle_tools_list_stdio(&tool_registry, req.id.clone()),
                    "tools/call" => {
                        handle_tools_call_stdio(&tool_registry, req.id.clone(), req.params.clone())
                    }
                    _ => {
                        // Unknown method - return method not found error
                        MCPResponse {
                            jsonrpc: "2.0".to_string(),
                            id: req.id.clone(),
                            result: None,
                            error: Some(MCPError {
                                code: -32601, // Method not found
                                message: format!("Method not found: {}", req.method),
                                data: None,
                            }),
                        }
                    }
                };
                
                // Serialize response to JSON string
                let response_json = match serde_json::to_string(&response) {
                    Ok(json) => json,
                    Err(e) => {
                        // Serialization error - log and skip this response
                        eprintln!("Error serializing response: {}", e);
                        continue;
                    }
                };
                
                // Write response to stdout (buffered)
                // Each response must be on a single line followed by newline
                if let Err(e) = stdout.write_all(response_json.as_bytes()).await {
                    eprintln!("Error writing to stdout: {}", e);
                    break;
                }
                if let Err(e) = stdout.write_all(b"\n").await {
                    eprintln!("Error writing newline: {}", e);
                    break;
                }
                // Flush after each response for low latency
                // This ensures responses are sent immediately rather than waiting for buffer fill
                if let Err(e) = stdout.flush().await {
                    eprintln!("Error flushing stdout: {}", e);
                    break;
                }
            }
            Err(e) => {
                // Invalid JSON-RPC request - attempt to extract ID for error response
                eprintln!("Parse error: {}", e);
                // Try to parse as generic JSON to extract ID if present
                if let Ok(partial) = serde_json::from_str::<serde_json::Value>(&line) {
                    if let Some(id) = partial.get("id") {
                        // Send parse error response if we can extract the ID
                        let error_response = MCPResponse {
                            jsonrpc: "2.0".to_string(),
                            id: Some(id.clone()),
                            result: None,
                            error: Some(MCPError {
                                code: -32700, // Parse error
                                message: format!("Parse error: {}", e),
                                data: None,
                            }),
                        };
                        if let Ok(response_json) = serde_json::to_string(&error_response) {
                            let _ = stdout.write_all(response_json.as_bytes()).await;
                            let _ = stdout.write_all(b"\n").await;
                            let _ = stdout.flush().await;
                        }
                    }
                }
            }
        }
    }
    
    Ok(())
}

/// Handle MCP initialize method in STDIO mode.
///
/// Same functionality as HTTP mode but takes a reference to AppState instead
/// of web::Data wrapper.
///
/// # Arguments
/// * `state` - Application state containing server name and version
/// * `id` - Request ID from the client
fn handle_initialize_stdio(state: &AppState, id: Option<serde_json::Value>) -> MCPResponse {
    MCPResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": state.server_name,
                "version": state.server_version
            }
        })),
        error: None,
    }
}

/// Handle MCP tools/list method in STDIO mode.
///
/// Serializes tools with proper field names (inputSchema in camelCase) to match
/// MCP protocol specification. Same functionality as HTTP mode but takes a
/// reference to Arc<ToolRegistry> instead of web::Data wrapper.
///
/// # Arguments
/// * `registry` - Tool registry containing all registered tools
/// * `id` - Request ID from the client
fn handle_tools_list_stdio(registry: &Arc<ToolRegistry>, id: Option<serde_json::Value>) -> MCPResponse {
    // Serialize tools with proper MCP protocol field names
    // inputSchema must be in camelCase per MCP specification
    let tools_json: Vec<serde_json::Value> = registry.tools.iter()
        .map(|tool| serde_json::json!({
            "name": tool.name,
            "description": tool.description,
            "inputSchema": tool.input_schema
        }))
        .collect();
    
    MCPResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(serde_json::json!({
            "tools": tools_json
        })),
        error: None,
    }
}

/// Handle MCP tools/call method in STDIO mode.
///
/// Executes a tool synchronously (STDIO mode processes one request at a time).
/// Same functionality as HTTP mode but takes a reference to Arc<ToolRegistry>
/// instead of web::Data wrapper and is synchronous.
///
/// # Arguments
/// * `registry` - Tool registry for looking up tool handlers
/// * `id` - Request ID from the client
/// * `params` - Method parameters containing tool name and arguments
fn handle_tools_call_stdio(
    registry: &Arc<ToolRegistry>,
    id: Option<serde_json::Value>,
    params: Option<serde_json::Value>,
) -> MCPResponse {
    // Extract tool call parameters from the request
    let tool_params: serde_json::Value = match params {
        Some(p) => p,
        None => {
            // Missing params - return invalid params error
            return MCPResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(MCPError {
                    code: -32602, // Invalid params
                    message: "Invalid params".to_string(),
                    data: None,
                }),
            };
        }
    };
    
    // Extract tool name from parameters
    let tool_name = tool_params.get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    
    // Extract tool arguments, defaulting to empty object if not provided
    let arguments = tool_params.get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));
    
    // Look up tool handler in registry
    if let Some(handler) = registry.handlers.get(tool_name) {
        // Execute tool handler with provided arguments
        match handler(arguments) {
            Ok(result) => {
                // Tool executed successfully - format as MCP content response
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "content": [
                            {
                                "type": "text",
                                "text": serde_json::to_string(&result).unwrap_or_default()
                            }
                        ],
                        "isError": false
                    })),
                    error: None,
                }
            }
            Err(e) => {
                // Tool execution failed - format as MCP error response
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(serde_json::json!({
                        "content": [
                            {
                                "type": "text",
                                "text": format!("Error: {}", e)
                            }
                        ],
                        "isError": true
                    })),
                    error: None,
                }
            }
        }
    } else {
        // Tool not found in registry
        MCPResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(MCPError {
                code: -32601, // Method not found
                message: format!("Unknown tool: {}", tool_name),
                data: None,
            }),
        }
    }
}
