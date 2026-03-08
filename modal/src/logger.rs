use crate::error::ModalError;

/// Log levels supported by the Modal SDK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// Parse a log level string into a LogLevel.
/// Empty string defaults to Warn. Case-insensitive.
pub fn parse_log_level(level: &str) -> Result<LogLevel, ModalError> {
    if level.is_empty() {
        return Ok(LogLevel::Warn);
    }

    match level.to_uppercase().as_str() {
        "DEBUG" => Ok(LogLevel::Debug),
        "INFO" => Ok(LogLevel::Info),
        "WARN" | "WARNING" => Ok(LogLevel::Warn),
        "ERROR" => Ok(LogLevel::Error),
        _ => Err(ModalError::Config(format!(
            "invalid log level value: {:?} (must be DEBUG, INFO, WARN, or ERROR)",
            level
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_log_level() {
        let tests = vec![
            ("DEBUG", LogLevel::Debug),
            ("INFO", LogLevel::Info),
            ("WARN", LogLevel::Warn),
            ("WARNING", LogLevel::Warn),
            ("ERROR", LogLevel::Error),
            ("eRrOr", LogLevel::Error),
            ("", LogLevel::Warn),
        ];

        for (input, expected) in tests {
            let level = parse_log_level(input).unwrap();
            assert_eq!(level, expected, "input: {:?}", input);
        }
    }

    #[test]
    fn test_parse_log_level_invalid() {
        let result = parse_log_level("invalid");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid log level"), "got: {}", err);
    }
}
