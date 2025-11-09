/// Utility Functions for Configuration and Environment Management
///
/// This module provides functions for loading configuration from YAML files
/// and accessing environment variables. Configuration is organized hierarchically
/// with tool-specific sections.

use std::collections::HashMap;
use serde_json::Value;

/// Load configuration from YAML file.
///
/// Currently returns an empty configuration. This function can be extended
/// to load configuration from kmcp.yaml or other YAML files. The configuration
/// structure should match the kmcp.yaml format with a "tools" section containing
/// tool-specific settings.
///
/// # Returns
/// A HashMap containing the loaded configuration, or an empty HashMap if no
/// configuration file is found or if loading fails.
pub fn load_config() -> HashMap<String, Value> {
    // TODO: Implement YAML file loading
    // Example structure:
    // {
    //   "tools": {
    //     "echo": { "prefix": "Echo: " },
    //     "weather": { "api_key_env": "WEATHER_API_KEY" }
    //   }
    // }
    HashMap::new()
}

/// Get tool-specific configuration from the loaded configuration.
///
/// Retrieves configuration settings for a specific tool from the configuration
/// hierarchy. The configuration is expected to have a "tools" section with
/// tool names as keys and their settings as values.
///
/// # Arguments
/// * `tool_name` - Name of the tool to get configuration for (e.g., "echo", "weather")
///
/// # Returns
/// A HashMap containing the tool's configuration settings, or an empty HashMap
/// if the tool has no configuration or doesn't exist.
///
/// # Example
/// If kmcp.yaml contains:
/// ```yaml
/// tools:
///   echo:
///     prefix: "Echo: "
/// ```
/// Then `get_tool_config("echo")` returns `{"prefix": "Echo: "}`
pub fn get_tool_config(tool_name: &str) -> HashMap<String, Value> {
    let config = load_config();
    // Navigate the configuration hierarchy: config -> tools -> tool_name
    if let Some(tools) = config.get("tools").and_then(|v| v.as_object()) {
        if let Some(tool_config) = tools.get(tool_name).and_then(|v| v.as_object()) {
            // Convert the tool's configuration object to a HashMap
            return tool_config.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
        }
    }
    // Return empty HashMap if tool configuration not found
    HashMap::new()
}

/// Get environment variable value with a default fallback.
///
/// Retrieves an environment variable by key, returning the default value if
/// the variable is not set. This is useful for configuration values that
/// may be provided via environment variables (e.g., API keys, service URLs).
///
/// # Arguments
/// * `key` - Environment variable name to look up
/// * `default` - Default value to return if the environment variable is not set
///
/// # Returns
/// The environment variable value if set, otherwise the default value
///
/// # Example
/// ```rust
/// let api_key = get_env_var("WEATHER_API_KEY", "");
/// let port = get_env_var("PORT", "3000");
/// ```
#[allow(dead_code)] // Utility function for tools to use
pub fn get_env_var(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

