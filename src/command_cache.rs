//! Command caching and persistence module.
//!
//! This module provides persistent storage for generated commands. Commands are
//! stored in a hierarchical cache system that searches from the current directory
//! upward, falling back to the home directory.
//!
//! # Cache Structure
//!
//! Commands are stored in `.abiogenesis/biomas/` directories:
//! - `commands.json` - Command metadata and permission decisions
//! - `*.ts` - Generated TypeScript script files
//!
//! # Hierarchy Resolution
//!
//! When looking up commands, the cache searches:
//! 1. Current directory's `.abiogenesis/biomas/`
//! 2. Parent directories' `.abiogenesis/biomas/`
//! 3. Home directory's `~/.abiogenesis/biomas/`

use crate::llm_generator::{GeneratedCommand, PermissionRequest};
use crate::providers::{SystemTimeProvider, TimeProvider};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info};

// =============================================================================
// Traits for Dependency Injection
// =============================================================================

/// Trait for resolving cache paths.
///
/// This abstraction enables testing without relying on the actual filesystem
/// or home directory.
pub trait CachePathResolver: Send + Sync {
    /// Returns the directory where new commands should be written.
    fn get_write_dir(&self) -> Result<PathBuf>;

    /// Finds a command by name.
    ///
    /// Returns the command if found, None otherwise.
    fn find_command(&self, name: &str) -> Result<Option<GeneratedCommand>>;

    /// Finds a script file by name.
    ///
    /// Returns the script content if found, None otherwise.
    fn find_script(&self, script_file: &str) -> Result<Option<String>>;
}

// =============================================================================
// Default Implementations
// =============================================================================

/// Default path resolver that searches the actual filesystem hierarchy.
pub struct HierarchyPathResolver;

impl HierarchyPathResolver {
    /// Creates a new hierarchy path resolver.
    pub fn new() -> Self {
        Self
    }

    /// Gets all cache directories, from closest to home.
    fn get_cache_dirs(&self) -> Result<Vec<PathBuf>> {
        let mut dirs = Vec::new();
        let mut current_dir = std::env::current_dir()?;

        // Search upward from current directory
        loop {
            let abiogenesis_dir = current_dir.join(".abiogenesis");
            if abiogenesis_dir.exists() && abiogenesis_dir.is_dir() {
                let cache_dir = abiogenesis_dir.join("biomas");
                dirs.push(cache_dir);
            }

            match current_dir.parent() {
                Some(parent) => current_dir = parent.to_path_buf(),
                None => break,
            }
        }

        // Add home directory as fallback
        if let Some(home) = dirs::home_dir() {
            let home_cache = home.join(".abiogenesis").join("biomas");
            if !dirs.contains(&home_cache) {
                dirs.push(home_cache);
            }
        }

        Ok(dirs)
    }
}

impl Default for HierarchyPathResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl CachePathResolver for HierarchyPathResolver {
    fn get_write_dir(&self) -> Result<PathBuf> {
        let dirs = self.get_cache_dirs()?;
        dirs.into_iter().next().ok_or_else(|| {
            anyhow::anyhow!("Could not determine cache directory: no home directory found")
        })
    }

