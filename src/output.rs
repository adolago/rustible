//! Output and reporting for Rustible

use colored::Colorize;

/// Print a play header
pub fn play_header(name: &str) {
    let header = format!("PLAY [{}]", name);
    let stars = "*".repeat(80_usize.saturating_sub(header.len()));
    println!("\n{} {}", header.bright_white().bold(), stars.bright_black());
}

/// Print a task header
pub fn task_header(name: &str) {
    let header = format!("TASK [{}]", name);
    let stars = "*".repeat(80_usize.saturating_sub(header.len()));
    println!("\n{} {}", header.bright_white().bold(), stars.bright_black());
}

/// Print an ok result
pub fn ok(host: &str) {
    println!("{}: [{}]", "ok".green(), host.bright_white().bold());
}

/// Print a changed result
pub fn changed(host: &str) {
    println!("{}: [{}]", "changed".yellow(), host.bright_white().bold());
}

/// Print a failed result
pub fn failed(host: &str, msg: &str) {
    println!("{}: [{}] => {}", "failed".red().bold(), host.bright_white().bold(), msg);
}

/// Print a skipped result
pub fn skipped(host: &str) {
    println!("{}: [{}]", "skipping".cyan(), host.bright_white().bold());
}

/// Print recap
pub fn recap(hosts: &[(String, u32, u32, u32, u32)]) {
    println!("\n{} {}", "PLAY RECAP".bright_white().bold(), "*".repeat(70).bright_black());

    for (host, ok, changed, failed, skipped) in hosts {
        let host_colored = if *failed > 0 {
            host.red().bold()
        } else if *changed > 0 {
            host.yellow()
        } else {
            host.green()
        };

        println!(
            "{:<30} : {}={:<4} {}={:<4} {}={:<4} {}={:<4}",
            host_colored,
            "ok".green(), ok,
            "changed".yellow(), changed,
            "failed".red(), failed,
            "skipped".cyan(), skipped,
        );
    }
}
