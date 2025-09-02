//! Cleanup script for orphaned test processes
//!
//! This script searches for and kills orphaned `geckodriver` and `trunk serve` processes
//! that may remain after test failures or cancellations.
//!
//! Usage:
//!   cargo run --bin cleanup

use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧹 Searching for orphaned test processes...");

    let mut killed_count = 0;

    // Find and kill geckodriver processes
    println!("🦎 Looking for geckodriver processes...");
    killed_count +=
        kill_processes_by_pattern("geckodriver --port", "geckodriver")?;

    // Find and kill trunk serve processes
    println!("🎨 Looking for trunk serve processes...");
    killed_count +=
        kill_processes_by_pattern("trunk serve --port", "trunk serve")?;

    if killed_count == 0 {
        println!("✨ No orphaned test processes found!");
    } else {
        println!("🎉 Cleaned up {} orphaned test processes", killed_count);
    }

    Ok(())
}

fn kill_processes_by_pattern(
    pattern: &str,
    process_name: &str,
) -> Result<u32, Box<dyn std::error::Error>> {
    let mut killed_count = 0;

    // Use pgrep to find processes matching the pattern
    let output = Command::new("pgrep").arg("-f").arg(pattern).output()?;

    let pids = String::from_utf8_lossy(&output.stdout);
    for pid in pids.lines() {
        if let Ok(pid_num) = pid.parse::<u32>() {
            println!("🔥 Killing {} process: {}", process_name, pid_num);
            let kill_result = Command::new("kill").arg("-9").arg(pid).output();

            match kill_result {
                Ok(_) => {
                    killed_count += 1;
                    println!("✅ Killed {} process: {}", process_name, pid_num);
                }
                Err(e) => {
                    println!(
                        "❌ Failed to kill {} process {}: {}",
                        process_name, pid_num, e
                    );
                }
            }
        }
    }

    Ok(killed_count)
}
