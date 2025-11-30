//! Abiogenesis - AI-powered command generation library.
//!
//! This library provides the core functionality for generating and executing
//! commands using AI. It supports:
//!
//! - **Command generation** via the Claude API
//! - **Command caching** with persistent storage
//! - **Sandboxed execution** via Deno runtime
//! - **Permission management** with user consent dialogs
//! - **Corrective feedback** to improve commands iteratively
//!
//! # Architecture
//!
//! The library is organized into several modules:
//!
//! - [`config`] - Configuration management (API keys, paths)
//! - [`command_cache`] - Persistent command storage
//! - [`command_router`] - Routes intents to appropriate handlers
//! - [`executor`] - Runs system and generated commands
//! - [`execution_context`] - Tracks last execution for corrective feedback
//! - [`llm_generator`] - AI-powered command generation
//! - [`permission_ui`] - User consent dialogs
//! - [`providers`] - Shared dependency injection traits
//! - [`http_client`] - HTTP client abstraction
//!
//! # Example
//!
//! ```ignore
//! use abiogenesis::command_router::CommandRouter;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let mut router = CommandRouter::new(false).await?;
//!
//!     // Generate and execute a command
//!     router.process_intent(vec!["hello".to_string()]).await?;
//!
//!     // If the command didn't work as expected, provide corrective feedback
//!     // to regenerate it with improvements
//!     router.process_corrective_feedback("make the greeting more formal").await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Corrective Feedback
//!
//! When a generated command doesn't meet expectations, use the `--nope` flag
//! to regenerate it with feedback:
//!
//! ```bash
//! # Generate a password command
//! ergo password
//!
//! # Output is too short? Provide feedback to improve it
//! ergo --nope "make it at least 20 characters with symbols"
//!
//! # Or just use stderr from the last execution as context
//! ergo --nope
//! ```
//!
//! The corrective feedback loop preserves the command name while regenerating
//! the implementation based on your feedback and any error output from the
//! previous execution.

pub mod command_cache;
pub mod command_router;
pub mod config;
pub mod execution_context;
pub mod executor;
pub mod http_client;
pub mod llm_generator;
pub mod permission_ui;
pub mod providers;