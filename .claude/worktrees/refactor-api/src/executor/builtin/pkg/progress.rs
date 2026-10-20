// src/executor/builtin/pkg/progress.rs
//
// Terminal progress-bar rendering used during download, extraction, and
// uninstallation.  Nothing in here touches the filesystem or network.

use std::io::Write;

const BAR_WIDTH   : usize = 20;
const FILLED_CHAR : &str  = "#";
const EMPTY_CHAR  : &str  = "·";
const BAR_OPEN    : &str  = "{";
const BAR_CLOSE   : &str  = "}";

// ── Bar builder ───────────────────────────────────────────────────────────────

fn make_bar(percent: usize) -> String {
    let filled = (percent * BAR_WIDTH) / 100;
    let empty  = BAR_WIDTH.saturating_sub(filled);
    format!(
        "{}{}{}{}",
        BAR_OPEN,
        FILLED_CHAR.repeat(filled),
        EMPTY_CHAR.repeat(empty),
        BAR_CLOSE,
    )
}

// ── Public printers ───────────────────────────────────────────────────────────

pub fn print_download_progress(downloaded: u64, total: Option<u64>) {
    match total {
        Some(t) if t > 0 => {
            let percent  = ((downloaded * 100) / t) as usize;
            let dl_mb    = downloaded as f64 / 1_048_576.0;
            let total_mb = t          as f64 / 1_048_576.0;
            print!("\r   {} {}%  {:.1}/{:.1} MB", make_bar(percent), percent, dl_mb, total_mb);
        }
        _ => {
            let dl_mb = downloaded as f64 / 1_048_576.0;
            print!("\r   ⬇️  {:.1} MB downloaded...", dl_mb);
        }
    }
    std::io::stdout().flush().ok();
}

pub fn print_extract_progress(current: usize, total: usize) {
    if total == 0 { return; }
    let percent = (current * 100) / total;
    print!("\r   {} {}%  ({}/{})", make_bar(percent), percent, current, total);
    std::io::stdout().flush().ok();
}

pub fn print_uninstall_progress(current: usize, total: usize, filename: &str) {
    if total == 0 { return; }
    let percent = (current * 100) / total;
    let name    = if filename.len() > 25 {
        format!("...{}", &filename[filename.len() - 22..])
    } else {
        filename.to_string()
    };
    print!("\r   {} {}%  {}", make_bar(percent), percent, name);
    std::io::stdout().flush().ok();
}

pub fn clear_progress_line() {
    print!("\r{}\r", " ".repeat(70));
    std::io::stdout().flush().ok();
}