    fn find_command(&self, name: &str) -> Result<Option<GeneratedCommand>> {
        for cache_dir in self.get_cache_dirs()? {
            let cache_file = cache_dir.join("commands.json");
            if cache_file.exists() {
                if let Ok(content) = fs::read_to_string(&cache_file) {
                    if let Ok(cache) = serde_json::from_str::<HashMap<String, CacheEntry>>(&content)
                    {
                        if let Some(entry) = cache.get(name) {
                            debug!("Found command '{}' in cache at {:?}", name, cache_dir);
                            return Ok(Some(entry.command.clone()));
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    fn find_script(&self, script_file: &str) -> Result<Option<String>> {
        for cache_dir in self.get_cache_dirs()? {
            let script_path = cache_dir.join(script_file);
            if script_path.exists() {
                debug!("Found script file '{}' at {:?}", script_file, cache_dir);
                return Ok(Some(fs::read_to_string(&script_path)?));
            }
        }
        Ok(None)
    }
}

// =============================================================================
// Data Types
// =============================================================================

/// User's consent choice for command permissions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PermissionConsent {
    /// Run once with these permissions, ask again next time.
    AcceptOnce,
    /// Always run with these permissions without asking.
    AcceptForever,
    /// User explicitly denied execution.
    Denied,
}

/// A user's permission decision for a command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDecision {
    /// The permissions that were requested.
    pub permissions: Vec<PermissionRequest>,
    /// The user's consent choice.
    pub consent: PermissionConsent,
    /// Unix timestamp when the decision was made.
    pub decided_at: u64,
}

/// Internal cache entry storing command metadata and usage statistics.
#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    command: GeneratedCommand,
    created_at: u64,
    usage_count: u32,
    last_used: u64,
    permission_decision: Option<PermissionDecision>,
}

// =============================================================================
// CommandCache Implementation
// =============================================================================

/// Persistent storage for generated commands.
///
/// The cache stores commands in a hierarchical directory structure, allowing
/// project-specific and global commands to coexist.
///
/// # Example
///
/// ```ignore
/// let cache = CommandCache::new().await?;
///
/// // Store a new command
/// cache.store_command("hello", &command, "console.log('Hello');").await?;
///
/// // Retrieve it later
/// if let Some(cmd) = cache.get_command("hello").await? {
///     println!("Found: {}", cmd.description);
/// }
/// ```
pub struct CommandCache {
    /// Directory where new commands are written.
    write_cache_dir: PathBuf,
    /// In-memory cache for the write directory.
    write_cache: HashMap<String, CacheEntry>,
    /// Path resolver for cache operations.
    path_resolver: Box<dyn CachePathResolver>,
    /// Time provider for timestamps.
    time_provider: Box<dyn TimeProvider>,
}

impl CommandCache {
    /// Creates a new command cache using default path resolution.
    ///
    /// The cache directory is determined by searching upward from the current
    /// directory for a `.abiogenesis` folder, falling back to the home directory.
    pub async fn new() -> Result<Self> {
        Self::with_providers(
            Box::new(HierarchyPathResolver::new()),
            Box::new(SystemTimeProvider),
        )
        .await
    }

    /// Creates a command cache with custom providers (for testing).
    pub async fn with_providers(
        path_resolver: Box<dyn CachePathResolver>,
        time_provider: Box<dyn TimeProvider>,
    ) -> Result<Self> {
        let write_cache_dir = path_resolver.get_write_dir()?;
        fs::create_dir_all(&write_cache_dir)?;

        let cache_file = write_cache_dir.join("commands.json");
        let write_cache = if cache_file.exists() {
            let content = fs::read_to_string(&cache_file)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };

        info!(
            "Write cache initialized at {:?} with {} entries",
            write_cache_dir,
            write_cache.len()
        );

        Ok(Self {
            write_cache_dir,
            write_cache,
            path_resolver,
            time_provider,
        })
    }

    /// Retrieves a command by name from the cache.
    ///
    /// Searches the in-memory cache first, then uses the path resolver.
    pub async fn get_command(&self, name: &str) -> Result<Option<GeneratedCommand>> {
        // First check the write cache (in-memory)
        if let Some(entry) = self.write_cache.get(name) {
            info!("Found cached command '{}' in write cache", name);
            return Ok(Some(entry.command.clone()));
        }

        // Then use the path resolver
        if let Some(command) = self.path_resolver.find_command(name)? {
            info!("Found cached command '{}' via path resolver", name);
            return Ok(Some(command));
        }

        Ok(None)
    }

    /// Retrieves the script content for a command.
    ///
    /// Searches the write cache directory first, then uses the path resolver.
    pub fn get_script_content(&self, command: &GeneratedCommand) -> Result<String> {
        // First try the write cache directory
        let script_path = self.write_cache_dir.join(&command.script_file);
        if script_path.exists() {
            return Ok(fs::read_to_string(&script_path)?);
        }

        // Then use the path resolver
        if let Some(content) = self.path_resolver.find_script(&command.script_file)? {
            return Ok(content);
        }

        Err(anyhow::anyhow!(
            "Script file '{}' not found",
            command.script_file
        ))
    }

    /// Stores a new command in the cache.
    ///
    /// # Arguments
    ///
    /// * `name` - The command name (used for lookup)
    /// * `command` - The command metadata
    /// * `script_content` - The TypeScript source code
    pub async fn store_command(
        &mut self,
        name: &str,
        command: &GeneratedCommand,
        script_content: &str,
    ) -> Result<()> {
        let now = self.time_provider.now();

        // Write the script file
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
            permission_decision: None,
        };

        self.write_cache.insert(name.to_string(), entry);
        self.persist_write_cache().await?;

        info!(
            "Stored command '{}' with script file '{}' at {:?}",
            name, script_filename, self.write_cache_dir
        );
        Ok(())
    }

    /// Updates the usage statistics for a command.
    pub async fn update_usage(&mut self, name: &str) -> Result<()> {
        if let Some(entry) = self.write_cache.get_mut(name) {
            let now = self.time_provider.now();
            entry.usage_count += 1;
            entry.last_used = now;
            self.persist_write_cache().await?;
            debug!("Updated usage for command '{}'", name);
        }
        Ok(())
    }

    /// Persists the in-memory cache to disk.
    async fn persist_write_cache(&self) -> Result<()> {
        let cache_file = self.write_cache_dir.join("commands.json");
        let content = serde_json::to_string_pretty(&self.write_cache)?;
        fs::write(cache_file, content)?;
        Ok(())
    }

    /// Lists all cached command names.
    #[allow(dead_code)]
    pub async fn list_cached_commands(&self) -> Vec<String> {
        self.write_cache.keys().cloned().collect()
    }

    /// Stores a permission decision for a command.
    pub async fn set_permission_decision(
        &mut self,
        name: &str,
        decision: PermissionDecision,
    ) -> Result<()> {
        if let Some(entry) = self.write_cache.get_mut(name) {
            entry.permission_decision = Some(decision);
            self.persist_write_cache().await?;
            info!("Updated permission decision for command '{}'", name);
        }
        Ok(())
    }

    /// Retrieves the permission decision for a command.
    pub fn get_permission_decision(&self, name: &str) -> Option<&PermissionDecision> {
        self.write_cache.get(name)?.permission_decision.as_ref()
    }

    /// Checks if permission consent is needed for a command.
    ///
    /// Returns true if:
    /// - No decision has been made yet
    /// - The previous decision was AcceptOnce
    /// - The previous decision was Denied (user might change their mind)
    pub fn needs_permission_consent(&self, name: &str) -> bool {
        match self.get_permission_decision(name) {
            None => true,
            Some(decision) => match decision.consent {
                PermissionConsent::AcceptOnce => true,
                PermissionConsent::AcceptForever => false,
                PermissionConsent::Denied => true,
            },
        }
    }

    /// Removes a command and its script file from the cache.
    pub async fn remove_command(&mut self, name: &str) -> Result<bool> {
        if let Some(entry) = self.write_cache.remove(name) {
            let script_path = self.write_cache_dir.join(&entry.command.script_file);
            if script_path.exists() {
                fs::remove_file(script_path)?;
            }
            self.persist_write_cache().await?;
            info!("Removed command '{}' and its script file", name);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Clears all commands from the cache.
    pub async fn clear_cache(&mut self) -> Result<()> {
        for entry in self.write_cache.values() {
            let script_path = self.write_cache_dir.join(&entry.command.script_file);
            if script_path.exists() {
                fs::remove_file(script_path).ok();
            }
        }

        self.write_cache.clear();
        self.persist_write_cache().await?;
        info!("Cache cleared");
        Ok(())
    }

    /// Lists all commands with their metadata and permission decisions.
    pub async fn list_commands(
        &self,
    ) -> Vec<(String, &GeneratedCommand, Option<&PermissionDecision>)> {
        self.write_cache
            .iter()
            .map(|(name, entry)| {
                (
                    name.clone(),
                    &entry.command,
                    entry.permission_decision.as_ref(),
                )
            })
            .collect()
    }

    /// Returns cache statistics.
    #[allow(dead_code)]
    pub async fn get_stats(&self) -> Result<String> {
        let total_commands = self.write_cache.len();
        let total_usage: u32 = self.write_cache.values().map(|e| e.usage_count).sum();
        let accepted_forever = self
            .write_cache
            .values()
            .filter(|e| {
                matches!(
                    e.permission_decision.as_ref().map(|d| &d.consent),
                    Some(PermissionConsent::AcceptForever)
                )
            })
            .count();

        Ok(format!(
            "Cache Stats:\n\
             - Total commands: {}\n\
             - Total usage: {}\n\
             - Average usage: {:.2}\n\
             - Accepted forever: {}\n\
             - Cache directory: {:?}",
            total_commands,
            total_usage,
            if total_commands > 0 {
                total_usage as f64 / total_commands as f64
            } else {
                0.0
            },
            accepted_forever,
            self.write_cache_dir
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // =========================================================================
    // Mock implementations
    // =========================================================================

    /// Mock path resolver for testing.
    struct MockPathResolver {
        write_dir: PathBuf,
        commands: HashMap<String, GeneratedCommand>,
        scripts: HashMap<String, String>,
    }

    impl MockPathResolver {
        fn new(write_dir: PathBuf) -> Self {
            Self {
                write_dir,
                commands: HashMap::new(),
                scripts: HashMap::new(),
            }
        }
    }

    impl CachePathResolver for MockPathResolver {
        fn get_write_dir(&self) -> Result<PathBuf> {
            Ok(self.write_dir.clone())
        }

        fn find_command(&self, name: &str) -> Result<Option<GeneratedCommand>> {
            Ok(self.commands.get(name).cloned())
        }

        fn find_script(&self, script_file: &str) -> Result<Option<String>> {
            Ok(self.scripts.get(script_file).cloned())
        }
    }

    /// Mock time provider for deterministic testing.
    struct MockTimeProvider {
        time: u64,
    }

    impl MockTimeProvider {
        fn new(time: u64) -> Self {
            Self { time }
        }
    }

    impl TimeProvider for MockTimeProvider {
        fn now(&self) -> u64 {
            self.time
        }
    }

    /// Creates a test command.
    fn test_command(name: &str) -> GeneratedCommand {
        GeneratedCommand {
            name: name.to_string(),
            description: format!("Test command: {}", name),
            script_file: format!("{}.ts", name),
            permissions: vec![],
        }
    }

    // =========================================================================
    // CommandCache tests
    // =========================================================================

    #[tokio::test]
    async fn test_new_cache_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("cache");

        let resolver = MockPathResolver::new(cache_dir.clone());
        let time = MockTimeProvider::new(1000);

        let cache = CommandCache::with_providers(Box::new(resolver), Box::new(time))
            .await
            .unwrap();

        assert!(cache_dir.exists());
        assert!(cache.write_cache.is_empty());
    }

    #[tokio::test]
    async fn test_store_and_get_command() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = MockPathResolver::new(temp_dir.path().to_path_buf());
        let time = MockTimeProvider::new(1000);

        let mut cache = CommandCache::with_providers(Box::new(resolver), Box::new(time))
            .await
            .unwrap();

        let cmd = test_command("hello");
        cache
            .store_command("hello", &cmd, "console.log('Hello');")
            .await
            .unwrap();

        let retrieved = cache.get_command("hello").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "hello");
    }

    #[tokio::test]
    async fn test_get_command_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = MockPathResolver::new(temp_dir.path().to_path_buf());
        let time = MockTimeProvider::new(1000);

        let cache = CommandCache::with_providers(Box::new(resolver), Box::new(time))
            .await
            .unwrap();

        let result = cache.get_command("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_script_content() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = MockPathResolver::new(temp_dir.path().to_path_buf());
        let time = MockTimeProvider::new(1000);

        let mut cache = CommandCache::with_providers(Box::new(resolver), Box::new(time))
            .await
            .unwrap();

        let cmd = test_command("hello");
        let script = "console.log('Hello, World!');";
        cache.store_command("hello", &cmd, script).await.unwrap();

        let content = cache.get_script_content(&cmd).unwrap();
        assert_eq!(content, script);
    }

    #[tokio::test]
    async fn test_update_usage() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = MockPathResolver::new(temp_dir.path().to_path_buf());
        let time = MockTimeProvider::new(1000);

        let mut cache = CommandCache::with_providers(Box::new(resolver), Box::new(time))
            .await
            .unwrap();

        let cmd = test_command("hello");
        cache
            .store_command("hello", &cmd, "console.log('Hello');")
            .await
            .unwrap();

        cache.update_usage("hello").await.unwrap();
        cache.update_usage("hello").await.unwrap();

        // Verify usage count is stored (check via the cache file)
        let cache_file = temp_dir.path().join("commands.json");
        let content = fs::read_to_string(&cache_file).unwrap();
        assert!(content.contains("\"usage_count\": 2"));
    }

    #[tokio::test]
    async fn test_remove_command() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = MockPathResolver::new(temp_dir.path().to_path_buf());
        let time = MockTimeProvider::new(1000);

        let mut cache = CommandCache::with_providers(Box::new(resolver), Box::new(time))
            .await
            .unwrap();

        let cmd = test_command("hello");
        cache
            .store_command("hello", &cmd, "console.log('Hello');")
            .await
            .unwrap();

        let removed = cache.remove_command("hello").await.unwrap();
        assert!(removed);

        let result = cache.get_command("hello").await.unwrap();
        assert!(result.is_none());

        // Script file should be removed
        let script_path = temp_dir.path().join("hello.ts");
        assert!(!script_path.exists());
    }

    #[tokio::test]
    async fn test_remove_nonexistent_command() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = MockPathResolver::new(temp_dir.path().to_path_buf());
        let time = MockTimeProvider::new(1000);

        let mut cache = CommandCache::with_providers(Box::new(resolver), Box::new(time))
            .await
            .unwrap();

        let removed = cache.remove_command("nonexistent").await.unwrap();
        assert!(!removed);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = MockPathResolver::new(temp_dir.path().to_path_buf());
        let time = MockTimeProvider::new(1000);

        let mut cache = CommandCache::with_providers(Box::new(resolver), Box::new(time))
            .await
            .unwrap();

        cache
            .store_command("cmd1", &test_command("cmd1"), "script1")
            .await
            .unwrap();
        cache
            .store_command("cmd2", &test_command("cmd2"), "script2")
            .await
            .unwrap();

        cache.clear_cache().await.unwrap();

        assert!(cache.get_command("cmd1").await.unwrap().is_none());
        assert!(cache.get_command("cmd2").await.unwrap().is_none());
    }

    // =========================================================================
    // Permission decision tests
    // =========================================================================

    #[tokio::test]
    async fn test_needs_permission_consent_no_decision() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = MockPathResolver::new(temp_dir.path().to_path_buf());
        let time = MockTimeProvider::new(1000);

        let mut cache = CommandCache::with_providers(Box::new(resolver), Box::new(time))
            .await
            .unwrap();

        let cmd = test_command("hello");
        cache
            .store_command("hello", &cmd, "console.log('Hello');")
            .await
            .unwrap();

        assert!(cache.needs_permission_consent("hello"));
    }

