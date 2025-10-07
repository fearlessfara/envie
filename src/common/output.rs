use colored::*;
use std::fmt;

pub struct OutputManager {
    suppress_echo: bool,
}

impl OutputManager {
    pub fn new() -> Self {
        Self {
            suppress_echo: std::env::var("SUPPRESS_ECHO").is_ok(),
        }
    }

    pub fn print_msg(&self, msg: &str) {
        if !self.suppress_echo {
            println!("{}", msg);
        }
    }

    pub fn print_blue(&self, msg: &str) {
        self.print_msg(&msg.blue().to_string());
    }

    pub fn print_green(&self, msg: &str) {
        self.print_msg(&msg.green().to_string());
    }

    pub fn print_yellow(&self, msg: &str) {
        self.print_msg(&msg.yellow().to_string());
    }

    pub fn print_red(&self, msg: &str) {
        self.print_msg(&msg.red().to_string());
    }

    pub fn print_gray(&self, msg: &str) {
        self.print_msg(&msg.bright_black().to_string());
    }

    pub fn print_success(&self, msg: &str) {
        self.print_green(&format!("✓ {}", msg));
    }

    pub fn print_error(&self, msg: &str) {
        self.print_red(&format!("✗ {}", msg));
    }

    pub fn print_warning(&self, msg: &str) {
        self.print_yellow(&format!("⚠ {}", msg));
    }

    pub fn print_info(&self, msg: &str) {
        self.print_blue(&format!("ℹ {}", msg));
    }
}

impl Default for OutputManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ProgressBar {
    current: usize,
    total: usize,
    message: String,
}

impl ProgressBar {
    pub fn new(total: usize, message: &str) -> Self {
        Self {
            current: 0,
            total,
            message: message.to_string(),
        }
    }

    pub fn update(&mut self, current: usize) {
        self.current = current;
    }

    pub fn increment(&mut self) {
        if self.current < self.total {
            self.current += 1;
        }
    }

    pub fn finish(&self) {
        println!("{}", self);
    }
}

impl fmt::Display for ProgressBar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let percentage = (self.current as f64 / self.total as f64 * 100.0) as usize;
        let bar_length = 50;
        let filled_length = (self.current as f64 / self.total as f64 * bar_length as f64) as usize;
        
        let bar = "█".repeat(filled_length) + &"░".repeat(bar_length - filled_length);
        
        write!(
            f,
            "\r{} [{}] {}% ({}/{})",
            self.message,
            bar,
            percentage,
            self.current,
            self.total
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_manager_creation() {
        let output = OutputManager::new();
        assert!(!output.suppress_echo || std::env::var("SUPPRESS_ECHO").is_ok());
    }

    #[test]
    fn test_progress_bar_creation() {
        let progress = ProgressBar::new(10, "Testing");
        assert_eq!(progress.current, 0);
        assert_eq!(progress.total, 10);
        assert_eq!(progress.message, "Testing");
    }

    #[test]
    fn test_progress_bar_increment() {
        let mut progress = ProgressBar::new(10, "Testing");
        progress.increment();
        assert_eq!(progress.current, 1);
    }

    #[test]
    fn test_progress_bar_update() {
        let mut progress = ProgressBar::new(10, "Testing");
        progress.update(5);
        assert_eq!(progress.current, 5);
    }
}
