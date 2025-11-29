use anyhow::{anyhow, Result};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::info;

/// Provides the base directory for configuration files.
///
/// This trait enables dependency injection for testing, allowing tests to use
/// temporary directories instead of the real home directory.
pub trait ConfigPathProvider: Send + Sync {
    /// Returns the base directory where configuration files should be stored.
    ///
    /// The config file will be stored at `{base_dir}/config.toml`.
    fn get_base_dir(&self) -> Result<PathBuf>;
}

/// Default path provider that uses the user's home directory.
///
/// Configuration is stored in `~/.abiogenesis/`.
#[derive(Debug, Clone, Default)]
pub struct HomePathProvider;

impl ConfigPathProvider for HomePathProvider {
    fn get_base_dir(&self) -> Result<PathBuf> {
        let home = home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        Ok(home.join(".abiogenesis"))
    }
}

/// Application configuration for ergo.
///
/// Configuration is loaded from `{base_dir}/config.toml` with environment
/// variables taking precedence. The struct is serializable to TOML format.
///
/// # Configuration Precedence
///
/// 1. Environment variables (highest priority)
/// 2. Config file (e.g., `~/.abiogenesis/config.toml`)
/// 3. Default values (lowest priority)
///
/// # Example
///
/// ```no_run
/// use abiogenesis::config::Config;
///
/// let config = Config::load()?;
/// if let Some(key) = config.get_api_key() {
///     println!("API key is configured");
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Config {
    /// The Anthropic API key used for LLM command generation.
    ///
    /// Can be set via:
    /// - Config file: `anthropic_api_key = "sk-ant-..."`
    /// - Environment variable: `ANTHROPIC_API_KEY`
    #[serde(default)]
    pub anthropic_api_key: Option<String>,
}


impl Config {
    // =========================================================================
    // Core methods with dependency injection (testable)
    // =========================================================================

