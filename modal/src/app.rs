use crate::error::ModalError;

/// App references a deployed Modal App.
#[derive(Debug, Clone)]
pub struct App {
    pub app_id: String,
    pub name: String,
}

/// GPUConfig parsed from a GPU string.
#[derive(Debug, Clone)]
pub struct GpuConfig {
    pub gpu_type: String,
    pub count: u32,
}

/// Parse a GPU configuration string into a GpuConfig.
/// The GPU string format is "type" or "type:count" (e.g. "T4", "A100:2").
/// Returns a config with empty type and count=0 if gpu is empty.
pub fn parse_gpu_config(gpu: &str) -> Result<GpuConfig, ModalError> {
    if gpu.is_empty() {
        return Ok(GpuConfig {
            gpu_type: String::new(),
            count: 0,
        });
    }

    let (gpu_type, count) = if let Some(idx) = gpu.find(':') {
        let type_part = &gpu[..idx];
        let count_str = &gpu[idx + 1..];
        let parsed_count: u64 = count_str.parse().map_err(|_| {
            ModalError::Invalid(format!(
                "invalid GPU count: {}, value must be a positive integer",
                count_str
            ))
        })?;
        if parsed_count < 1 {
            return Err(ModalError::Invalid(format!(
                "invalid GPU count: {}, value must be a positive integer",
                count_str
            )));
        }
        (type_part.to_uppercase(), parsed_count as u32)
    } else {
        (gpu.to_uppercase(), 1)
    };

    Ok(GpuConfig { gpu_type, count })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gpu_config() {
        // Empty
        let config = parse_gpu_config("").unwrap();
        assert_eq!(config.count, 0);
        assert_eq!(config.gpu_type, "");

        // Simple type
        let config = parse_gpu_config("T4").unwrap();
        assert_eq!(config.count, 1);
        assert_eq!(config.gpu_type, "T4");

        let config = parse_gpu_config("A10G").unwrap();
        assert_eq!(config.count, 1);
        assert_eq!(config.gpu_type, "A10G");

        let config = parse_gpu_config("A100-80GB").unwrap();
        assert_eq!(config.count, 1);
        assert_eq!(config.gpu_type, "A100-80GB");

        // Type with count
        let config = parse_gpu_config("A100-80GB:3").unwrap();
        assert_eq!(config.count, 3);
        assert_eq!(config.gpu_type, "A100-80GB");

        let config = parse_gpu_config("T4:2").unwrap();
        assert_eq!(config.count, 2);
        assert_eq!(config.gpu_type, "T4");

        // Case insensitive
        let config = parse_gpu_config("a100:4").unwrap();
        assert_eq!(config.count, 4);
        assert_eq!(config.gpu_type, "A100");

        // Error cases
        let err = parse_gpu_config("T4:invalid").unwrap_err().to_string();
        assert!(err.contains("invalid GPU count: invalid"), "got: {}", err);

        let err = parse_gpu_config("T4:").unwrap_err().to_string();
        assert!(err.contains("invalid GPU count: "), "got: {}", err);

        let err = parse_gpu_config("T4:0").unwrap_err().to_string();
        assert!(err.contains("invalid GPU count: 0"), "got: {}", err);

        let err = parse_gpu_config("T4:-1").unwrap_err().to_string();
        assert!(err.contains("invalid GPU count: -1"), "got: {}", err);
    }
}
