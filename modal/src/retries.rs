use std::time::Duration;

use crate::error::ModalError;

/// Retries represents retry policy configuration for a Modal Function/Cls.
#[derive(Debug, Clone)]
pub struct Retries {
    pub max_retries: i32,
    pub backoff_coefficient: f32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

/// RetriesParams are optional parameters for creating a Retries policy.
#[derive(Debug, Clone, Default)]
pub struct RetriesParams {
    pub backoff_coefficient: Option<f32>,
    pub initial_delay: Option<Duration>,
    pub max_delay: Option<Duration>,
}

impl Retries {
    /// Creates a new Retries configuration.
    pub fn new(max_retries: i32, params: Option<&RetriesParams>) -> Result<Self, ModalError> {
        let mut backoff_coefficient: f32 = 2.0;
        let mut initial_delay = Duration::from_secs(1);
        let mut max_delay = Duration::from_secs(60);

        if let Some(p) = params {
            if let Some(bc) = p.backoff_coefficient {
                backoff_coefficient = bc;
            }
            if let Some(id) = p.initial_delay {
                initial_delay = id;
            }
            if let Some(md) = p.max_delay {
                max_delay = md;
            }
        }

        let r = Retries {
            max_retries,
            backoff_coefficient,
            initial_delay,
            max_delay,
        };

        if r.max_retries < 0 || r.max_retries > 10 {
            return Err(ModalError::Invalid(format!(
                "invalid maxRetries: {}. Must be between 0 and 10",
                r.max_retries
            )));
        }

        if r.backoff_coefficient < 1.0 || r.backoff_coefficient > 10.0 {
            return Err(ModalError::Invalid(format!(
                "invalid backoffCoefficient: {}. Must be between 1.0 and 10.0",
                r.backoff_coefficient
            )));
        }

        if r.initial_delay > Duration::from_secs(60) {
            return Err(ModalError::Invalid(format!(
                "invalid initialDelay: {:?}. Must be between 0 and 60s",
                r.initial_delay
            )));
        }

        if r.max_delay < Duration::from_secs(1) || r.max_delay > Duration::from_secs(60) {
            return Err(ModalError::Invalid(format!(
                "invalid maxDelay: {:?}. Must be between 1s and 60s",
                r.max_delay
            )));
        }

        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_retries_defaults() {
        let r = Retries::new(3, None).unwrap();
        assert_eq!(r.max_retries, 3);
        assert_eq!(r.backoff_coefficient, 2.0);
        assert_eq!(r.initial_delay, Duration::from_secs(1));
        assert_eq!(r.max_delay, Duration::from_secs(60));
    }

    #[test]
    fn test_new_retries_max_retries_too_high() {
        let result = Retries::new(11, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("maxRetries"));
    }

    #[test]
    fn test_new_retries_max_retries_negative() {
        let result = Retries::new(-1, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("maxRetries"));
    }

    #[test]
    fn test_new_retries_valid_boundaries() {
        assert!(Retries::new(0, None).is_ok());
        assert!(Retries::new(10, None).is_ok());
    }

    #[test]
    fn test_new_retries_backoff_too_low() {
        let params = RetriesParams {
            backoff_coefficient: Some(0.5),
            ..Default::default()
        };
        let result = Retries::new(3, Some(&params));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("backoffCoefficient"));
    }

    #[test]
    fn test_new_retries_backoff_too_high() {
        let params = RetriesParams {
            backoff_coefficient: Some(11.0),
            ..Default::default()
        };
        let result = Retries::new(3, Some(&params));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("backoffCoefficient"));
    }

    #[test]
    fn test_new_retries_initial_delay_too_high() {
        let params = RetriesParams {
            initial_delay: Some(Duration::from_secs(61)),
            ..Default::default()
        };
        let result = Retries::new(3, Some(&params));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("initialDelay"));
    }

    #[test]
    fn test_new_retries_max_delay_too_low() {
        let params = RetriesParams {
            max_delay: Some(Duration::from_millis(500)),
            ..Default::default()
        };
        let result = Retries::new(3, Some(&params));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("maxDelay"));
    }

    #[test]
    fn test_new_retries_max_delay_too_high() {
        let params = RetriesParams {
            max_delay: Some(Duration::from_secs(61)),
            ..Default::default()
        };
        let result = Retries::new(3, Some(&params));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("maxDelay"));
    }
}
