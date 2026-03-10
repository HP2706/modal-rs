/// gRPC interceptor chain for the Modal client.
///
/// Provides automatic header injection on every gRPC request matching the
/// Go SDK's `headerInjectorUnaryInterceptor` and `headerInjectorStreamInterceptor`.
///
/// Transport-level retry logic for transient gRPC errors is also defined here,
/// matching the Go SDK's `retryInterceptor`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tonic::metadata::MetadataValue;
use tonic::{Code, Request, Status};

use crate::error::ModalError;

// ============================================================================
// Header injection interceptor
// ============================================================================

/// A tonic interceptor that injects required Modal headers on every gRPC request.
///
/// Matches the Go SDK's `headerInjectorUnaryInterceptor` and
/// `headerInjectorStreamInterceptor`, injecting:
/// - `x-modal-client-type`: "9" (CLIENT_TYPE_LIBMODAL)
/// - `x-modal-client-version`: "1.0.0"
/// - `x-modal-libmodal-version`: "modal-rs/{sdk_version}"
/// - `x-modal-token-id`: credential token ID
/// - `x-modal-token-secret`: credential token secret
#[derive(Clone, Debug)]
pub struct ModalInterceptor {
    token_id: MetadataValue<tonic::metadata::Ascii>,
    token_secret: MetadataValue<tonic::metadata::Ascii>,
    libmodal_version: MetadataValue<tonic::metadata::Ascii>,
}

/// Static header values shared across all interceptor instances.
const CLIENT_TYPE: &str = "9";
const CLIENT_VERSION: &str = "1.0.0";

impl ModalInterceptor {
    /// Create a new interceptor with the given credentials and SDK version.
    ///
    /// Returns an error if token_id or token_secret are empty or contain
    /// invalid ASCII for gRPC metadata.
    pub fn new(token_id: &str, token_secret: &str, sdk_version: &str) -> Result<Self, ModalError> {
        if token_id.is_empty() || token_secret.is_empty() {
            return Err(ModalError::Config(
                "missing token_id or token_secret, please set in .modal.toml, environment variables, or via ClientParams".to_string(),
            ));
        }

        let token_id = MetadataValue::try_from(token_id).map_err(|e| {
            ModalError::Config(format!("invalid token_id for gRPC metadata: {}", e))
        })?;
        let token_secret = MetadataValue::try_from(token_secret).map_err(|e| {
            ModalError::Config(format!("invalid token_secret for gRPC metadata: {}", e))
        })?;
        let libmodal_version =
            MetadataValue::try_from(format!("modal-rs/{}", sdk_version)).map_err(|e| {
                ModalError::Config(format!("invalid sdk_version for gRPC metadata: {}", e))
            })?;

        Ok(Self {
            token_id,
            token_secret,
            libmodal_version,
        })
    }
}

impl tonic::service::Interceptor for ModalInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let md = request.metadata_mut();
        md.insert("x-modal-client-type", MetadataValue::from_static(CLIENT_TYPE));
        md.insert(
            "x-modal-client-version",
            MetadataValue::from_static(CLIENT_VERSION),
        );
        md.insert("x-modal-libmodal-version", self.libmodal_version.clone());
        md.insert("x-modal-token-id", self.token_id.clone());
        md.insert("x-modal-token-secret", self.token_secret.clone());
        Ok(request)
    }
}

// ============================================================================
// Transport-level retry configuration
// ============================================================================

/// Retry configuration for transient gRPC errors.
///
/// Matches the Go SDK's `retryInterceptor` defaults:
/// - 3 retry attempts
/// - 100ms base delay
/// - 1s max delay
/// - 2x backoff multiplier
/// - Retryable codes: DeadlineExceeded, Unavailable, Canceled
#[derive(Debug, Clone)]
pub struct GrpcRetryConfig {
    pub max_retries: usize,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_factor: u32,
}

impl Default for GrpcRetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(1),
            backoff_factor: 2,
        }
    }
}

