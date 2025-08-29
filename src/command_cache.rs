use crate::llm_generator::GeneratedCommand;
use anyhow::Result;
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, debug};

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    command: GeneratedCommand,
    created_at: u64,
    usage_count: u32,
    last_used: u64,
}

pub struct CommandCache {
    write_cache_dir: PathBuf, // Directory where new commands are written
    write_cache: HashMap<String, CacheEntry>, // In-memory cache for the write directory
}

impl CommandCache {
    pub async fn new() -> Result<Self> {
        let write_cache_dir = Self::get_write_cache_dir()?;
        fs::create_dir_all(&write_cache_dir)?;
        
        // Load the write cache (closest .abiogenesis/biomas or home)
        let cache_file = write_cache_dir.join("commands.json");
        let write_cache = if cache_file.exists() {
            let content = fs::read_to_string(&cache_file)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };

        info!("Write cache initialized at {:?} with {} entries", write_cache_dir, write_cache.len());

        Ok(Self { write_cache_dir, write_cache })
    }

    fn get_write_cache_dir() -> Result<PathBuf> {
        // Find the closest .abiogenesis directory (for writing new commands)
        let mut current_dir = env::current_dir()?;
        
        loop {
            let abiogenesis_dir = current_dir.join(".abiogenesis");
            if abiogenesis_dir.exists() && abiogenesis_dir.is_dir() {
                let biomas_dir = abiogenesis_dir.join("biomas");
                
                let cache_dir = if env::var("ABIOGENESIS_USE_MOCK").is_ok() {
                    biomas_dir.join("mock")
                } else {
                    biomas_dir.join("production")
                };
                
                return Ok(cache_dir);
            }
            
            // Move to parent directory
            match current_dir.parent() {
                Some(parent) => current_dir = parent.to_path_buf(),
                None => break,
            }
        }
        
        // Fall back to home directory
        let home = home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        let base_dir = home.join(".abiogenesis").join("biomas");
        
        if env::var("ABIOGENESIS_USE_MOCK").is_ok() {
            Ok(base_dir.join("mock"))
        } else {
            Ok(base_dir.join("production"))
        }
    }

    fn find_command_in_hierarchy(&self, name: &str) -> Result<Option<GeneratedCommand>> {
        let mut current_dir = env::current_dir()?;
        
        loop {
            let abiogenesis_dir = current_dir.join(".abiogenesis");
            if abiogenesis_dir.exists() && abiogenesis_dir.is_dir() {
                let biomas_dir = abiogenesis_dir.join("biomas");
                
                let cache_dir = if env::var("ABIOGENESIS_USE_MOCK").is_ok() {
                    biomas_dir.join("mock")
                } else {
                    biomas_dir.join("production")
                };
                
                let cache_file = cache_dir.join("commands.json");
                if cache_file.exists() {
                    if let Ok(content) = fs::read_to_string(&cache_file) {
                        if let Ok(cache) = serde_json::from_str::<HashMap<String, CacheEntry>>(&content) {
                            if let Some(entry) = cache.get(name) {
                                debug!("Found command '{}' in cache at {:?}", name, cache_dir);
                                return Ok(Some(entry.command.clone()));
                            }
                        }
                    }
                }
            }
            
            // Move to parent directory
            match current_dir.parent() {
                Some(parent) => current_dir = parent.to_path_buf(),
                None => break,
            }
        }
        
        // Check home directory cache as fallback
        let home = home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        let base_dir = home.join(".abiogenesis").join("biomas");
        
        let cache_dir = if env::var("ABIOGENESIS_USE_MOCK").is_ok() {
            base_dir.join("mock")
        } else {
            base_dir.join("production")
        };
        
        let cache_file = cache_dir.join("commands.json");
        if cache_file.exists() {
            if let Ok(content) = fs::read_to_string(&cache_file) {
                if let Ok(cache) = serde_json::from_str::<HashMap<String, CacheEntry>>(&content) {
                    if let Some(entry) = cache.get(name) {
                        debug!("Found command '{}' in home cache at {:?}", name, cache_dir);
                        return Ok(Some(entry.command.clone()));
                    }
                }
            }
        }
        
        Ok(None)
    }

