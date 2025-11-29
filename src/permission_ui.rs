//! User interface for permission consent dialogs.
//!
//! This module provides the interactive UI for requesting user consent when
//! commands require special permissions (file access, network, etc.).

use crate::command_cache::{PermissionConsent, PermissionDecision};
use crate::llm_generator::PermissionRequest;
use crate::providers::{SystemTimeProvider, TimeProvider};
use anyhow::Result;
use std::io::{self, BufRead, Write};
use tracing::info;

/// Handles user interaction for permission consent dialogs.
///
/// `PermissionUI` displays permission requests to users and collects their
/// consent decisions. It supports three response types:
/// - Accept Once: Run the command this time, ask again next time
/// - Accept Forever: Always run with these permissions
/// - Deny: Don't run the command
///
/// # Example
///
/// ```no_run
/// use abiogenesis::permission_ui::PermissionUI;
/// use abiogenesis::llm_generator::PermissionRequest;
///
/// let ui = PermissionUI::new(true);
/// let permissions = vec![
///     PermissionRequest {
///         permission: "--allow-read".to_string(),
///         reason: "Read configuration files".to_string(),
///     },
/// ];
///
/// let consent = ui.prompt_for_consent("my-command", "Does something", &permissions)?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub struct PermissionUI {
    verbose: bool,
    time_provider: Box<dyn TimeProvider>,
}

impl PermissionUI {
    /// Creates a new `PermissionUI` instance.
    ///
    /// # Arguments
    ///
    /// * `verbose` - If true, shows additional output messages
    pub fn new(verbose: bool) -> Self {
        Self::with_time_provider(verbose, Box::new(SystemTimeProvider))
    }

    /// Creates a `PermissionUI` with a custom time provider (for testing).
    pub fn with_time_provider(verbose: bool, time_provider: Box<dyn TimeProvider>) -> Self {
        Self {
            verbose,
            time_provider,
        }
    }

    // =========================================================================
    // Core methods with I/O injection (testable)
    // =========================================================================

