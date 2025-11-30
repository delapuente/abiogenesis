//! Execution context tracking for the feedback loop.
//!
//! This module tracks the last executed command and its output, enabling
//! the `--nope` feedback feature for refining generated commands.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Context from the last command execution.
///
/// Stores information needed to regenerate a command with feedback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Name of the last executed command.
    pub command_name: String,
    /// The original script content that was executed.
    pub script_content: String,
    /// Standard error output (if any).
    pub stderr: Option<String>,
    /// Whether the command succeeded.
    pub success: bool,
}

impl ExecutionContext {
    /// Creates a new execution context.
    pub fn new(command_name: &str, script_content: &str, stderr: Option<String>, success: bool) -> Self {
        Self {
            command_name: command_name.to_string(),
            script_content: script_content.to_string(),
            stderr,
            success,
        }
    }

    /// Returns the path to the context file.
    fn context_file_path() -> Result<PathBuf> {
        let config_dir = crate::config::Config::get_config_dir()?;
        Ok(config_dir.join("last_execution.json"))
    }

    /// Saves the execution context to disk.
    pub fn save(&self) -> Result<()> {
        let path = Self::context_file_path()?;
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Loads the last execution context from disk.
    pub fn load() -> Result<Option<Self>> {
        let path = Self::context_file_path()?;
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(path)?;
        let context: Self = serde_json::from_str(&content)?;
        Ok(Some(context))
    }

    /// Clears the saved execution context.
    pub fn clear() -> Result<()> {
        let path = Self::context_file_path()?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_context_serialization() {
        let context = ExecutionContext::new(
            "password",
            "console.log('short');",
            Some("Error: too short".to_string()),
            false,
        );

        let json = serde_json::to_string(&context).unwrap();
        let deserialized: ExecutionContext = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.command_name, "password");
        assert_eq!(deserialized.script_content, "console.log('short');");
        assert_eq!(deserialized.stderr, Some("Error: too short".to_string()));
        assert!(!deserialized.success);
    }

    #[test]
    fn test_execution_context_with_success() {
        let context = ExecutionContext::new(
            "hello",
            "console.log('Hello');",
            None,
            true,
        );

        assert!(context.success);
        assert!(context.stderr.is_none());
    }

    #[test]
    fn test_execution_context_new_sets_all_fields() {
        let context = ExecutionContext::new(
            "test-cmd",
            "const x = 1;",
            Some("warning: unused".to_string()),
            true,
        );

        assert_eq!(context.command_name, "test-cmd");
        assert_eq!(context.script_content, "const x = 1;");
        assert_eq!(context.stderr, Some("warning: unused".to_string()));
        assert!(context.success);
    }

    #[test]
    fn test_execution_context_clone() {
        let context = ExecutionContext::new(
            "original",
            "script content",
            None,
            true,
        );

        let cloned = context.clone();
        assert_eq!(cloned.command_name, context.command_name);
        assert_eq!(cloned.script_content, context.script_content);
        assert_eq!(cloned.stderr, context.stderr);
        assert_eq!(cloned.success, context.success);
    }

    #[test]
    fn test_execution_context_json_roundtrip_with_multiline_script() {
        let script = r#"
            const password = generatePassword();
            console.log(password);
            function generatePassword() {
                return "abc123";
            }
        "#;
        let context = ExecutionContext::new(
            "password",
            script,
            None,
            true,
        );

        let json = serde_json::to_string(&context).unwrap();
        let deserialized: ExecutionContext = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.script_content, script);
    }

    #[test]
    fn test_execution_context_json_roundtrip_with_special_chars_in_stderr() {
        let stderr = "Error: unexpected token '<' at line 1\n\tat parse (file:///tmp/script.ts:1:1)";
        let context = ExecutionContext::new(
            "broken",
            "console.log('<invalid>');",
            Some(stderr.to_string()),
            false,
        );

        let json = serde_json::to_string(&context).unwrap();
        let deserialized: ExecutionContext = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.stderr, Some(stderr.to_string()));
    }

    #[test]
    fn test_execution_context_deserialize_from_json() {
        let json = r#"{
            "command_name": "test",
            "script_content": "console.log('test');",
            "stderr": null,
            "success": true
        }"#;

        let context: ExecutionContext = serde_json::from_str(json).unwrap();
        assert_eq!(context.command_name, "test");
        assert!(context.stderr.is_none());
        assert!(context.success);
    }

    #[test]
    fn test_execution_context_deserialize_with_stderr() {
        let json = r#"{
            "command_name": "failing",
            "script_content": "throw new Error();",
            "stderr": "Error: something went wrong",
            "success": false
        }"#;

        let context: ExecutionContext = serde_json::from_str(json).unwrap();
        assert_eq!(context.command_name, "failing");
        assert_eq!(context.stderr, Some("Error: something went wrong".to_string()));
        assert!(!context.success);
    }
}
