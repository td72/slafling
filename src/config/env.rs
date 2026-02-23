use super::util::is_truthy;

/// All environment variables read at startup, in one place.
#[derive(Debug, Default)]
pub struct Env {
    pub headless: bool,
    pub profile: Option<String>,       // normal mode only
    pub token: Option<String>,         // headless only
    pub channel: Option<String>,       // headless only
    pub output: Option<String>,        // both modes
    pub max_file_size: Option<String>, // both modes
    pub confirm: Option<String>,       // both modes
    pub search_types: Option<String>,  // both modes
}

impl Env {
    pub fn load() -> Self {
        fn opt(key: &str) -> Option<String> {
            std::env::var(key).ok().filter(|s| !s.is_empty())
        }
        Self {
            headless: opt("SLAFLING_HEADLESS")
                .map(|v| is_truthy(&v))
                .unwrap_or(false),
            profile: opt("SLAFLING_PROFILE"),
            token: opt("SLAFLING_TOKEN"),
            channel: opt("SLAFLING_CHANNEL"),
            output: opt("SLAFLING_OUTPUT"),
            max_file_size: opt("SLAFLING_MAX_FILE_SIZE"),
            confirm: opt("SLAFLING_CONFIRM"),
            search_types: opt("SLAFLING_SEARCH_TYPES"),
        }
    }
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    #[test]
    fn env_default_is_all_none() {
        let env = Env::default();
        assert!(!env.headless);
        assert!(env.profile.is_none());
        assert!(env.token.is_none());
        assert!(env.channel.is_none());
        assert!(env.output.is_none());
        assert!(env.max_file_size.is_none());
        assert!(env.confirm.is_none());
        assert!(env.search_types.is_none());
    }

    #[test]
    #[serial]
    fn env_load_reads_all_vars() {
        let keys = [
            ("SLAFLING_HEADLESS", "1"),
            ("SLAFLING_PROFILE", "work"),
            ("SLAFLING_TOKEN", "xoxb-test"),
            ("SLAFLING_CHANNEL", "#general"),
            ("SLAFLING_OUTPUT", "json"),
            ("SLAFLING_MAX_FILE_SIZE", "50MB"),
            ("SLAFLING_CONFIRM", "true"),
            ("SLAFLING_SEARCH_TYPES", "im,mpim"),
        ];
        let prev: Vec<_> = keys
            .iter()
            .map(|(k, _)| (*k, std::env::var(k).ok()))
            .collect();
        for (k, v) in &keys {
            std::env::set_var(k, v);
        }

        let env = Env::load();

        for (k, p) in prev {
            match p {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }

        assert!(env.headless);
        assert_eq!(env.profile.as_deref(), Some("work"));
        assert_eq!(env.token.as_deref(), Some("xoxb-test"));
        assert_eq!(env.channel.as_deref(), Some("#general"));
        assert_eq!(env.output.as_deref(), Some("json"));
        assert_eq!(env.max_file_size.as_deref(), Some("50MB"));
        assert_eq!(env.confirm.as_deref(), Some("true"));
        assert_eq!(env.search_types.as_deref(), Some("im,mpim"));
    }

    #[test]
    #[serial]
    fn env_load_filters_empty_strings() {
        let keys = [
            "SLAFLING_TOKEN",
            "SLAFLING_CHANNEL",
            "SLAFLING_OUTPUT",
            "SLAFLING_MAX_FILE_SIZE",
            "SLAFLING_CONFIRM",
            "SLAFLING_SEARCH_TYPES",
            "SLAFLING_PROFILE",
        ];
        let prev: Vec<_> = keys.iter().map(|k| (*k, std::env::var(k).ok())).collect();
        for k in &keys {
            std::env::set_var(k, "");
        }
        std::env::set_var("SLAFLING_HEADLESS", "0");

        let env = Env::load();

        for (k, p) in prev {
            match p {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
        std::env::remove_var("SLAFLING_HEADLESS");

        assert!(!env.headless);
        assert!(env.token.is_none());
        assert!(env.channel.is_none());
        assert!(env.output.is_none());
        assert!(env.max_file_size.is_none());
        assert!(env.confirm.is_none());
        assert!(env.search_types.is_none());
        assert!(env.profile.is_none());
    }
}