impl GrpcRetryConfig {
    /// Check if a gRPC status code is retryable.
    ///
    /// Matches the Go SDK's `retryableGrpcStatusCodes`:
    /// DeadlineExceeded, Unavailable, Canceled.
    pub fn is_retryable(&self, code: Code) -> bool {
        matches!(
            code,
            Code::DeadlineExceeded | Code::Unavailable | Code::Cancelled
        )
    }
}

// ============================================================================
// Retry execution
// ============================================================================

/// Global counter for generating unique idempotency keys (combined with timestamp).
static IDEMPOTENCY_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a simple idempotency key from timestamp + atomic counter.
fn generate_idempotency_key() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let seq = IDEMPOTENCY_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}", ts, seq)
}

/// Execute a gRPC call with automatic retry on transient errors.
///
/// Matches the Go SDK's `retryInterceptor` behavior:
/// - On transient error codes, sleeps with exponential backoff and retries
/// - Non-retryable errors are returned immediately
/// - After exhausting retries, returns the last error
///
/// The `make_call` closure is invoked on each attempt, receiving the current
/// attempt number (0-based). This allows callers to add retry metadata headers
/// if desired.
pub fn retry_call<T, F, Fut>(
    runtime: &tokio::runtime::Handle,
    config: &GrpcRetryConfig,
    make_call: F,
) -> Result<T, ModalError>
where
    F: Fn(RetryContext) -> Fut,
    Fut: std::future::Future<Output = Result<T, Status>>,
{
    runtime.block_on(retry_call_async(config, make_call))
}

/// Async implementation of retry logic, usable from both sync and async contexts.
pub async fn retry_call_async<T, F, Fut>(
    config: &GrpcRetryConfig,
    make_call: F,
) -> Result<T, ModalError>
where
    F: Fn(RetryContext<'_>) -> Fut,
    Fut: std::future::Future<Output = Result<T, Status>>,
{
    let idempotency_key = generate_idempotency_key();
    let mut delay = config.base_delay;

    for attempt in 0..=config.max_retries {
        let ctx = RetryContext {
            attempt,
            idempotency_key: &idempotency_key,
        };

        match make_call(ctx).await {
            Ok(val) => return Ok(val),
            Err(status) => {
                if config.is_retryable(status.code()) && attempt < config.max_retries {
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay * config.backoff_factor, config.max_delay);
                    continue;
                }
                return Err(ModalError::Grpc(status));
            }
        }
    }

    unreachable!()
}

