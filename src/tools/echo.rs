/// Echo Tool Implementation
///
/// This is an example tool that demonstrates the basic structure for implementing
/// MCP tools. The echo tool takes a message parameter and returns it, optionally
/// with a configurable prefix from the tool configuration.

use crate::core::server::{MCPTool, ToolRegistry, ToolHandler};
use crate::core::utils;
use serde_json::Value;

/// Register the echo tool with the tool registry.
///
/// This function is called during server initialization to add the echo tool
/// to the available tools list. It defines the tool's metadata (name, description,
/// input schema) and implements the tool's handler function.
///
/// # Arguments
/// * `registry` - Mutable reference to the tool registry where the tool will be registered
pub fn register(registry: &mut ToolRegistry) {
    let tool = MCPTool {
        name: "echo".to_string(),
        description: "Echo a message back to the client.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to echo"
                }
            },
            "required": ["message"]
        }),
    };
    
    // Define the tool handler function
    // The handler receives JSON arguments and returns either a JSON result or an error string
    let handler: ToolHandler = Box::new(|args: Value| -> Result<Value, String> {
        // Extract and validate the required "message" parameter
        // Returns an error if the parameter is missing or not a string
        let message = args.get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: message".to_string())?;
        
        // Load tool-specific configuration from kmcp.yaml
        // The echo tool supports an optional "prefix" configuration value
        let config = utils::get_tool_config("echo");
        let prefix = config.get("prefix")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        
        // Build the result string with optional prefix
        // Pre-allocate string capacity when prefix is present to avoid reallocations
        let result = if prefix.is_empty() {
            // No prefix configured - return message as-is
            message.to_string()
        } else {
            // Prefix configured - concatenate prefix and message
            // Pre-allocate with known capacity for efficiency
            let mut result = String::with_capacity(prefix.len() + message.len());
            result.push_str(prefix);
            result.push_str(message);
            result
        };
        
        // Return result as JSON object
        Ok(serde_json::json!({ "result": result }))
    });
    
    registry.register(tool, handler);
}

