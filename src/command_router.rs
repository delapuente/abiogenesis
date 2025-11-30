//! Command routing and orchestration module.
//!
//! This module is the central coordinator for the ergo system. It routes user
//! intents to the appropriate handler based on:
//!
//! 1. **System commands** - If the command exists in PATH, execute directly
//! 2. **Cached commands** - If previously generated, retrieve and execute
//! 3. **AI generation** - Generate a new command using the LLM
//!
//! # Flow
//!
//! ```text
//! User Intent
//!     │
//!     ├── System Command? ──→ Execute via OS
//!     │
//!     ├── Cached Command? ──→ Check Permissions ──→ Execute via Deno
//!     │
//!     └── Unknown? ──→ Generate via LLM ──→ Cache ──→ Check Permissions ──→ Execute
//! ```
//!
//! # Conversational Mode
//!
//! When the user provides a single argument containing spaces (e.g., "show me
//! the current date"), it's treated as a natural language description. The
//! router will generate a command based on this description and suggest a name.

use crate::{
    command_cache::{CommandCache, PermissionConsent},
    executor::Executor,
    llm_generator::{CommandGenerator, LlmGenerator},
    permission_ui::PermissionUI,
};
use anyhow::Result;
use tracing::{info, warn};
use which::which;

/// Routes user intents to appropriate command handlers.
///
/// The router is the main orchestrator that coordinates between:
/// - Command cache for persistent storage
/// - LLM generator for creating new commands
/// - Executor for running commands
/// - Permission UI for user consent
///
/// # Example
///
/// ```ignore
/// let mut router = CommandRouter::new(false).await?;
///
/// // Execute a system command
/// router.process_intent(vec!["ls".to_string(), "-la".to_string()]).await?;
///
/// // Generate and execute a new command
/// router.process_intent(vec!["hello".to_string()]).await?;
///
/// // Conversational mode
/// router.process_intent(vec!["show me today's date".to_string()]).await?;
/// ```
pub struct CommandRouter {
    cache: CommandCache,
    generator: LlmGenerator,
    executor: Executor,
    permission_ui: PermissionUI,
    verbose: bool,
}

impl CommandRouter {
    /// Creates a new command router.
    ///
    /// Initializes all subsystems including the command cache, LLM generator,
    /// executor, and permission UI.
    ///
    /// # Arguments
    ///
    /// * `verbose` - If true, enables verbose output during command processing
    ///
    /// # Errors
    ///
    /// Returns an error if the command cache cannot be initialized.
    pub async fn new(verbose: bool) -> Result<Self> {
        Ok(Self {
            cache: CommandCache::new().await?,
            generator: LlmGenerator::new(),
            executor: Executor::new(verbose),
            permission_ui: PermissionUI::new(verbose),
            verbose,
        })
    }

    /// Processes a user intent and executes the appropriate command.
    ///
    /// This is the main entry point for command execution. The router determines
    /// how to handle the intent based on:
    ///
    /// 1. If the first argument is a system command, execute it directly
    /// 2. If the command is cached, retrieve and execute with permission check
    /// 3. If the intent is conversational (contains spaces), generate from description
    /// 4. Otherwise, generate a new command with the given name
    ///
    /// # Arguments
    ///
    /// * `intent_args` - The command name and arguments, or a natural language description
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Command generation fails
    /// - Command execution fails
    /// - Cache operations fail
    pub async fn process_intent(&mut self, intent_args: Vec<String>) -> Result<()> {
        // Conversational mode: single argument with spaces = natural language
        if intent_args.len() == 1 && intent_args[0].contains(' ') {
            info!("Detected conversational mode: {}", intent_args[0]);
            return self.process_conversational_intent(&intent_args[0]).await;
        }

        let command_name = &intent_args[0];
        let args = &intent_args[1..];

        info!("Processing intent: {} with args: {:?}", command_name, args);

        // Check if command exists in system PATH
        if which(command_name).is_ok() {
            info!("Command '{}' found in system PATH, executing directly", command_name);
            return self.executor.execute_system_command(&intent_args).await;
        }

        // Check if command exists in our cache
        if let Some(cached_command) = self.cache.get_command(command_name).await? {
            info!("Command '{}' found in cache, checking permissions", command_name);
            
            if let Some(decision) = self.check_and_request_permissions(command_name, &cached_command).await? {
                match decision.consent {
                    PermissionConsent::AcceptOnce | PermissionConsent::AcceptForever => {
                        self.permission_ui.show_running_with_permissions(command_name, &cached_command.permissions);
                        self.cache.update_usage(command_name).await?;
                        let _result = self.executor.execute_generated_command_with_context(&cached_command, &self.cache, args).await;
                        return Ok(());
                    }
                    PermissionConsent::Denied => {
                        self.permission_ui.show_permission_denied(command_name);
                        return Ok(());
                    }
                }
            } else {
                // User denied permission, don't execute
                return Ok(());
            }
        }

        // Generate new command using LLM
        if self.verbose {
            println!("⚡ Command '{}' not found, generating with AI...", command_name);
        }
        warn!("Command '{}' not found, generating with AI", command_name);
        let generation_result = self.generator.generate_command(command_name, args).await?;
        
        // Cache the generated command and its script
        self.cache.store_command(command_name, &generation_result.command, &generation_result.script_content).await?;
        
        // Check permissions for generated command
        if let Some(decision) = self.check_and_request_permissions(command_name, &generation_result.command).await? {
            match decision.consent {
                PermissionConsent::AcceptOnce | PermissionConsent::AcceptForever => {
                    self.permission_ui.show_running_with_permissions(command_name, &generation_result.command.permissions);
                    self.cache.update_usage(command_name).await?;
                    let _result = self.executor.execute_generated_command_with_context(&generation_result.command, &self.cache, args).await;
                    Ok(())
                }
                PermissionConsent::Denied => {
                    self.permission_ui.show_permission_denied(command_name);
                    Ok(())
                }
            }
        } else {
            // User denied permission, don't execute
            Ok(())
        }
    }