    pub async fn get_command(&self, name: &str) -> Result<Option<GeneratedCommand>> {
        // First check the write cache (in-memory cache for closest biomas)
        if let Some(entry) = self.write_cache.get(name) {
            info!("Found cached command '{}' in write cache", name);
            return Ok(Some(entry.command.clone()));
        }
        
        // Then search up the hierarchy for the command
        if let Some(command) = self.find_command_in_hierarchy(name)? {
            info!("Found cached command '{}' in hierarchy", name);
            return Ok(Some(command));
        }
        
        Ok(None)
    }

    pub fn get_script_content(&self, command: &GeneratedCommand) -> Result<String> {
        // First try the write cache directory
        let script_path = self.write_cache_dir.join(&command.script_file);
        if script_path.exists() {
            return Ok(fs::read_to_string(&script_path)?);
        }
        
        // Then search up the hierarchy for the script file
        let mut current_dir = env::current_dir()?;
        
        loop {
            let abiogenesis_dir = current_dir.join(".abiogenesis");
            if abiogenesis_dir.exists() && abiogenesis_dir.is_dir() {
                let biomas_dir = abiogenesis_dir.join("biomas");
                
                let cache_dir = if env::var("ABIOGENESIS_USE_MOCK").is_ok() {
                    biomas_dir.join("mock")
                } else {
                    biomas_dir.join("production")
                };
                
                let script_path = cache_dir.join(&command.script_file);
                if script_path.exists() {
                    debug!("Found script file '{}' at {:?}", command.script_file, cache_dir);
                    return Ok(fs::read_to_string(&script_path)?);
                }
            }
            
            // Move to parent directory
            match current_dir.parent() {
                Some(parent) => current_dir = parent.to_path_buf(),
                None => break,
            }
        }
        
        // Check home directory as fallback
        let home = home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        let base_dir = home.join(".abiogenesis").join("biomas");
        
        let cache_dir = if env::var("ABIOGENESIS_USE_MOCK").is_ok() {
            base_dir.join("mock")
        } else {
            base_dir.join("production")
        };
        
        let script_path = cache_dir.join(&command.script_file);
        if script_path.exists() {
            debug!("Found script file '{}' in home cache at {:?}", command.script_file, cache_dir);
            return Ok(fs::read_to_string(&script_path)?);
        }
        
        Err(anyhow::anyhow!("Script file '{}' not found in any biomas directory", command.script_file))
    }

    pub async fn store_command(&mut self, name: &str, command: &GeneratedCommand, script_content: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        // Write the script file directly to the biomas directory
        let script_filename = format!("{}.ts", name);
        let script_path = self.write_cache_dir.join(&script_filename);
        fs::write(&script_path, script_content)?;
        
        // Create command entry with script file reference
        let command_with_file = GeneratedCommand {
            name: command.name.clone(),
            description: command.description.clone(),
            script_file: script_filename.clone(),
            permissions: command.permissions.clone(),
        };

        let entry = CacheEntry {
            command: command_with_file,
            created_at: now,
            usage_count: 0,
            last_used: now,
        };

        self.write_cache.insert(name.to_string(), entry);
        self.persist_write_cache().await?;
        
        info!("Stored command '{}' with script file '{}' in write cache at {:?}", name, script_filename, self.write_cache_dir);
        Ok(())
    }

    pub async fn update_usage(&mut self, name: &str) -> Result<()> {
        if let Some(entry) = self.write_cache.get_mut(name) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();

            entry.usage_count += 1;
            entry.last_used = now;
            self.persist_write_cache().await?;
            debug!("Updated usage for command '{}' in write cache", name);
        }
        Ok(())
    }

    async fn persist_write_cache(&self) -> Result<()> {
        let cache_file = self.write_cache_dir.join("commands.json");
        let content = serde_json::to_string_pretty(&self.write_cache)?;
        fs::write(cache_file, content)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn list_cached_commands(&self) -> Vec<String> {
        self.write_cache.keys().cloned().collect()
    }

    #[allow(dead_code)]
    pub async fn clear_cache(&mut self) -> Result<()> {
        self.write_cache.clear();
        self.persist_write_cache().await?;
        info!("Write cache cleared");
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn get_stats(&self) -> Result<String> {
        let total_commands = self.write_cache.len();
        let total_usage: u32 = self.write_cache.values().map(|e| e.usage_count).sum();
        
        Ok(format!(
            "Write Cache Stats:\n- Total commands: {}\n- Total usage: {}\n- Average usage: {:.2}\n- Cache directory: {:?}",
            total_commands,
            total_usage,
            if total_commands > 0 { total_usage as f64 / total_commands as f64 } else { 0.0 },
            self.write_cache_dir
        ))
    }
}