use abiogenesis::command_cache::{CommandCache, PermissionConsent};
use abiogenesis::command_router::CommandRouter;
use abiogenesis::config::Config;
use abiogenesis::execution_context::ExecutionContext;
use abiogenesis::executor::Executor;
use abiogenesis::llm_generator::LlmGenerator;
use abiogenesis::permission_ui::PermissionUI;
use clap::{Arg, Command};
use std::fs::OpenOptions;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

fn setup_logging(verbose: bool) -> anyhow::Result<()> {
    // Get log directory from config
    let config_dir = Config::get_config_dir().unwrap_or_else(|_| {
        dirs::home_dir().unwrap_or_default().join(".abiogenesis")
    });
    
    // Create log directory if it doesn't exist
    std::fs::create_dir_all(&config_dir)?;
    
    let log_file = config_dir.join("ergo.log");
    
    // Create or open log file
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)?;
    
    // Set log level based on verbosity
    let log_level = if verbose { "debug" } else { "info" };
    
    // Configure tracing to write to file
    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(EnvFilter::from_default_env().add_directive(log_level.parse()?))
        .with_writer(file)
        .with_ansi(false) // No colors in log file
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)?;
    
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    
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
        .arg(Arg::new("clear-cache")
            .long("clear-cache")
            .help("Clear the command cache")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("list-cache")
            .long("list-cache")
            .help("List cached commands and their permissions")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("remove-command")
            .long("remove-command")
            .help("Remove a specific command from cache")
            .value_name("COMMAND_NAME")
            .num_args(1))
        .arg(Arg::new("cache-stats")
            .long("cache-stats")
            .help("Show cache statistics")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("verbose")
            .short('v')
            .long("verbose")
            .help("Enable verbose output")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("nope")
            .short('n')
            .long("nope")
            .help("Regenerate the last command (uses stderr as context if no feedback provided)")
            .value_name("FEEDBACK")
            .num_args(0..=1)
            .default_missing_value(""))
        .get_matches();
    
    // Setup logging early, but after parsing verbose flag
    let verbose = matches.get_flag("verbose");
    setup_logging(verbose)?;
    
    // Handle configuration commands
    if let Some(api_key) = matches.get_one::<String>("set-api-key") {
        let mut config = Config::load()?;
        config.set_api_key(api_key.clone())?;
        println!("✅ API key saved successfully");
        return Ok(());
    }

    if matches.get_flag("config") {
        Config::show_config_info()?;
        return Ok(());
    }

    // Handle cache management commands
    if matches.get_flag("clear-cache") {
        let mut cache = CommandCache::new().await?;
        cache.clear_cache().await?;
        println!("✅ Cache cleared successfully");
        return Ok(());
    }

    if matches.get_flag("list-cache") {
        let cache = CommandCache::new().await?;
        let commands = cache.list_commands().await;
        if commands.is_empty() {
            println!("📭 No commands in cache");
        } else {
            println!("📋 Cached Commands:");
            println!("{}", "=".repeat(50));
            for (name, command, decision) in commands {
                println!("🔧 {}", name);
                println!("   📝 {}", command.description);
                if !command.permissions.is_empty() {
                    println!("   🔑 Permissions:");
                    for perm in &command.permissions {
                        println!("      🛡️  {} - {}", perm.permission, perm.reason);
                    }
                }
                if let Some(decision) = decision {
                    let consent_str = match decision.consent {
                        PermissionConsent::AcceptOnce => "Accept Once",
                        PermissionConsent::AcceptForever => "Accept Forever",
                        PermissionConsent::Denied => "Denied",
                    };
                    println!("   ✅ User Decision: {}", consent_str);
                }
                println!();
            }
        }
        return Ok(());
    }

    if let Some(command_name) = matches.get_one::<String>("remove-command") {
        let mut cache = CommandCache::new().await?;
        if cache.remove_command(command_name).await? {
            println!("✅ Removed command '{}' from cache", command_name);
        } else {
            println!("❌ Command '{}' not found in cache", command_name);
        }
        return Ok(());
    }

    if matches.get_flag("cache-stats") {
        let cache = CommandCache::new().await?;
        let stats = cache.get_stats().await?;
        println!("{}", stats);
        return Ok(());
    }

    // Handle --nope feedback loop
    if let Some(feedback) = matches.get_one::<String>("nope") {
        return handle_nope_feedback(feedback, verbose).await;
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

    let mut router = CommandRouter::new(verbose).await?;
    router.process_intent(intent_args).await?;

    Ok(())
}

/// Handles the --nope feedback loop to regenerate a command.
async fn handle_nope_feedback(feedback: &str, verbose: bool) -> anyhow::Result<()> {
    // Load the last execution context
    let context = match ExecutionContext::load()? {
        Some(ctx) => ctx,
        None => {
            eprintln!("No previous command execution found. Run a command first, then use --nope.");
            return Ok(());
        }
    };

    if verbose {
        println!("🔄 Regenerating command '{}' with feedback...", context.command_name);
        println!("💭 Feedback: {}", feedback);
    }

    info!("Regenerating command '{}' with feedback: {}", context.command_name, feedback);

    // Regenerate the command with feedback
    let generator = LlmGenerator::new();
    let generation_result = generator
        .regenerate_command_with_feedback(
            &context.command_name,
            &context.script_content,
            context.stderr.as_deref(),
            feedback,
        )
        .await?;

    if verbose {
        println!("✨ Command regenerated successfully!");
        println!("📝 New description: {}", generation_result.command.description);
    }

    // Update the command in cache
    let mut cache = CommandCache::new().await?;
    cache
        .store_command(
            &context.command_name,
            &generation_result.command,
            &generation_result.script_content,
        )
        .await?;

    // Reset permission decision since the command changed
    // (User should re-approve the new version)

    // Check permissions and execute
    let permission_ui = PermissionUI::new(verbose);
    let consent = permission_ui.prompt_for_consent(
        &context.command_name,
        &generation_result.command.description,
        &generation_result.command.permissions,
    )?;

    let decision = permission_ui.create_permission_decision(
        generation_result.command.permissions.clone(),
        consent,
    );

    cache
        .set_permission_decision(&context.command_name, decision.clone())
        .await?;

    match decision.consent {
        PermissionConsent::AcceptOnce | PermissionConsent::AcceptForever => {
            permission_ui.show_running_with_permissions(
                &context.command_name,
                &generation_result.command.permissions,
            );
            cache.update_usage(&context.command_name).await?;

            // Execute the regenerated command and save context
            let executor = Executor::new(verbose);
            let _result = executor
                .execute_generated_command_with_context(&generation_result.command, &cache, &[])
                .await;
        }
        PermissionConsent::Denied => {
            permission_ui.show_permission_denied(&context.command_name);
        }
    }

    Ok(())
}
