use clap::{Arg, Command};
use tracing::info;

mod command_router;
mod llm_generator;
mod command_cache;
mod executor;

use command_router::CommandRouter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    let matches = Command::new("ergo")
        .about("AI-powered command interceptor - cogito, ergo sum")
        .long_about("ergo bridges intent (cogito) to execution (sum) by generating commands on the fly when they don't exist")
        .arg(Arg::new("intent")
            .help("The command or intent to execute")
            .required(true)
            .num_args(1..))
        .get_matches();
    
    let intent_args: Vec<String> = matches
        .get_many::<String>("intent")
        .unwrap_or_default()
        .map(|s| s.to_string())
        .collect();
    
    if intent_args.is_empty() {
        eprintln!("No intent provided");
        return Ok(());
    }
    
    info!("Processing intent: {:?}", intent_args);
    
    let mut router = CommandRouter::new().await?;
    router.process_intent(intent_args).await?;
    
    Ok(())
}
