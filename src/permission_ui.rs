use crate::command_cache::{PermissionConsent, PermissionDecision};
use crate::llm_generator::PermissionRequest;
use anyhow::Result;
use std::io::{self, Write};
use tracing::info;

pub struct PermissionUI {
    verbose: bool,
}

impl PermissionUI {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }

    pub fn prompt_for_consent(
        &self,
        command_name: &str,
        command_description: &str,
        permissions: &[PermissionRequest],
    ) -> Result<PermissionConsent> {
        if permissions.is_empty() {
            // No permissions needed, auto-accept
            return Ok(PermissionConsent::AcceptForever);
        }

        self.display_permission_request(command_name, command_description, permissions)?;
        
        loop {
            print!("\nChoose an option (1/2/3): ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let choice = input.trim();

            match choice {
                "1" => {
                    info!("User chose 'Accept Once' for command '{}'", command_name);
                    return Ok(PermissionConsent::AcceptOnce);
                }
                "2" => {
                    info!("User chose 'Accept Forever' for command '{}'", command_name);
                    return Ok(PermissionConsent::AcceptForever);
                }
                "3" => {
                    info!("User chose 'Deny' for command '{}'", command_name);
                    return Ok(PermissionConsent::Denied);
                }
                _ => {
                    println!("Invalid choice. Please enter 1, 2, or 3.");
                }
            }
        }
    }

    fn display_permission_request(
        &self,
        command_name: &str,
        command_description: &str,
        permissions: &[PermissionRequest],
    ) -> Result<()> {
        println!("\n{}", "=".repeat(60));
        println!("🔐 PERMISSION REQUEST");
        println!("{}", "=".repeat(60));
        println!();
        println!("📋 Command: {}", command_name);
        println!("📝 Description: {}", command_description);
        println!();
        
        if permissions.is_empty() {
            println!("✅ This command requires no special permissions.");
        } else {
            println!("🔑 This command requires the following permissions:");
            println!();
            
            for (i, perm) in permissions.iter().enumerate() {
                println!("   {}. 🛡️ {}", i + 1, perm.permission);
                println!("      💡 Why: {}", perm.reason);
                println!();
            }
        }

        println!("{}", "-".repeat(60));
        println!("What would you like to do?");
        println!();
        println!("  1️⃣  Accept Once    - Run this time only, ask again next time");
        println!("  2️⃣  Accept Forever - Always run with these permissions");  
        println!("  3️⃣  Deny          - Don't run this command");
        println!();
        println!("{}", "=".repeat(60));

        Ok(())
    }

    pub fn create_permission_decision(
        &self,
        permissions: Vec<PermissionRequest>,
        consent: PermissionConsent,
    ) -> PermissionDecision {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        PermissionDecision {
            permissions,
            consent,
            decided_at: now,
        }
    }

    pub fn show_permission_denied(&self, command_name: &str) {
        println!("\n❌ Permission denied for command '{}'", command_name);
        println!("   The command will not be executed.");
    }

    pub fn show_running_with_permissions(&self, command_name: &str, permissions: &[PermissionRequest]) {
        if self.verbose {
            if permissions.is_empty() {
                println!("▶️  Running '{}' (no special permissions needed)", command_name);
            } else {
                println!("▶️  Running '{}' with approved permissions:", command_name);
                for perm in permissions {
                    println!("   🛡️  {}", perm.permission);
                }
            }
        } else if !permissions.is_empty() {
            // Always show when permissions are involved for security
            println!("▶️  Running '{}' with permissions:", command_name);
            for perm in permissions {
                println!("   🛡️  {}", perm.permission);
            }
        }
    }
}