    /// Processes a natural language description to generate and execute a command.
    ///
    /// This handles "conversational mode" where the user provides a description
    /// instead of a command name. The LLM will suggest both the command name
    /// and implementation.
    async fn process_conversational_intent(&mut self, description: &str) -> Result<()> {
        info!("Processing conversational intent: {}", description);
        if self.verbose {
            println!("💭 Understanding your request: {}", description);
        }

        // Generate command from natural language description
        let generation_result = self
            .generator
            .generate_command_from_description(description)
            .await?;
        
        if self.verbose {
            println!("🎯 Generated command: {}", generation_result.command.name);
            println!("📝 Description: {}", generation_result.command.description);
        }
        
        // Cache the generated command and its script
        self.cache.store_command(&generation_result.command.name, &generation_result.command, &generation_result.script_content).await?;
        
        // Check permissions for generated command
        if let Some(decision) = self.check_and_request_permissions(&generation_result.command.name, &generation_result.command).await? {
            match decision.consent {
                PermissionConsent::AcceptOnce | PermissionConsent::AcceptForever => {
                    self.permission_ui.show_running_with_permissions(&generation_result.command.name, &generation_result.command.permissions);
                    self.cache.update_usage(&generation_result.command.name).await?;
                    let _result = self.executor.execute_generated_command_with_context(&generation_result.command, &self.cache, &[]).await;
                    Ok(())
                }
                PermissionConsent::Denied => {
                    self.permission_ui.show_permission_denied(&generation_result.command.name);
                    Ok(())
                }
            }
        } else {
            // User denied permission, don't execute
            Ok(())
        }
    }

    /// Checks and requests permission consent for a command.
    ///
    /// If the user has previously granted "AcceptForever" consent, returns the
    /// stored decision. Otherwise, prompts the user for consent and stores
    /// their decision.
    ///
    /// # Returns
    ///
    /// - `Some(decision)` with the user's consent choice
    /// - The decision is also persisted to the cache
    async fn check_and_request_permissions(
        &mut self,
        command_name: &str,
        command: &crate::llm_generator::GeneratedCommand,
    ) -> Result<Option<crate::command_cache::PermissionDecision>> {
        // Check if we need to ask for consent
        if !self.cache.needs_permission_consent(command_name) {
            // Permission already granted forever, return existing decision
            if let Some(decision) = self.cache.get_permission_decision(command_name) {
                return Ok(Some(decision.clone()));
            }
        }

        // Ask user for consent
        let consent = self.permission_ui.prompt_for_consent(
            command_name,
            &command.description,
            &command.permissions,
        )?;

        // Create and store decision
        let decision = self
            .permission_ui
            .create_permission_decision(command.permissions.clone(), consent);

        self.cache
            .set_permission_decision(command_name, decision.clone())
            .await?;

        Ok(Some(decision))
    }
}