/// Context passed to each retry attempt.
#[derive(Debug, Clone)]
pub struct RetryContext<'a> {
    pub attempt: usize,
    pub idempotency_key: &'a str,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;
    use tonic::service::Interceptor;

    #[test]
    fn test_interceptor_injects_all_headers() {
        let mut interceptor =
            ModalInterceptor::new("tk-test-id", "ts-test-secret", "0.1.0").unwrap();
        let request = Request::new(());
        let result = interceptor.call(request).unwrap();
        let md = result.metadata();

        assert_eq!(md.get("x-modal-client-type").unwrap(), "9");
        assert_eq!(md.get("x-modal-client-version").unwrap(), "1.0.0");
        assert_eq!(md.get("x-modal-libmodal-version").unwrap(), "modal-rs/0.1.0");
        assert_eq!(md.get("x-modal-token-id").unwrap(), "tk-test-id");
        assert_eq!(md.get("x-modal-token-secret").unwrap(), "ts-test-secret");
    }

    #[test]
    fn test_interceptor_rejects_empty_token_id() {
        let result = ModalInterceptor::new("", "ts-secret", "0.1.0");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing token_id"));
    }

    #[test]
    fn test_interceptor_rejects_empty_token_secret() {
        let result = ModalInterceptor::new("tk-id", "", "0.1.0");
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("missing token_id"),
            "both empty checks use the same message"
        );
    }

    #[test]
    fn test_retry_config_defaults() {
        let config = GrpcRetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_delay, Duration::from_millis(100));
        assert_eq!(config.max_delay, Duration::from_secs(1));
        assert_eq!(config.backoff_factor, 2);
    }

    #[test]
    fn test_is_retryable_codes() {
        let config = GrpcRetryConfig::default();
        // Retryable
        assert!(config.is_retryable(Code::DeadlineExceeded));
        assert!(config.is_retryable(Code::Unavailable));
        assert!(config.is_retryable(Code::Cancelled));
        // Not retryable
        assert!(!config.is_retryable(Code::NotFound));
        assert!(!config.is_retryable(Code::PermissionDenied));
        assert!(!config.is_retryable(Code::InvalidArgument));
        assert!(!config.is_retryable(Code::Internal));
        assert!(!config.is_retryable(Code::Ok));
    }

    #[test]
    fn test_generate_idempotency_key_unique() {
        let key1 = generate_idempotency_key();
        let key2 = generate_idempotency_key();
        assert_ne!(key1, key2);
        assert!(key1.contains('-'));
    }

    #[tokio::test]
    async fn test_retry_call_success_no_retry() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&call_count);

        let config = GrpcRetryConfig::default();

        let result: Result<String, ModalError> = retry_call_async(&config, |_ctx| {
            let c = Arc::clone(&count);
            async move {
                c.fetch_add(1, Ordering::Relaxed);
                Ok("success".to_string())
            }
        })
        .await;

        assert_eq!(result.unwrap(), "success");
        assert_eq!(call_count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_retry_call_retries_transient_error() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&call_count);

        let config = GrpcRetryConfig {
            base_delay: Duration::from_millis(1), // fast for tests
            max_delay: Duration::from_millis(10),
            ..GrpcRetryConfig::default()
        };

        let result: Result<String, ModalError> = retry_call_async(&config, |_ctx| {
            let c = Arc::clone(&count);
            async move {
                let attempt = c.fetch_add(1, Ordering::Relaxed);
                if attempt < 2 {
                    Err(Status::unavailable("transient"))
                } else {
                    Ok("recovered".to_string())
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), "recovered");
        assert_eq!(call_count.load(Ordering::Relaxed), 3); // 2 failures + 1 success
    }

    #[tokio::test]
    async fn test_retry_call_non_retryable_fails_immediately() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&call_count);

        let config = GrpcRetryConfig {
            base_delay: Duration::from_millis(1),
            ..GrpcRetryConfig::default()
        };

        let result: Result<String, ModalError> = retry_call_async(&config, |_ctx| {
            let c = Arc::clone(&count);
            async move {
                c.fetch_add(1, Ordering::Relaxed);
                Err(Status::not_found("gone"))
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::Relaxed), 1); // no retries
    }

    #[tokio::test]
    async fn test_retry_call_exhausts_retries() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&call_count);

        let config = GrpcRetryConfig {
            max_retries: 2,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            ..GrpcRetryConfig::default()
        };

        let result: Result<String, ModalError> = retry_call_async(&config, |_ctx| {
            let c = Arc::clone(&count);
            async move {
                c.fetch_add(1, Ordering::Relaxed);
                Err(Status::unavailable("always failing"))
            }
        })
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("always failing"));
        assert_eq!(call_count.load(Ordering::Relaxed), 3); // initial + 2 retries
    }

    #[tokio::test]
    async fn test_retry_call_passes_context() {
        let attempts = Arc::new(std::sync::Mutex::new(Vec::new()));
        let a = Arc::clone(&attempts);

        let config = GrpcRetryConfig {
            max_retries: 2,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
            ..GrpcRetryConfig::default()
        };

        let _result: Result<String, ModalError> = retry_call_async(&config, |ctx| {
            let a = Arc::clone(&a);
            let attempt = ctx.attempt;
            let key = ctx.idempotency_key.to_string();
            async move {
                a.lock().unwrap().push((attempt, key));
                if attempt < 2 {
                    Err(Status::unavailable("retry"))
                } else {
                    Ok("done".to_string())
                }
            }
        })
        .await;

        let recorded = attempts.lock().unwrap();
        assert_eq!(recorded.len(), 3);
        assert_eq!(recorded[0].0, 0);
        assert_eq!(recorded[1].0, 1);
        assert_eq!(recorded[2].0, 2);
        // All attempts should have the same idempotency key
        assert_eq!(recorded[0].1, recorded[1].1);
        assert_eq!(recorded[1].1, recorded[2].1);
    }
}
