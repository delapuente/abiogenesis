use clap::{Arg, Command};
use tracing::info;

mod command_router;
mod llm_generator;
mod command_cache;
mod executor;
mod config;

use command_router::CommandRouter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    let matches = Command::new("ergo")
        .about("AI-powered command interceptor - cogito, ergo sum")
        .long_about("ergo bridges intent (cogito) to execution (sum) by generating commands on the fly when they don't exist")
        .arg(Arg::new("intent")
            .help("The command or intent to execute")
            .num_args(1..))
        .arg(Arg::new("set-api-key")
            .long("set-api-key")
            .help("Set the Anthropic API key")
            .value_name("API_KEY")
            .num_args(1))
        .arg(Arg::new("config")
            .long("config")
            .help("Show configuration information")
            .action(clap::ArgAction::SetTrue))
        .get_matches();
    
    // Handle configuration commands
    if let Some(api_key) = matches.get_one::<String>("set-api-key") {
        let mut config = config::Config::load()?;
        config.set_api_key(api_key.clone())?;
        println!("✅ API key saved successfully");
        return Ok(());
    }

    if matches.get_flag("config") {
        config::Config::show_config_info()?;
        return Ok(());
    }

    // Handle normal command execution
    let intent_args: Vec<String> = matches
        .get_many::<String>("intent")
        .unwrap_or_default()
        .map(|s| s.to_string())
        .collect();
    
    if intent_args.is_empty() {
        eprintln!("No intent provided. Use 'ergo --help' for usage information.");
        return Ok(());
    }
    
    info!("Processing intent: {:?}", intent_args);
    
    let mut router = CommandRouter::new().await?;
    router.process_intent(intent_args).await?;
    
    Ok(())
}
