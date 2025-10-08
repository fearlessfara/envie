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
