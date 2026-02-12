use clap::ValueEnum;
use std::collections::HashMap;
use std::io::IsTerminal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorPolicy {
    Auto,
    Always,
    Never,
}

impl ColorPolicy {
    pub fn enabled(self) -> bool {
        let stdout_is_tty = std::io::stdout().is_terminal();
        let env: HashMap<String, String> = std::env::vars().collect();
        self.enabled_with(stdout_is_tty, &env)
    }

    pub fn enabled_with(self, stdout_is_tty: bool, env: &HashMap<String, String>) -> bool {
        match self {
            Self::Always => return true,
            Self::Never => return false,
            Self::Auto => {}
        }

        if env.contains_key("NO_COLOR") {
            return false;
        }

        if matches!(env.get("CLICOLOR").map(String::as_str), Some("0")) {
            return false;
        }

        if matches!(env.get("TERM").map(String::as_str), Some("dumb")) {
            return false;
        }

        if matches!(env.get("CLICOLOR_FORCE").map(String::as_str), Some("1")) {
            return true;
        }

        if matches!(env.get("FORCE_COLOR").map(String::as_str), Some("1")) {
            return true;
        }

        stdout_is_tty
    }
}

#[cfg(test)]
mod tests {
    use super::ColorPolicy;
    use std::collections::HashMap;

    #[test]
    fn color_auto_tty_enabled() {
        assert!(ColorPolicy::Auto.enabled_with(true, &HashMap::new()));
    }

    #[test]
    fn color_auto_non_tty_disabled() {
        assert!(!ColorPolicy::Auto.enabled_with(false, &HashMap::new()));
    }

    #[test]
    fn color_never_forces_mono() {
        let mut env = HashMap::new();
        env.insert("FORCE_COLOR".to_string(), "1".to_string());
        assert!(!ColorPolicy::Never.enabled_with(true, &env));
    }

    #[test]
    fn color_always_forces_ansi() {
        assert!(ColorPolicy::Always.enabled_with(false, &HashMap::new()));
    }

    #[test]
    fn color_respects_no_color() {
        let mut env = HashMap::new();
        env.insert("NO_COLOR".to_string(), "1".to_string());
        assert!(!ColorPolicy::Auto.enabled_with(true, &env));
    }

    #[test]
    fn color_respects_clicolor_zero() {
        let mut env = HashMap::new();
        env.insert("CLICOLOR".to_string(), "0".to_string());
        assert!(!ColorPolicy::Auto.enabled_with(true, &env));
    }

    #[test]
    fn color_respects_force_color() {
        let mut env = HashMap::new();
        env.insert("FORCE_COLOR".to_string(), "1".to_string());
        assert!(ColorPolicy::Auto.enabled_with(false, &env));
    }
}
