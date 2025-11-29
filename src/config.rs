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

/// Application configuration data for ergo.
///
/// This is a simple data struct that holds configuration values. It is
/// serializable to TOML format and can be cloned and compared.
///
/// For loading, saving, and managing configuration, use [`ConfigLoader`].
///
/// # Configuration Precedence
///
/// When loaded via [`ConfigLoader`]:
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

/// Handles loading, saving, and managing configuration files.
///
/// Uses constructor injection for the path provider, enabling testability
/// with mock directories.
///
/// # Example
///
/// ```no_run
/// use abiogenesis::config::{ConfigLoader, HomePathProvider};
///
/// // Production usage with default provider
/// let loader = ConfigLoader::new();
/// let config = loader.load()?;
///
/// // Or with a custom provider for testing
/// let loader = ConfigLoader::with_provider(Box::new(HomePathProvider));
/// # Ok::<(), anyhow::Error>(())
/// ```
pub struct ConfigLoader {
    path_provider: Box<dyn ConfigPathProvider>,
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigLoader {
    /// Creates a new ConfigLoader with the default [`HomePathProvider`].
    pub fn new() -> Self {
        Self::with_provider(Box::new(HomePathProvider))
    }

    /// Creates a new ConfigLoader with a custom path provider.
    ///
    /// This is primarily useful for testing with temporary directories.
    ///
    /// # Arguments
    ///
    /// * `path_provider` - The provider that determines where config files are stored
    pub fn with_provider(path_provider: Box<dyn ConfigPathProvider>) -> Self {
        Self { path_provider }
    }

    /// Loads configuration from the config file only (no env var overrides).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The config file doesn't exist
    /// - The config file cannot be read
    /// - The config file contains invalid TOML
    pub fn load_from_file(&self) -> Result<Config> {
        let config_path = self.get_config_path()?;
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            info!("Loaded config from: {}", config_path.display());
            Ok(config)
        } else {
            Err(anyhow!("Config file not found"))
        }
    }

    /// Loads configuration with full precedence rules.
    ///
    /// # Configuration Precedence
    ///
    /// 1. Environment variables (highest priority)
    /// 2. Config file
    /// 3. Default values (lowest priority)
    ///
    /// # Errors
    ///
    /// Returns an error only if the path provider fails. Missing config files
    /// are handled gracefully by using defaults.
    pub fn load(&self) -> Result<Config> {
        let mut config = self.load_from_file().unwrap_or_else(|_| {
            info!("No config file found, using defaults");
            Config::default()
        });

        // Environment variables override config file
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            config.anthropic_api_key = Some(api_key);
        }

        Ok(config)
    }

    /// Saves the configuration to disk.
    ///
    /// Creates the parent directory if it doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to save
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The parent directory cannot be created
    /// - The file cannot be written
    pub fn save(&self, config: &Config) -> Result<()> {
        let config_path = self.get_config_path()?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(config)?;
        fs::write(&config_path, content)?;
        info!("Saved config to: {}", config_path.display());
        Ok(())
    }

    /// Returns the full path to the config file.
    pub fn get_config_path(&self) -> Result<PathBuf> {
        Ok(self.path_provider.get_base_dir()?.join("config.toml"))
    }

    /// Returns the configuration directory path.
    pub fn get_config_dir(&self) -> Result<PathBuf> {
        self.path_provider.get_base_dir()
    }

    /// Sets the API key and saves the configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to update
    /// * `api_key` - The Anthropic API key to store
    ///
    /// # Errors
    ///
    /// Returns an error if saving fails.
    pub fn set_api_key(&self, config: &mut Config, api_key: String) -> Result<()> {
        config.anthropic_api_key = Some(api_key);
        self.save(config)?;
        info!("API key saved to config file");
        Ok(())
    }

    /// Displays configuration information to stdout.
    ///
    /// This is a convenience wrapper around [`show_config_info_with_io`].
    pub fn show_config_info(&self) -> Result<()> {
        self.show_config_info_with_io(&mut std::io::stdout())
    }

    /// Displays configuration information to the provided writer.
    ///
    /// Shows:
    /// - Config file path and status
    /// - Whether API key is set
    /// - Log file location
    /// - Instructions for setting the API key
    ///
    /// # Arguments
    ///
    /// * `output` - Writer to output configuration information to
    pub fn show_config_info_with_io<W: std::io::Write>(&self, output: &mut W) -> Result<()> {
        let config_path = self.get_config_path()?;
        writeln!(output, "Configuration file: {}", config_path.display())?;

        if config_path.exists() {
            writeln!(output, "Status: Found")?;
            let config = self.load_from_file()?;
            writeln!(
                output,
                "API Key: {}",
                if config.anthropic_api_key.is_some() {
                    "Set"
                } else {
                    "Not set"
                }
            )?;
        } else {
            writeln!(output, "Status: Not found (using defaults)")?;
        }

        writeln!(
            output,
            "\nLog file: {}",
            self.get_config_dir()?.join("ergo.log").display()
        )?;

        writeln!(output, "\nTo set API key:")?;
        writeln!(output, "  ergo --set-api-key <your-key>")?;
        writeln!(output, "\nOr set environment variable:")?;
        writeln!(output, "  export ANTHROPIC_API_KEY=<your-key>")?;

        Ok(())
    }
}


