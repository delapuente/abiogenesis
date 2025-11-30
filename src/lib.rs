//! Abiogenesis - AI-powered command generation library.
//!
//! This library provides the core functionality for generating and executing
//! commands using AI. It supports:
//!
//! - **Command generation** via the Claude API
//! - **Command caching** with persistent storage
//! - **Sandboxed execution** via Deno runtime
//! - **Permission management** with user consent dialogs
//!
//! # Architecture
//!
//! The library is organized into several modules:
//!
//! - [`config`] - Configuration management (API keys, paths)
//! - [`command_cache`] - Persistent command storage
//! - [`command_router`] - Routes intents to appropriate handlers
//! - [`executor`] - Runs system and generated commands
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
//!     router.process_intent(vec!["hello".to_string()]).await?;
//!     Ok(())
//! }
//! ```

pub mod command_cache;
pub mod command_router;
pub mod config;
pub mod execution_context;
pub mod executor;
pub mod http_client;
pub mod llm_generator;
pub mod permission_ui;
pub mod providers;