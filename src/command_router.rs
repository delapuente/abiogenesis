use crate::{command_cache::CommandCache, executor::Executor, llm_generator::{LlmGenerator, CommandGenerator}};
use anyhow::Result;
use tracing::{info, warn};
use which::which;

pub struct CommandRouter {
    cache: CommandCache,
    generator: LlmGenerator,
    executor: Executor,
}

impl CommandRouter {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            cache: CommandCache::new().await?,
            generator: LlmGenerator::new(),
            executor: Executor::new(),
        })
    }

    pub async fn process_intent(&mut self, intent_args: Vec<String>) -> Result<()> {
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
            info!("Command '{}' found in cache, executing", command_name);
            return self.executor.execute_cached_command(cached_command, &self.cache, args).await;
        }

        // Generate new command using LLM
        warn!("Command '{}' not found, generating with AI", command_name);
        let generation_result = self.generator.generate_command(command_name, args).await?;
        
        // Cache the generated command and its script
        self.cache.store_command(command_name, &generation_result.command, &generation_result.script_content).await?;
        
        // Execute the generated command
        self.executor.execute_generated_command(&generation_result.command, &self.cache, args).await
    }
}