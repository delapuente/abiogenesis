use crate::llm_generator::GeneratedCommand;
use anyhow::Result;
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    command: GeneratedCommand,
    created_at: u64,
    usage_count: u32,
    last_used: u64,
}

pub struct CommandCache {
    cache_dir: PathBuf,
    cache: HashMap<String, CacheEntry>,
}

impl CommandCache {
    pub async fn new() -> Result<Self> {
        let cache_dir = Self::get_cache_dir()?;
        fs::create_dir_all(&cache_dir)?;
        
        let cache_file = cache_dir.join("commands.json");
        let cache = if cache_file.exists() {
            let content = fs::read_to_string(&cache_file)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };

        info!("Command cache initialized with {} entries", cache.len());

        Ok(Self { cache_dir, cache })
    }

    fn get_cache_dir() -> Result<PathBuf> {
        let home = home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        let base_dir = home.join(".abiogenesis").join("cache");
        
        // Use separate cache directories for different modes
        if std::env::var("ABIOGENESIS_USE_MOCK").is_ok() {
            Ok(base_dir.join("mock"))
        } else {
            Ok(base_dir.join("production"))
        }
    }

    pub async fn get_command(&self, name: &str) -> Result<Option<GeneratedCommand>> {
        if let Some(entry) = self.cache.get(name) {
            info!("Found cached command: {}", name);
            Ok(Some(entry.command.clone()))
        } else {
            Ok(None)
        }
    }

    pub async fn store_command(&mut self, name: &str, command: &GeneratedCommand) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        let entry = CacheEntry {
            command: command.clone(),
            created_at: now,
            usage_count: 0,
            last_used: now,
        };

        self.cache.insert(name.to_string(), entry);
        self.persist_cache().await?;
        
        info!("Stored command in cache: {}", name);
        Ok(())
    }

    pub async fn update_usage(&mut self, name: &str) -> Result<()> {
        if let Some(entry) = self.cache.get_mut(name) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();

            entry.usage_count += 1;
            entry.last_used = now;
            self.persist_cache().await?;
        }
        Ok(())
    }

    async fn persist_cache(&self) -> Result<()> {
        let cache_file = self.cache_dir.join("commands.json");
        let content = serde_json::to_string_pretty(&self.cache)?;
        fs::write(cache_file, content)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn list_cached_commands(&self) -> Vec<String> {
        self.cache.keys().cloned().collect()
    }

    #[allow(dead_code)]
    pub async fn clear_cache(&mut self) -> Result<()> {
        self.cache.clear();
        self.persist_cache().await?;
        info!("Cache cleared");
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn get_stats(&self) -> Result<String> {
        let total_commands = self.cache.len();
        let total_usage: u32 = self.cache.values().map(|e| e.usage_count).sum();
        
        Ok(format!(
            "Cache Stats:\n- Total commands: {}\n- Total usage: {}\n- Average usage: {:.2}",
            total_commands,
            total_usage,
            if total_commands > 0 { total_usage as f64 / total_commands as f64 } else { 0.0 }
        ))
    }
}