use crate::{
    command_cache::{CommandCache, PermissionConsent},
    executor::Executor,
    llm_generator::{LlmGenerator, CommandGenerator},
    permission_ui::PermissionUI,
};
use anyhow::Result;
use tracing::{info, warn};
use which::which;

pub struct CommandRouter {
    cache: CommandCache,
    generator: LlmGenerator,
    executor: Executor,
    permission_ui: PermissionUI,
}

impl CommandRouter {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            cache: CommandCache::new().await?,
            generator: LlmGenerator::new(),
            executor: Executor::new(),
            permission_ui: PermissionUI::new(),
        })
    }

    pub async fn process_intent(&mut self, intent_args: Vec<String>) -> Result<()> {
        // Check if this is conversational mode (single argument with spaces = natural language description)
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
                        return self.executor.execute_cached_command(cached_command, &self.cache, args).await;
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
                    self.executor.execute_generated_command(&generation_result.command, &self.cache, args).await
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

    async fn process_conversational_intent(&mut self, description: &str) -> Result<()> {
        info!("Processing conversational intent: {}", description);
        println!("💭 Understanding your request: {}", description);
        
        // Generate command from natural language description
        let generation_result = self.generator.generate_command_from_description(description).await?;
        
        println!("🎯 Generated command: {}", generation_result.command.name);
        println!("📝 Description: {}", generation_result.command.description);
        
        // Cache the generated command and its script
        self.cache.store_command(&generation_result.command.name, &generation_result.command, &generation_result.script_content).await?;
        
        // Check permissions for generated command
        if let Some(decision) = self.check_and_request_permissions(&generation_result.command.name, &generation_result.command).await? {
            match decision.consent {
                PermissionConsent::AcceptOnce | PermissionConsent::AcceptForever => {
                    self.permission_ui.show_running_with_permissions(&generation_result.command.name, &generation_result.command.permissions);
                    self.cache.update_usage(&generation_result.command.name).await?;
                    self.executor.execute_generated_command(&generation_result.command, &self.cache, &[]).await
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
        let decision = self.permission_ui.create_permission_decision(
            command.permissions.clone(),
            consent,
        );

        self.cache.set_permission_decision(command_name, decision.clone()).await?;
        
        Ok(Some(decision))
    }
}