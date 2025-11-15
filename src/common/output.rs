use colored::*;

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

    pub fn print_cyan(&self, msg: &str) {
        self.print_msg(&msg.cyan().to_string());
    }

    pub fn print_bold(&self, msg: &str) {
        self.print_msg(&msg.bold().to_string());
    }

    pub fn section_header(&self, title: &str) {
        self.print_cyan(&format!("\n▶ {}", title));
    }

    pub fn success(&self, msg: &str) {
        self.print_green(&format!("✓ {}", msg));
    }

    pub fn info(&self, msg: &str) {
        self.print_blue(&format!("ℹ {}", msg));
    }

    pub fn warning(&self, msg: &str) {
        self.print_yellow(&format!("⚠ {}", msg));
    }

    pub fn error(&self, msg: &str) {
        self.print_red(&format!("✗ {}", msg));
    }

    pub fn progress(&self, current: usize, total: usize, unit: &str) {
        self.print_cyan(&format!("[{}/{}] {}", current, total, unit));
    }

    pub fn unit_prefix(&self, unit: &str, msg: &str) {
        self.print_blue(&format!("[{}] {}", unit, msg));
    }
}

impl Default for OutputManager {
    fn default() -> Self {
        Self::new()
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

}