    #[tokio::test]
    async fn test_needs_permission_consent_accept_once() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = MockPathResolver::new(temp_dir.path().to_path_buf());
        let time = MockTimeProvider::new(1000);

        let mut cache = CommandCache::with_providers(Box::new(resolver), Box::new(time))
            .await
            .unwrap();

        let cmd = test_command("hello");
        cache
            .store_command("hello", &cmd, "console.log('Hello');")
            .await
            .unwrap();

        let decision = PermissionDecision {
            permissions: vec![],
            consent: PermissionConsent::AcceptOnce,
            decided_at: 1000,
        };
        cache
            .set_permission_decision("hello", decision)
            .await
            .unwrap();

        assert!(cache.needs_permission_consent("hello"));
    }

    #[tokio::test]
    async fn test_needs_permission_consent_accept_forever() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = MockPathResolver::new(temp_dir.path().to_path_buf());
        let time = MockTimeProvider::new(1000);

        let mut cache = CommandCache::with_providers(Box::new(resolver), Box::new(time))
            .await
            .unwrap();

        let cmd = test_command("hello");
        cache
            .store_command("hello", &cmd, "console.log('Hello');")
            .await
            .unwrap();

        let decision = PermissionDecision {
            permissions: vec![],
            consent: PermissionConsent::AcceptForever,
            decided_at: 1000,
        };
        cache
            .set_permission_decision("hello", decision)
            .await
            .unwrap();

        assert!(!cache.needs_permission_consent("hello"));
    }

    #[tokio::test]
    async fn test_needs_permission_consent_denied() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = MockPathResolver::new(temp_dir.path().to_path_buf());
        let time = MockTimeProvider::new(1000);

        let mut cache = CommandCache::with_providers(Box::new(resolver), Box::new(time))
            .await
            .unwrap();

        let cmd = test_command("hello");
        cache
            .store_command("hello", &cmd, "console.log('Hello');")
            .await
            .unwrap();

        let decision = PermissionDecision {
            permissions: vec![],
            consent: PermissionConsent::Denied,
            decided_at: 1000,
        };
        cache
            .set_permission_decision("hello", decision)
            .await
            .unwrap();

        // Denied commands should ask again
        assert!(cache.needs_permission_consent("hello"));
    }

    #[tokio::test]
    async fn test_list_commands() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = MockPathResolver::new(temp_dir.path().to_path_buf());
        let time = MockTimeProvider::new(1000);

        let mut cache = CommandCache::with_providers(Box::new(resolver), Box::new(time))
            .await
            .unwrap();

        cache
            .store_command("cmd1", &test_command("cmd1"), "script1")
            .await
            .unwrap();
        cache
            .store_command("cmd2", &test_command("cmd2"), "script2")
            .await
            .unwrap();

        let commands = cache.list_commands().await;
        assert_eq!(commands.len(), 2);

        let names: Vec<_> = commands.iter().map(|(n, _, _)| n.as_str()).collect();
        assert!(names.contains(&"cmd1"));
        assert!(names.contains(&"cmd2"));
    }

    // =========================================================================
    // Time provider tests
    // =========================================================================

    #[tokio::test]
    async fn test_store_command_uses_time_provider() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = MockPathResolver::new(temp_dir.path().to_path_buf());
        let time = MockTimeProvider::new(12345);

        let mut cache = CommandCache::with_providers(Box::new(resolver), Box::new(time))
            .await
            .unwrap();

        let cmd = test_command("hello");
        cache
            .store_command("hello", &cmd, "console.log('Hello');")
            .await
            .unwrap();

        // Verify the timestamp in the cache file
        let cache_file = temp_dir.path().join("commands.json");
        let content = fs::read_to_string(&cache_file).unwrap();
        assert!(content.contains("\"created_at\": 12345"));
        assert!(content.contains("\"last_used\": 12345"));
    }
}