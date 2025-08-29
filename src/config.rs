use anyhow::{anyhow, Result};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub anthropic_api_key: Option<String>,
    #[serde(default)]
    pub use_mock: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            anthropic_api_key: None,
            use_mock: false,
        }
    }
}

impl Config {
    /// Load configuration from file, environment variables, or create default
    pub fn load() -> Result<Self> {
        let mut config = Self::load_from_file().unwrap_or_else(|_| {
            info!("No config file found, using defaults");
            Self::default()
        });

        // Environment variables override config file
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            config.anthropic_api_key = Some(api_key);
        }

        if std::env::var("ABIOGENESIS_USE_MOCK").is_ok() {
            config.use_mock = true;
        }

        Ok(config)
    }

    fn load_from_file() -> Result<Self> {
        let config_path = Self::get_config_path()?;
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            info!("Loaded config from: {}", config_path.display());
            Ok(config)
        } else {
            Err(anyhow!("Config file not found"))
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path()?;
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&config_path, content)?;
        info!("Saved config to: {}", config_path.display());
        Ok(())
    }

    fn get_config_path() -> Result<PathBuf> {
        let home = home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        Ok(home.join(".abiogenesis").join("config.toml"))
    }

    pub fn get_config_dir() -> Result<PathBuf> {
        let home = home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        Ok(home.join(".abiogenesis"))
    }

    /// Set API key and save config
    pub fn set_api_key(&mut self, api_key: String) -> Result<()> {
        self.anthropic_api_key = Some(api_key);
        self.save()?;
        info!("API key saved to config file");
        Ok(())
    }

    /// Get API key from config or environment
    pub fn get_api_key(&self) -> Option<&String> {
        self.anthropic_api_key.as_ref()
    }

    pub fn is_mock_mode(&self) -> bool {
        self.use_mock
    }

    pub fn show_config_info() -> Result<()> {
        let config_path = Self::get_config_path()?;
        println!("Configuration file: {}", config_path.display());
        
        if config_path.exists() {
            println!("Status: Found");
            let config = Self::load_from_file()?;
            println!("API Key: {}", if config.anthropic_api_key.is_some() { "Set" } else { "Not set" });
            println!("Mock mode: {}", config.use_mock);
        } else {
            println!("Status: Not found (using defaults)");
        }

        println!("\nTo set API key:");
        println!("  ergo --set-api-key <your-key>");
        println!("\nOr set environment variable:");
        println!("  export ANTHROPIC_API_KEY=<your-key>");
        
        Ok(())
    }
}