    /// Prompts the user for permission consent using custom I/O streams.
    ///
    /// This method enables testing by allowing injection of mock stdin/stdout.
    ///
    /// # Arguments
    ///
    /// * `command_name` - Name of the command requesting permissions
    /// * `command_description` - Human-readable description of the command
    /// * `permissions` - List of permissions the command requires
    /// * `input` - Reader to get user input from (e.g., stdin or mock)
    /// * `output` - Writer for displaying the prompt (e.g., stdout or mock)
    ///
    /// # Returns
    ///
    /// The user's consent decision, or auto-accepts if no permissions needed.
    ///
    /// # Errors
    ///
    /// Returns an error if I/O operations fail.
    pub fn prompt_for_consent_with_io<R: BufRead, W: Write>(
        &self,
        command_name: &str,
        command_description: &str,
        permissions: &[PermissionRequest],
        input: &mut R,
        output: &mut W,
    ) -> Result<PermissionConsent> {
        if permissions.is_empty() {
            // No permissions needed, auto-accept
            return Ok(PermissionConsent::AcceptForever);
        }

        self.display_permission_request_with_io(command_name, command_description, permissions, output)?;

        loop {
            write!(output, "\nChoose an option (1/2/3): ")?;
            output.flush()?;

            let mut line = String::new();
            input.read_line(&mut line)?;
            let choice = line.trim();

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
                    writeln!(output, "Invalid choice. Please enter 1, 2, or 3.")?;
                }
            }
        }
    }

    /// Displays the permission request dialog to the provided output.
    fn display_permission_request_with_io<W: Write>(
        &self,
        command_name: &str,
        command_description: &str,
        permissions: &[PermissionRequest],
        output: &mut W,
    ) -> Result<()> {
        writeln!(output, "\n{}", "=".repeat(60))?;
        writeln!(output, "🔐 PERMISSION REQUEST")?;
        writeln!(output, "{}", "=".repeat(60))?;
        writeln!(output)?;
        writeln!(output, "📋 Command: {}", command_name)?;
        writeln!(output, "📝 Description: {}", command_description)?;
        writeln!(output)?;

        if permissions.is_empty() {
            writeln!(output, "✅ This command requires no special permissions.")?;
        } else {
            writeln!(output, "🔑 This command requires the following permissions:")?;
            writeln!(output)?;

            for (i, perm) in permissions.iter().enumerate() {
                writeln!(output, "   {}. 🛡️ {}", i + 1, perm.permission)?;
                writeln!(output, "      💡 Why: {}", perm.reason)?;
                writeln!(output)?;
            }
        }

        writeln!(output, "{}", "-".repeat(60))?;
        writeln!(output, "What would you like to do?")?;
        writeln!(output)?;
        writeln!(output, "  1️⃣  Accept Once    - Run this time only, ask again next time")?;
        writeln!(output, "  2️⃣  Accept Forever - Always run with these permissions")?;
        writeln!(output, "  3️⃣  Deny          - Don't run this command")?;
        writeln!(output)?;
        writeln!(output, "{}", "=".repeat(60))?;

        Ok(())
    }

    /// Shows permission denied message to the provided output.
    ///
    /// # Arguments
    ///
    /// * `command_name` - Name of the denied command
    /// * `output` - Writer for the message
    pub fn show_permission_denied_with_io<W: Write>(
        &self,
        command_name: &str,
        output: &mut W,
    ) -> Result<()> {
        writeln!(output, "\n❌ Permission denied for command '{}'", command_name)?;
        writeln!(output, "   The command will not be executed.")?;
        Ok(())
    }

    /// Shows the "running with permissions" message to the provided output.
    ///
    /// # Arguments
    ///
    /// * `command_name` - Name of the command being run
    /// * `permissions` - List of permissions granted
    /// * `output` - Writer for the message
    pub fn show_running_with_permissions_with_io<W: Write>(
        &self,
        command_name: &str,
        permissions: &[PermissionRequest],
        output: &mut W,
    ) -> Result<()> {
        if self.verbose {
            if permissions.is_empty() {
                writeln!(output, "▶️  Running '{}' (no special permissions needed)", command_name)?;
            } else {
                writeln!(output, "▶️  Running '{}' with approved permissions:", command_name)?;
                for perm in permissions {
                    writeln!(output, "   🛡️  {}", perm.permission)?;
                }
            }
        } else if !permissions.is_empty() {
            // Always show when permissions are involved for security
            writeln!(output, "▶️  Running '{}' with permissions:", command_name)?;
            for perm in permissions {
                writeln!(output, "   🛡️  {}", perm.permission)?;
            }
        }
        Ok(())
    }

    // =========================================================================
    // Convenience methods using standard I/O
    // =========================================================================

    /// Prompts the user for permission consent using stdin/stdout.
    ///
    /// This is a convenience wrapper around [`Self::prompt_for_consent_with_io`].
    ///
    /// # Arguments
    ///
    /// * `command_name` - Name of the command requesting permissions
    /// * `command_description` - Human-readable description of the command
    /// * `permissions` - List of permissions the command requires
    ///
    /// # Returns
    ///
    /// The user's consent decision. If `permissions` is empty, automatically
    /// returns [`PermissionConsent::AcceptForever`].
    pub fn prompt_for_consent(
        &self,
        command_name: &str,
        command_description: &str,
        permissions: &[PermissionRequest],
    ) -> Result<PermissionConsent> {
        let stdin = io::stdin();
        let mut input = stdin.lock();
        let mut output = io::stdout();
        self.prompt_for_consent_with_io(command_name, command_description, permissions, &mut input, &mut output)
    }

    /// Creates a permission decision record.
    ///
    /// Creates a [`PermissionDecision`] with the current timestamp from
    /// the time provider.
    ///
    /// # Arguments
    ///
    /// * `permissions` - The permissions that were requested
    /// * `consent` - The user's consent decision
    ///
    /// # Returns
    ///
    /// A new [`PermissionDecision`] with the current Unix timestamp.
    pub fn create_permission_decision(
        &self,
        permissions: Vec<PermissionRequest>,
        consent: PermissionConsent,
    ) -> PermissionDecision {
        Self::create_permission_decision_with_timestamp(
            self,
            permissions,
            consent,
            self.time_provider.now(),
        )
    }

    /// Creates a permission decision with a specific timestamp.
    ///
    /// This method is useful for testing with deterministic timestamps.
    ///
    /// # Arguments
    ///
    /// * `permissions` - The permissions that were requested
    /// * `consent` - The user's consent decision
    /// * `timestamp` - Unix timestamp for when the decision was made
    pub fn create_permission_decision_with_timestamp(
        &self,
        permissions: Vec<PermissionRequest>,
        consent: PermissionConsent,
        timestamp: u64,
    ) -> PermissionDecision {
        PermissionDecision {
            permissions,
            consent,
            decided_at: timestamp,
        }
    }

    /// Shows permission denied message to stdout.
    ///
    /// This is a convenience wrapper around [`Self::show_permission_denied_with_io`].
    pub fn show_permission_denied(&self, command_name: &str) {
        let mut output = io::stdout();
        let _ = self.show_permission_denied_with_io(command_name, &mut output);
    }

    /// Shows the "running with permissions" message to stdout.
    ///
    /// This is a convenience wrapper around [`Self::show_running_with_permissions_with_io`].
    ///
    /// In verbose mode, always shows the message. In non-verbose mode, only shows
    /// the message when permissions are not empty (for security awareness).
    pub fn show_running_with_permissions(&self, command_name: &str, permissions: &[PermissionRequest]) {
        let mut output = io::stdout();
        let _ = self.show_running_with_permissions_with_io(command_name, permissions, &mut output);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Helper to create a test permission
    fn test_permission(name: &str, reason: &str) -> PermissionRequest {
        PermissionRequest {
            permission: name.to_string(),
            reason: reason.to_string(),
        }
    }

    // =========================================================================
    // Constructor tests
    // =========================================================================

    #[test]
    fn test_new_creates_instance_with_verbose_true() {
        let ui = PermissionUI::new(true);
        assert!(ui.verbose);
    }

    #[test]
    fn test_new_creates_instance_with_verbose_false() {
        let ui = PermissionUI::new(false);
        assert!(!ui.verbose);
    }

    // =========================================================================
    // prompt_for_consent_with_io tests
    // =========================================================================

    #[test]
    fn test_prompt_auto_accepts_when_no_permissions() {
        let ui = PermissionUI::new(false);
        let permissions: Vec<PermissionRequest> = vec![];

        let mut input = Cursor::new(b"");
        let mut output = Vec::new();

        let result = ui
            .prompt_for_consent_with_io("test-cmd", "Test command", &permissions, &mut input, &mut output)
            .unwrap();

        assert!(matches!(result, PermissionConsent::AcceptForever));
        // Should not have written anything since no prompt was needed
        assert!(output.is_empty());
    }

    #[test]
    fn test_prompt_returns_accept_once_for_input_1() {
        let ui = PermissionUI::new(false);
        let permissions = vec![test_permission("--allow-read", "Read files")];

        let mut input = Cursor::new(b"1\n");
        let mut output = Vec::new();

        let result = ui
            .prompt_for_consent_with_io("test-cmd", "Test command", &permissions, &mut input, &mut output)
            .unwrap();

        assert!(matches!(result, PermissionConsent::AcceptOnce));
    }

    #[test]
    fn test_prompt_returns_accept_forever_for_input_2() {
        let ui = PermissionUI::new(false);
        let permissions = vec![test_permission("--allow-net", "Network access")];

        let mut input = Cursor::new(b"2\n");
        let mut output = Vec::new();

        let result = ui
            .prompt_for_consent_with_io("test-cmd", "Test command", &permissions, &mut input, &mut output)
            .unwrap();

        assert!(matches!(result, PermissionConsent::AcceptForever));
    }

    #[test]
    fn test_prompt_returns_denied_for_input_3() {
        let ui = PermissionUI::new(false);
        let permissions = vec![test_permission("--allow-write", "Write files")];

        let mut input = Cursor::new(b"3\n");
        let mut output = Vec::new();

        let result = ui
            .prompt_for_consent_with_io("test-cmd", "Test command", &permissions, &mut input, &mut output)
            .unwrap();

        assert!(matches!(result, PermissionConsent::Denied));
    }

    #[test]
    fn test_prompt_retries_on_invalid_input() {
        let ui = PermissionUI::new(false);
        let permissions = vec![test_permission("--allow-read", "Read files")];

        // First invalid, then valid
        let mut input = Cursor::new(b"invalid\n2\n");
        let mut output = Vec::new();

        let result = ui
            .prompt_for_consent_with_io("test-cmd", "Test command", &permissions, &mut input, &mut output)
            .unwrap();

        assert!(matches!(result, PermissionConsent::AcceptForever));

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("Invalid choice"));
    }

    #[test]
    fn test_prompt_displays_permission_info() {
        let ui = PermissionUI::new(false);
        let permissions = vec![
            test_permission("--allow-read", "Read config files"),
            test_permission("--allow-net", "Call external API"),
        ];

        let mut input = Cursor::new(b"1\n");
        let mut output = Vec::new();

        ui.prompt_for_consent_with_io("my-command", "Does important stuff", &permissions, &mut input, &mut output)
            .unwrap();

        let output_str = String::from_utf8(output).unwrap();

        // Check command info is displayed
        assert!(output_str.contains("my-command"));
        assert!(output_str.contains("Does important stuff"));

        // Check permissions are displayed
        assert!(output_str.contains("--allow-read"));
        assert!(output_str.contains("Read config files"));
        assert!(output_str.contains("--allow-net"));
        assert!(output_str.contains("Call external API"));

        // Check options are displayed
        assert!(output_str.contains("Accept Once"));
        assert!(output_str.contains("Accept Forever"));
        assert!(output_str.contains("Deny"));
    }

    #[test]
    fn test_prompt_trims_whitespace_from_input() {
        let ui = PermissionUI::new(false);
        let permissions = vec![test_permission("--allow-read", "Read files")];

        // Input with extra whitespace
        let mut input = Cursor::new(b"  2  \n");
        let mut output = Vec::new();

        let result = ui
            .prompt_for_consent_with_io("test-cmd", "Test", &permissions, &mut input, &mut output)
            .unwrap();

        assert!(matches!(result, PermissionConsent::AcceptForever));
    }

    // =========================================================================
    // create_permission_decision tests
    // =========================================================================

    #[test]
    fn test_create_permission_decision_with_timestamp() {
        let ui = PermissionUI::new(false);
        let permissions = vec![test_permission("--allow-read", "Read files")];
        let timestamp = 1234567890u64;

        let decision = ui.create_permission_decision_with_timestamp(
            permissions.clone(),
            PermissionConsent::AcceptOnce,
            timestamp,
        );

        assert_eq!(decision.decided_at, timestamp);
        assert!(matches!(decision.consent, PermissionConsent::AcceptOnce));
        assert_eq!(decision.permissions.len(), 1);
        assert_eq!(decision.permissions[0].permission, "--allow-read");
    }

    #[test]
    fn test_create_permission_decision_uses_injected_time_provider() {
        use crate::providers::TimeProvider;

        struct MockTime;
        impl TimeProvider for MockTime {
            fn now(&self) -> u64 {
                42
            }
        }

        let ui = PermissionUI::with_time_provider(false, Box::new(MockTime));
        let permissions = vec![];

        let decision = ui.create_permission_decision(permissions, PermissionConsent::AcceptOnce);

        assert_eq!(decision.decided_at, 42);
    }

    // =========================================================================
    // show_permission_denied_with_io tests
    // =========================================================================

    #[test]
    fn test_show_permission_denied_displays_command_name() {
        let ui = PermissionUI::new(false);
        let mut output = Vec::new();

        ui.show_permission_denied_with_io("dangerous-cmd", &mut output).unwrap();

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("dangerous-cmd"));
        assert!(output_str.contains("denied"));
    }

    // =========================================================================
    // show_running_with_permissions_with_io tests
    // =========================================================================

    #[test]
    fn test_show_running_verbose_with_permissions() {
        let ui = PermissionUI::new(true); // verbose
        let permissions = vec![test_permission("--allow-read", "Read files")];
        let mut output = Vec::new();

        ui.show_running_with_permissions_with_io("my-cmd", &permissions, &mut output).unwrap();

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("my-cmd"));
        assert!(output_str.contains("--allow-read"));
    }

    #[test]
    fn test_show_running_verbose_no_permissions() {
        let ui = PermissionUI::new(true); // verbose
        let permissions: Vec<PermissionRequest> = vec![];
        let mut output = Vec::new();

        ui.show_running_with_permissions_with_io("my-cmd", &permissions, &mut output).unwrap();

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("my-cmd"));
        assert!(output_str.contains("no special permissions"));
    }

    #[test]
    fn test_show_running_non_verbose_with_permissions() {
        let ui = PermissionUI::new(false); // non-verbose
        let permissions = vec![test_permission("--allow-net", "Network")];
        let mut output = Vec::new();

        ui.show_running_with_permissions_with_io("my-cmd", &permissions, &mut output).unwrap();

        let output_str = String::from_utf8(output).unwrap();
        // Should still show when permissions are involved (security)
        assert!(output_str.contains("my-cmd"));
        assert!(output_str.contains("--allow-net"));
    }

    #[test]
    fn test_show_running_non_verbose_no_permissions_is_silent() {
        let ui = PermissionUI::new(false); // non-verbose
        let permissions: Vec<PermissionRequest> = vec![];
        let mut output = Vec::new();

        ui.show_running_with_permissions_with_io("my-cmd", &permissions, &mut output).unwrap();

        // Should be silent when non-verbose and no permissions
        assert!(output.is_empty());
    }
}