impl Config {
    // =========================================================================
    // Convenience methods using default ConfigLoader
    // =========================================================================

    /// Loads configuration from `~/.abiogenesis/config.toml` with env var overrides.
    ///
    /// This is a convenience wrapper that creates a default [`ConfigLoader`]
    /// and calls [`ConfigLoader::load`].
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
        ConfigLoader::new().load()
    }

    /// Returns the configuration directory path (`~/.abiogenesis`).
    ///
    /// This is a convenience wrapper that creates a default [`ConfigLoader`]
    /// and calls [`ConfigLoader::get_config_dir`].
    pub fn get_config_dir() -> Result<PathBuf> {
        ConfigLoader::new().get_config_dir()
    }

    /// Sets the API key and saves to `~/.abiogenesis/config.toml`.
    ///
    /// This is a convenience wrapper that creates a default [`ConfigLoader`]
    /// and calls [`ConfigLoader::set_api_key`].
    pub fn set_api_key(&mut self, api_key: String) -> Result<()> {
        ConfigLoader::new().set_api_key(self, api_key)
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
        ConfigLoader::new().show_config_info()
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
    // ConfigLoader tests (using temp directories)
    // =========================================================================

    #[test]
    fn test_config_loader_save_creates_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let loader = ConfigLoader::with_provider(Box::new(TempPathProvider::new(&temp_dir)));

        let config = Config {
            anthropic_api_key: Some("save-test-key".to_string()),
        };

        loader.save(&config).unwrap();

        let config_path = temp_dir.path().join("config.toml");
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("save-test-key"));
    }

    #[test]
    fn test_config_loader_save_creates_parent_directory() {
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

        let loader = ConfigLoader::with_provider(Box::new(NestedProvider {
            path: nested_path.clone(),
        }));
        let config = Config::default();

        loader.save(&config).unwrap();

        assert!(nested_path.join("config.toml").exists());
    }

    #[test]
    fn test_config_loader_load_from_file_reads_existing_config() {
        let temp_dir = TempDir::new().unwrap();
        let loader = ConfigLoader::with_provider(Box::new(TempPathProvider::new(&temp_dir)));

        // Manually write a config file
        let config_path = temp_dir.path().join("config.toml");
        fs::write(&config_path, r#"anthropic_api_key = "loaded-key""#).unwrap();

        let config = loader.load_from_file().unwrap();
        assert_eq!(config.anthropic_api_key, Some("loaded-key".to_string()));
    }

    #[test]
    fn test_config_loader_load_from_file_returns_error_when_missing() {
        let temp_dir = TempDir::new().unwrap();
        let loader = ConfigLoader::with_provider(Box::new(TempPathProvider::new(&temp_dir)));

        let result = loader.load_from_file();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_loader_load_uses_defaults_when_file_missing() {
        let temp_dir = TempDir::new().unwrap();
        let loader = ConfigLoader::with_provider(Box::new(TempPathProvider::new(&temp_dir)));

        // Ensure no env var interference
        let _guard = ENV_MUTEX.lock().unwrap();
        // SAFETY: We hold a mutex to ensure no other test is accessing env vars concurrently
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }

        let config = loader.load().unwrap();
        assert!(config.anthropic_api_key.is_none());
    }

    #[test]
    fn test_config_loader_load_env_var_overrides_file() {
        let temp_dir = TempDir::new().unwrap();
        let loader = ConfigLoader::with_provider(Box::new(TempPathProvider::new(&temp_dir)));

        // Write config file with one key
        let config_path = temp_dir.path().join("config.toml");
        fs::write(&config_path, r#"anthropic_api_key = "file-key""#).unwrap();

        // Set env var with different key
        let _guard = ENV_MUTEX.lock().unwrap();
        // SAFETY: We hold a mutex to ensure no other test is accessing env vars concurrently
        unsafe {
            std::env::set_var("ANTHROPIC_API_KEY", "env-key");
        }

        let config = loader.load().unwrap();

        // Clean up env var
        // SAFETY: We hold a mutex to ensure no other test is accessing env vars concurrently
        unsafe {
            std::env::remove_var("ANTHROPIC_API_KEY");
        }

        // Env var should win
        assert_eq!(config.anthropic_api_key, Some("env-key".to_string()));
    }

    #[test]
    fn test_config_loader_set_api_key_saves_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let loader = ConfigLoader::with_provider(Box::new(TempPathProvider::new(&temp_dir)));

        let mut config = Config::default();
        loader.set_api_key(&mut config, "new-api-key".to_string()).unwrap();

        // Verify in-memory state
        assert_eq!(config.anthropic_api_key, Some("new-api-key".to_string()));

        // Verify persisted state
        let loaded = loader.load_from_file().unwrap();
        assert_eq!(loaded.anthropic_api_key, Some("new-api-key".to_string()));
    }

    #[test]
    fn test_config_loader_get_config_path() {
        let temp_dir = TempDir::new().unwrap();
        let loader = ConfigLoader::with_provider(Box::new(TempPathProvider::new(&temp_dir)));

        let path = loader.get_config_path().unwrap();
        assert_eq!(path, temp_dir.path().join("config.toml"));
    }

    #[test]
    fn test_config_loader_get_config_dir() {
        let temp_dir = TempDir::new().unwrap();
        let loader = ConfigLoader::with_provider(Box::new(TempPathProvider::new(&temp_dir)));

        let dir = loader.get_config_dir().unwrap();
        assert_eq!(dir, temp_dir.path());
    }

    #[test]
    fn test_config_loader_default() {
        let loader = ConfigLoader::default();
        // Just verify it doesn't panic and returns a valid path
        let dir = loader.get_config_dir().unwrap();
        assert!(dir.ends_with(".abiogenesis"));
    }

    // =========================================================================
    // show_config_info_with_io tests
    // =========================================================================

    #[test]
    fn test_show_config_info_when_config_file_missing() {
        let temp_dir = TempDir::new().unwrap();
        let loader = ConfigLoader::with_provider(Box::new(TempPathProvider::new(&temp_dir)));
        let mut output = Vec::new();

        loader.show_config_info_with_io(&mut output).unwrap();

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("Configuration file:"));
        assert!(output_str.contains("Status: Not found (using defaults)"));
        assert!(output_str.contains("Log file:"));
        assert!(output_str.contains("ergo --set-api-key"));
    }

    #[test]
    fn test_show_config_info_when_config_file_exists_with_api_key() {
        let temp_dir = TempDir::new().unwrap();
        let loader = ConfigLoader::with_provider(Box::new(TempPathProvider::new(&temp_dir)));

        // Create config file with API key
        let config_path = temp_dir.path().join("config.toml");
        fs::write(&config_path, r#"anthropic_api_key = "test-key""#).unwrap();

        let mut output = Vec::new();
        loader.show_config_info_with_io(&mut output).unwrap();

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("Status: Found"));
        assert!(output_str.contains("API Key: Set"));
    }

    #[test]
    fn test_show_config_info_when_config_file_exists_without_api_key() {
        let temp_dir = TempDir::new().unwrap();
        let loader = ConfigLoader::with_provider(Box::new(TempPathProvider::new(&temp_dir)));

        // Create config file without API key
        let config_path = temp_dir.path().join("config.toml");
        fs::write(&config_path, "").unwrap();

        let mut output = Vec::new();
        loader.show_config_info_with_io(&mut output).unwrap();

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("Status: Found"));
        assert!(output_str.contains("API Key: Not set"));
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