    /// Loads configuration from a file using the provided path provider.
    ///
    /// This method does NOT apply environment variable overrides. Use
    /// [`load_with_provider`] for the full loading behavior.
    ///
    /// # Arguments
    ///
    /// * `provider` - The path provider that determines where to look for config
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path provider fails to return a base directory
    /// - The config file exists but cannot be read
    /// - The config file contains invalid TOML
    pub fn load_from_file_with_provider(provider: &dyn ConfigPathProvider) -> Result<Self> {
        let config_path = Self::get_config_path_with_provider(provider)?;
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            info!("Loaded config from: {}", config_path.display());
            Ok(config)
        } else {
            Err(anyhow!("Config file not found"))
        }
    }

    /// Loads configuration with full precedence rules using the provided path provider.
    ///
    /// # Configuration Precedence
    ///
    /// 1. Environment variables (highest priority)
    /// 2. Config file
    /// 3. Default values (lowest priority)
    ///
    /// # Arguments
    ///
    /// * `provider` - The path provider that determines where to look for config
    ///
    /// # Errors
    ///
    /// Returns an error only if the path provider fails. Missing config files
    /// are handled gracefully by using defaults.
    pub fn load_with_provider(provider: &dyn ConfigPathProvider) -> Result<Self> {
        let mut config = Self::load_from_file_with_provider(provider).unwrap_or_else(|_| {
            info!("No config file found, using defaults");
            Self::default()
        });

        // Environment variables override config file
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            config.anthropic_api_key = Some(api_key);
        }

        Ok(config)
    }

    /// Saves the configuration to disk using the provided path provider.
    ///
    /// Creates the parent directory if it doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `provider` - The path provider that determines where to save config
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path provider fails to return a base directory
    /// - The parent directory cannot be created
    /// - The file cannot be written
    pub fn save_with_provider(&self, provider: &dyn ConfigPathProvider) -> Result<()> {
        let config_path = Self::get_config_path_with_provider(provider)?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&config_path, content)?;
        info!("Saved config to: {}", config_path.display());
        Ok(())
    }

    /// Returns the full path to the config file using the provided path provider.
    ///
    /// # Arguments
    ///
    /// * `provider` - The path provider that determines the base directory
    ///
    /// # Returns
    ///
    /// The path `{base_dir}/config.toml`
    pub fn get_config_path_with_provider(provider: &dyn ConfigPathProvider) -> Result<PathBuf> {
        Ok(provider.get_base_dir()?.join("config.toml"))
    }

    /// Returns the configuration directory using the provided path provider.
    ///
    /// # Arguments
    ///
    /// * `provider` - The path provider that determines the base directory
    pub fn get_config_dir_with_provider(provider: &dyn ConfigPathProvider) -> Result<PathBuf> {
        provider.get_base_dir()
    }

    /// Sets the API key and saves the configuration using the provided path provider.
    ///
    /// # Arguments
    ///
    /// * `api_key` - The Anthropic API key to store
    /// * `provider` - The path provider that determines where to save config
    ///
    /// # Errors
    ///
    /// Returns an error if saving fails.
    pub fn set_api_key_with_provider(
        &mut self,
        api_key: String,
        provider: &dyn ConfigPathProvider,
    ) -> Result<()> {
        self.anthropic_api_key = Some(api_key);
        self.save_with_provider(provider)?;
        info!("API key saved to config file");
        Ok(())
    }

    // =========================================================================
    // Convenience methods using default HomePathProvider
    // =========================================================================

    /// Loads configuration from `~/.abiogenesis/config.toml` with env var overrides.
    ///
    /// This is a convenience wrapper around [`load_with_provider`] using
    /// [`HomePathProvider`].
    ///
    /// # Configuration Precedence
    ///
    /// 1. `ANTHROPIC_API_KEY` environment variable (highest priority)
    /// 2. Config file (`~/.abiogenesis/config.toml`)
    /// 3. Default values (lowest priority)
    ///
    /// # Errors
    ///
    /// Returns an error only if the home directory cannot be determined.
    pub fn load() -> Result<Self> {
        Self::load_with_provider(&HomePathProvider)
    }

    /// Saves the configuration to `~/.abiogenesis/config.toml`.
    ///
    /// This is a convenience wrapper around [`save_with_provider`] using
    /// [`HomePathProvider`].
    pub fn save(&self) -> Result<()> {
        self.save_with_provider(&HomePathProvider)
    }

    /// Returns the configuration directory path (`~/.abiogenesis`).
    ///
    /// This is a convenience wrapper around [`get_config_dir_with_provider`]
    /// using [`HomePathProvider`].
    pub fn get_config_dir() -> Result<PathBuf> {
        Self::get_config_dir_with_provider(&HomePathProvider)
    }

    /// Sets the API key and saves to `~/.abiogenesis/config.toml`.
    ///
    /// This is a convenience wrapper around [`set_api_key_with_provider`]
    /// using [`HomePathProvider`].
    pub fn set_api_key(&mut self, api_key: String) -> Result<()> {
        self.set_api_key_with_provider(api_key, &HomePathProvider)
    }

    /// Returns the API key if configured.
    ///
    /// Note: This returns the key stored in the struct. If you need the
    /// environment variable to take precedence, use [`load`] first.
    pub fn get_api_key(&self) -> Option<&String> {
        self.anthropic_api_key.as_ref()
    }

    /// Displays configuration information to stdout.
    ///
    /// Shows:
    /// - Config file path and status
    /// - Whether API key is set
    /// - Log file location
    /// - Instructions for setting the API key
    pub fn show_config_info() -> Result<()> {
        Self::show_config_info_with_provider(&HomePathProvider)
    }

    /// Displays configuration information using the provided path provider.
    pub fn show_config_info_with_provider(provider: &dyn ConfigPathProvider) -> Result<()> {
        let config_path = Self::get_config_path_with_provider(provider)?;
        println!("Configuration file: {}", config_path.display());

        if config_path.exists() {
            println!("Status: Found");
            let config = Self::load_from_file_with_provider(provider)?;
            println!(
                "API Key: {}",
                if config.anthropic_api_key.is_some() {
                    "Set"
                } else {
                    "Not set"
                }
            );
        } else {
            println!("Status: Not found (using defaults)");
        }

        println!(
            "\nLog file: {}",
            Self::get_config_dir_with_provider(provider)?
                .join("ergo.log")
                .display()
        );

        println!("\nTo set API key:");
        println!("  ergo --set-api-key <your-key>");
        println!("\nOr set environment variable:");
        println!("  export ANTHROPIC_API_KEY=<your-key>");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// A path provider that uses a temporary directory for testing.
    struct TempPathProvider {
        base_dir: PathBuf,
    }

    impl TempPathProvider {
        fn new(temp_dir: &TempDir) -> Self {
            Self {
                base_dir: temp_dir.path().to_path_buf(),
            }
        }
    }

    impl ConfigPathProvider for TempPathProvider {
        fn get_base_dir(&self) -> Result<PathBuf> {
            Ok(self.base_dir.clone())
        }
    }

    // Mutex to prevent parallel tests from interfering with env vars
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    // =========================================================================
    // Pure function tests (no I/O)
    // =========================================================================

    #[test]
    fn test_default_config_has_no_api_key() {
        let config = Config::default();
        assert!(config.anthropic_api_key.is_none());
    }

    #[test]
    fn test_get_api_key_returns_none_when_not_set() {
        let config = Config::default();
        assert!(config.get_api_key().is_none());
    }

    #[test]
    fn test_get_api_key_returns_value_when_set() {
        let config = Config {
            anthropic_api_key: Some("test-key".to_string()),
        };
        assert_eq!(config.get_api_key(), Some(&"test-key".to_string()));
    }

    // =========================================================================
    // Serialization tests (no filesystem)
    // =========================================================================

    #[test]
    fn test_config_serializes_to_toml() {
        let config = Config {
            anthropic_api_key: Some("sk-ant-test123".to_string()),
        };

        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("anthropic_api_key"));
        assert!(toml_str.contains("sk-ant-test123"));
    }

    #[test]
    fn test_config_deserializes_from_toml() {
        let toml_str = r#"anthropic_api_key = "sk-ant-test456""#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.anthropic_api_key,
            Some("sk-ant-test456".to_string())
        );
    }

    #[test]
    fn test_config_deserializes_with_missing_api_key() {
        let toml_str = "";
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.anthropic_api_key.is_none());
    }

    #[test]
    fn test_config_roundtrip_serialization() {
        let original = Config {
            anthropic_api_key: Some("roundtrip-key".to_string()),
        };

        let toml_str = toml::to_string(&original).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(original, deserialized);
    }

    // =========================================================================
    // Filesystem tests (using temp directories)
    // =========================================================================

    #[test]
    fn test_save_creates_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let provider = TempPathProvider::new(&temp_dir);

        let config = Config {
            anthropic_api_key: Some("save-test-key".to_string()),
        };

        config.save_with_provider(&provider).unwrap();

        let config_path = temp_dir.path().join("config.toml");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("save-test-key"));
    }

    #[test]
    fn test_save_creates_parent_directory() {
        let temp_dir = TempDir::new().unwrap();
        // Use a nested path that doesn't exist yet
        let nested_path = temp_dir.path().join("nested").join("config");

        struct NestedProvider {
            path: PathBuf,
        }
        impl ConfigPathProvider for NestedProvider {
            fn get_base_dir(&self) -> Result<PathBuf> {
                Ok(self.path.clone())
            }
        }

        let provider = NestedProvider { path: nested_path.clone() };
        let config = Config::default();

        config.save_with_provider(&provider).unwrap();

        assert!(nested_path.join("config.toml").exists());
    }

    #[test]
    fn test_load_from_file_reads_existing_config() {
        let temp_dir = TempDir::new().unwrap();
        let provider = TempPathProvider::new(&temp_dir);

        // Manually write a config file
        let config_path = temp_dir.path().join("config.toml");
        fs::write(&config_path, r#"anthropic_api_key = "loaded-key""#).unwrap();

        let config = Config::load_from_file_with_provider(&provider).unwrap();
        assert_eq!(config.anthropic_api_key, Some("loaded-key".to_string()));
    }

    #[test]
    fn test_load_from_file_returns_error_when_missing() {
        let temp_dir = TempDir::new().unwrap();
        let provider = TempPathProvider::new(&temp_dir);

        let result = Config::load_from_file_with_provider(&provider);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_with_provider_uses_defaults_when_file_missing() {
        let temp_dir = TempDir::new().unwrap();
        let provider = TempPathProvider::new(&temp_dir);

        // Ensure no env var interference
        let _guard = ENV_MUTEX.lock().unwrap();
        // SAFETY: We hold a mutex to ensure no other test is accessing env vars concurrently
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }

        let config = Config::load_with_provider(&provider).unwrap();
        assert!(config.anthropic_api_key.is_none());
    }

    #[test]
    fn test_load_with_provider_env_var_overrides_file() {
        let temp_dir = TempDir::new().unwrap();
        let provider = TempPathProvider::new(&temp_dir);

        // Write config file with one key
        let config_path = temp_dir.path().join("config.toml");
        fs::write(&config_path, r#"anthropic_api_key = "file-key""#).unwrap();

        // Set env var with different key
        let _guard = ENV_MUTEX.lock().unwrap();
        // SAFETY: We hold a mutex to ensure no other test is accessing env vars concurrently
        unsafe {
            std::env::set_var("ANTHROPIC_API_KEY", "env-key");
        }

        let config = Config::load_with_provider(&provider).unwrap();

        // Clean up env var
        // SAFETY: We hold a mutex to ensure no other test is accessing env vars concurrently
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }

        // Env var should win
        assert_eq!(config.anthropic_api_key, Some("env-key".to_string()));
    }

    #[test]
    fn test_set_api_key_with_provider_saves_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let provider = TempPathProvider::new(&temp_dir);

        let mut config = Config::default();
        config
            .set_api_key_with_provider("new-api-key".to_string(), &provider)
            .unwrap();

        // Verify in-memory state
        assert_eq!(config.anthropic_api_key, Some("new-api-key".to_string()));

        // Verify persisted state
        let loaded = Config::load_from_file_with_provider(&provider).unwrap();
        assert_eq!(loaded.anthropic_api_key, Some("new-api-key".to_string()));
    }

    #[test]
    fn test_get_config_path_with_provider() {
        let temp_dir = TempDir::new().unwrap();
        let provider = TempPathProvider::new(&temp_dir);

        let path = Config::get_config_path_with_provider(&provider).unwrap();
        assert_eq!(path, temp_dir.path().join("config.toml"));
    }

    #[test]
    fn test_get_config_dir_with_provider() {
        let temp_dir = TempDir::new().unwrap();
        let provider = TempPathProvider::new(&temp_dir);

        let dir = Config::get_config_dir_with_provider(&provider).unwrap();
        assert_eq!(dir, temp_dir.path());
    }

    // =========================================================================
    // HomePathProvider tests
    // =========================================================================

    #[test]
    fn test_home_path_provider_returns_abiogenesis_dir() {
        let provider = HomePathProvider;
        let base_dir = provider.get_base_dir().unwrap();

        assert!(base_dir.ends_with(".abiogenesis"));
    }
}