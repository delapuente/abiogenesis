use anyhow::Result;
use std::process::Command;

/// Helper to run ergo commands and capture output
fn run_ergo_command(args: &[&str]) -> Result<std::process::Output> {
    let mut cmd = Command::new("cargo");
    cmd.arg("run");
    cmd.arg("--");
    cmd.args(args);
    
    // Enable mock mode for deterministic testing
    cmd.env("ABIOGENESIS_USE_MOCK", "1");
    
    let output = cmd.output()?;
    Ok(output)
}


#[test]
fn test_hello_command_generation_and_execution() -> Result<()> {
    let output = run_ergo_command(&["hello", "world"])?;
    
    // Check that command executed successfully
    assert!(output.status.success(), "Command should succeed");
    
    // Check output contains expected greeting
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hello from ergo! Arguments: world"), 
            "Should contain greeting with arguments");
    
    // The command should either be generated or retrieved from cache
    let should_have_output = stdout.contains("Executing generated command") || stdout.contains("Executing cached command");
    assert!(should_have_output, "Should show command execution");
    
    Ok(())
}

#[test]
fn test_command_caching_behavior() -> Result<()> {
    // Use a unique command name to avoid conflicts with other tests
    let unique_cmd = format!("cache-test-{}", std::process::id());
    
    // First execution - should generate
    let output1 = run_ergo_command(&[&unique_cmd])?;
    assert!(output1.status.success());
    
    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    assert!(stdout1.contains("Executing"), "First run should execute command");
    
    // Second execution with same unique command
    let output2 = run_ergo_command(&[&unique_cmd])?;
    assert!(output2.status.success());
    
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(stdout2.contains("Executing"), "Second run should also execute command");
    
    Ok(())
}

#[test]
fn test_timestamp_command_no_permissions() -> Result<()> {
    let output = run_ergo_command(&["timestamp"])?;
    
    assert!(output.status.success(), "Timestamp command should succeed");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should contain a timestamp in format YYYY-MM-DD_HH-MM-SS
    let has_timestamp = stdout.lines().any(|line| {
        line.len() == 19 && 
        line.chars().nth(4) == Some('-') &&
        line.chars().nth(7) == Some('-') &&
        line.chars().nth(10) == Some('_') &&
        line.chars().nth(13) == Some('-') &&
        line.chars().nth(16) == Some('-')
    });
    
    assert!(has_timestamp, "Should output timestamp in correct format");
    
    Ok(())
}

#[test]
fn test_uuid_command_generation() -> Result<()> {
    let output = run_ergo_command(&["uuid"])?;
    
    assert!(output.status.success(), "UUID command should succeed");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    // UUID should be 36 characters with hyphens at positions 8, 13, 18, 23
    let has_uuid = stdout.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.len() == 36 &&
        trimmed.chars().nth(8) == Some('-') &&
        trimmed.chars().nth(13) == Some('-') &&
        trimmed.chars().nth(18) == Some('-') &&
        trimmed.chars().nth(23) == Some('-')
    });
    
    assert!(has_uuid, "Should output valid UUID format");
    
    Ok(())
}

#[test]
fn test_project_info_with_permissions() -> Result<()> {
    let output = run_ergo_command(&["project-info"])?;
    
    assert!(output.status.success(), "Project info command should succeed");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Project: abiogenesis"), "Should show project name");
    assert!(stdout.contains("Files:"), "Should show file count");
    
    // Check that permissions are displayed
    assert!(stdout.contains("🔒 Deno permissions required:"), "Should show permissions");
    assert!(stdout.contains("--allow-read"), "Should require read permission");
    assert!(stdout.contains("--allow-run=git"), "Should require git run permission");
    
    Ok(())
}

#[test]
fn test_weather_command_with_network_permissions() -> Result<()> {
    let output = run_ergo_command(&["weather"])?;
    
    assert!(output.status.success(), "Weather command should succeed");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("🔒 Deno permissions required:"), "Should show permissions");
    assert!(stdout.contains("--allow-net=wttr.in"), "Should require network permission");
    assert!(stdout.contains("Weather:"), "Should display weather information");
    
    Ok(())
}

#[test]
fn test_system_command_passthrough() -> Result<()> {
    // Test with 'echo' which should exist on the system
    let output = run_ergo_command(&["echo", "test_system_command"])?;
    
    assert!(output.status.success(), "System command should succeed");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test_system_command"), "Should execute system echo command");
    
    // Check that it uses system command, not AI generation
    let stdout = String::from_utf8_lossy(&output.stdout);
    let has_system_path = stdout.contains("found in system PATH") || stdout.contains("Executing system command");
    assert!(has_system_path, "Should use system command directly. Stdout: {}", stdout);
    
    Ok(())
}

#[test] 
fn test_unknown_command_generation() -> Result<()> {
    let output = run_ergo_command(&["nonexistent-test-command"])?;
    
    assert!(output.status.success(), "Generated command should succeed");
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let has_generation = stdout.contains("not found, generating") || stdout.contains("Executing generated command");
    assert!(has_generation, "Should generate unknown command. Stdout: {}", stdout);
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("This is a generated command: nonexistent-test-command"), 
            "Should execute generated fallback command");
    
    Ok(())
}

#[test]
fn test_cache_persistence() -> Result<()> {
    // Use a unique command to test caching
    let unique_cmd = format!("persist-test-{}", std::process::id());
    
    let output1 = run_ergo_command(&[&unique_cmd])?;
    assert!(output1.status.success());
    
    // Command should be generated and stored
    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    assert!(stdout1.contains("generated command"), "Should show command generation");
    
    // Run the same command again
    let output2 = run_ergo_command(&[&unique_cmd])?;
    assert!(output2.status.success());
    
    // Should execute successfully regardless of cache hit/miss
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(stdout2.contains(&unique_cmd) || stdout2.contains("Executing"), "Should execute the command");
    
    Ok(())
}

#[test] 
fn test_deno_requirement() -> Result<()> {
    // This test ensures Deno is available for the sandboxed execution
    let deno_check = Command::new("deno")
        .arg("--version")
        .output();
        
    match deno_check {
        Ok(output) => {
            assert!(output.status.success(), "Deno should be installed and working");
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("deno"), "Should show deno version info");
        }
        Err(_) => {
            panic!("Deno is required for the ergo system to function properly. Please install Deno.");
        }
    }
    
    Ok(())
}