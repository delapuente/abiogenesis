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
    execution_context::ExecutionContext,
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
            return self
                .execute_with_permissions(command_name, &cached_command, args)
                .await;
        }

        // Generate new command using LLM
        if self.verbose {
            println!("⚡ Command '{}' not found, generating with AI...", command_name);
        }
        warn!("Command '{}' not found, generating with AI", command_name);
        let generation_result = self.generator.generate_command(command_name, args).await?;

        // Cache the generated command and its script
        self.cache
            .store_command(command_name, &generation_result.command, &generation_result.script_content)
            .await?;

        self.execute_with_permissions(command_name, &generation_result.command, args)
            .await
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
        self.cache
            .store_command(
                &generation_result.command.name,
                &generation_result.command,
                &generation_result.script_content,
            )
            .await?;

        self.execute_with_permissions(&generation_result.command.name, &generation_result.command, &[])
            .await
    }

    /// Processes corrective feedback loop to regenerate a command.
    ///
    /// This method loads the last execution context, regenerates the command
    /// with user feedback (or stderr if no feedback provided), and re-executes.
    ///
    /// # Arguments
    ///
    /// * `feedback` - User feedback about what went wrong (empty string uses stderr only)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No previous execution context exists
    /// - Command regeneration fails
    /// - Cache operations fail
    pub async fn process_corrective_feedback(&mut self, feedback: &str) -> Result<()> {
        // Load the last execution context
        let context = match ExecutionContext::load()? {
            Some(ctx) => ctx,
            None => {
                eprintln!("No previous command execution found. Run a command first, then use --nope.");
                return Ok(());
            }
        };

        if self.verbose {
            println!("🔄 Regenerating command '{}'...", context.command_name);
            if !feedback.is_empty() {
                println!("💭 Feedback: {}", feedback);
            } else if context.stderr.is_some() {
                println!("💭 Using stderr from last execution as context");
            }
        }

        info!(
            "Regenerating command '{}' with feedback: {}",
            context.command_name, feedback
        );

        // Regenerate the command with feedback
        let generation_result = self
            .generator
            .regenerate_command_with_feedback(
                &context.command_name,
                &context.script_content,
                context.stderr.as_deref(),
                feedback,
            )
            .await?;

        if self.verbose {
            println!("✨ Command regenerated successfully!");
            println!("📝 New description: {}", generation_result.command.description);
        }

        // Update the command in cache
        self.cache
            .store_command(
                &context.command_name,
                &generation_result.command,
                &generation_result.script_content,
            )
            .await?;

        self.execute_with_permissions(&context.command_name, &generation_result.command, &[])
            .await
    }

    /// Checks permissions and executes a generated command if approved.
    ///
    /// This is the common workflow for executing any generated command:
    /// 1. Check/request permission consent from the user
    /// 2. If approved, show permissions and execute the command
    /// 3. If denied, show denial message
    ///
    /// # Arguments
    ///
    /// * `command_name` - The name of the command to execute
    /// * `command` - The generated command metadata
    /// * `args` - Arguments to pass to the command
    async fn execute_with_permissions(
        &mut self,
        command_name: &str,
        command: &crate::llm_generator::GeneratedCommand,
        args: &[String],
    ) -> Result<()> {
        if let Some(decision) = self.check_and_request_permissions(command_name, command).await? {
            match decision.consent {
                PermissionConsent::AcceptOnce | PermissionConsent::AcceptForever => {
                    self.permission_ui
                        .show_running_with_permissions(command_name, &command.permissions);
                    self.cache.update_usage(command_name).await?;
                    let _result = self
                        .executor
                        .execute_generated_command_with_context(command, &self.cache, args)
                        .await;
                }
                PermissionConsent::Denied => {
                    self.permission_ui.show_permission_denied(command_name);
                }
            }
        }
        Ok(())
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