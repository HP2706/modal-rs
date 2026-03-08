use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::error::ModalError;

/// Retry options for task command router calls.
#[derive(Debug, Clone)]
pub struct RetryOptions {
    pub base_delay: Duration,
    pub delay_factor: f64,
    pub max_retries: Option<i32>,
    pub deadline: Option<Instant>,
}

impl Default for RetryOptions {
    fn default() -> Self {
        Self {
            base_delay: Duration::from_millis(10),
            delay_factor: 2.0,
            max_retries: Some(10),
            deadline: None,
        }
    }
}

/// gRPC status codes that are considered transient and retryable.
fn is_transient_code(code: tonic::Code) -> bool {
    matches!(
        code,
        tonic::Code::DeadlineExceeded
            | tonic::Code::Unavailable
            | tonic::Code::Cancelled
            | tonic::Code::Internal
            | tonic::Code::Unknown
    )
}

/// Call a function with retries on transient gRPC errors.
pub async fn call_with_retries_on_transient_errors<T, F, Fut>(
    f: F,
    opts: RetryOptions,
    closed: Option<&AtomicBool>,
) -> Result<T, ModalError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, tonic::Status>>,
{
    let mut delay = opts.base_delay;
    let mut num_retries = 0;

    loop {
        if let Some(deadline) = opts.deadline {
            if Instant::now() >= deadline {
                return Err(ModalError::Other("deadline exceeded".to_string()));
            }
        }

        match f().await {
            Ok(result) => return Ok(result),
            Err(status) => {
                if let Some(c) = closed {
                    if c.load(Ordering::SeqCst) && status.code() == tonic::Code::Cancelled {
                        return Err(ModalError::ClientClosed(
                            "Unable to perform operation on a detached sandbox".to_string(),
                        ));
                    }
                }

                if !is_transient_code(status.code()) {
                    return Err(ModalError::Grpc(status));
                }

                if let Some(max) = opts.max_retries {
                    if num_retries >= max {
                        return Err(ModalError::Grpc(status));
                    }
                }

                if let Some(deadline) = opts.deadline {
                    if Instant::now() + delay >= deadline {
                        return Err(ModalError::Other("deadline exceeded".to_string()));
                    }
                }

                tokio::time::sleep(delay).await;
                delay = Duration::from_secs_f64(delay.as_secs_f64() * opts.delay_factor);
                num_retries += 1;
            }
        }
    }
}

/// RetryableClient trait for auth retry.
pub trait RetryableClient: Send + Sync {
    fn auth_context(&self) -> u32; // returns a counter for tracking
    fn refresh_jwt(&self) -> Result<(), ModalError>;
}

/// Call a function with auth retry on UNAUTHENTICATED.
pub async fn call_with_auth_retry<T, C, F, Fut>(
    client: &C,
    f: F,
) -> Result<T, ModalError>
where
    C: RetryableClient,
    F: Fn(u32) -> Fut,
    Fut: std::future::Future<Output = Result<T, tonic::Status>>,
{
    let auth_ctx = client.auth_context();
    match f(auth_ctx).await {
        Ok(result) => Ok(result),
        Err(status) if status.code() == tonic::Code::Unauthenticated => {
            client.refresh_jwt()?;
            let auth_ctx = client.auth_context();
            f(auth_ctx).await.map_err(ModalError::Grpc)
        }
        Err(status) => Err(ModalError::Grpc(status)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_call_with_retries_success_first_attempt() {
        let call_count = Arc::new(AtomicI32::new(0));
        let cc = call_count.clone();

        let result = call_with_retries_on_transient_errors(
            || {
                let cc = cc.clone();
                async move {
                    cc.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, tonic::Status>("success".to_string())
                }
            },
            RetryOptions::default(),
            None,
        )
        .await
        .unwrap();

        assert_eq!(result, "success");
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_call_with_retries_on_transient_codes() {
        let codes = vec![
            ("DeadlineExceeded", tonic::Code::DeadlineExceeded),
            ("Unavailable", tonic::Code::Unavailable),
            ("Canceled", tonic::Code::Cancelled),
            ("Internal", tonic::Code::Internal),
            ("Unknown", tonic::Code::Unknown),
        ];

        for (name, code) in codes {
            let call_count = Arc::new(AtomicI32::new(0));
            let cc = call_count.clone();

            let result = call_with_retries_on_transient_errors(
                || {
                    let cc = cc.clone();
                    async move {
                        let count = cc.fetch_add(1, Ordering::SeqCst) + 1;
                        if count == 1 {
                            Err(tonic::Status::new(code, "error"))
                        } else {
                            Ok::<_, tonic::Status>("success".to_string())
                        }
                    }
                },
                RetryOptions {
                    base_delay: Duration::from_millis(1),
                    delay_factor: 1.0,
                    max_retries: Some(10),
                    deadline: None,
                },
                None,
            )
            .await
            .unwrap();

            assert_eq!(result, "success", "case: {}", name);
            assert_eq!(call_count.load(Ordering::SeqCst), 2, "case: {}", name);
        }
    }

    #[tokio::test]
    async fn test_call_with_retries_non_retryable_error() {
        let call_count = Arc::new(AtomicI32::new(0));
        let cc = call_count.clone();

        let result = call_with_retries_on_transient_errors(
            || {
                let cc = cc.clone();
                async move {
                    cc.fetch_add(1, Ordering::SeqCst);
                    Err::<String, _>(tonic::Status::new(
                        tonic::Code::InvalidArgument,
                        "invalid",
                    ))
                }
            },
            RetryOptions {
                base_delay: Duration::from_millis(1),
                delay_factor: 1.0,
                max_retries: Some(10),
                deadline: None,
            },
            None,
        )
        .await;

        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_call_with_retries_max_retries_exceeded() {
        let call_count = Arc::new(AtomicI32::new(0));
        let cc = call_count.clone();
        let max_retries = 3;

        let result = call_with_retries_on_transient_errors(
            || {
                let cc = cc.clone();
                async move {
                    cc.fetch_add(1, Ordering::SeqCst);
                    Err::<String, _>(tonic::Status::new(tonic::Code::Unavailable, "unavailable"))
                }
            },
            RetryOptions {
                base_delay: Duration::from_millis(1),
                delay_factor: 1.0,
                max_retries: Some(max_retries),
                deadline: None,
            },
            None,
        )
        .await;

        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), max_retries + 1);
    }

    #[tokio::test]
    async fn test_call_with_retries_deadline_exceeded() {
        let call_count = Arc::new(AtomicI32::new(0));
        let cc = call_count.clone();

        let result = call_with_retries_on_transient_errors(
            || {
                let cc = cc.clone();
                async move {
                    cc.fetch_add(1, Ordering::SeqCst);
                    Err::<String, _>(tonic::Status::new(tonic::Code::Unavailable, "unavailable"))
                }
            },
            RetryOptions {
                base_delay: Duration::from_millis(100),
                delay_factor: 1.0,
                max_retries: None,
                deadline: Some(Instant::now() + Duration::from_millis(50)),
            },
            None,
        )
        .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "deadline exceeded");
    }

    #[tokio::test]
    async fn test_call_with_retries_closed() {
        let call_count = Arc::new(AtomicI32::new(0));
        let cc = call_count.clone();
        let closed = AtomicBool::new(true);

        let result = call_with_retries_on_transient_errors(
            || {
                let cc = cc.clone();
                async move {
                    cc.fetch_add(1, Ordering::SeqCst);
                    Err::<String, _>(tonic::Status::new(tonic::Code::Cancelled, "cancelled"))
                }
            },
            RetryOptions::default(),
            Some(&closed),
        )
        .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("ClientClosedError: Unable to perform operation on a detached sandbox"),
            "got: {}",
            err_msg
        );
    }

    // Mock retryable client for auth retry tests
    struct MockRetryableClient {
        refresh_jwt_call_count: AtomicI32,
        auth_context_call_count: AtomicI32,
    }

    impl MockRetryableClient {
        fn new() -> Self {
            Self {
                refresh_jwt_call_count: AtomicI32::new(0),
                auth_context_call_count: AtomicI32::new(0),
            }
        }
    }

    impl RetryableClient for MockRetryableClient {
        fn auth_context(&self) -> u32 {
            self.auth_context_call_count.fetch_add(1, Ordering::SeqCst) as u32 + 1
        }

        fn refresh_jwt(&self) -> Result<(), ModalError> {
            self.refresh_jwt_call_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_call_with_auth_retry_success_first_attempt() {
        let c = MockRetryableClient::new();

        let result = call_with_auth_retry(&c, |_auth_ctx| async move {
            Ok::<_, tonic::Status>(3)
        })
        .await
        .unwrap();

        assert_eq!(c.auth_context_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(c.refresh_jwt_call_count.load(Ordering::SeqCst), 0);
        assert_eq!(result, 3);
    }

    #[tokio::test]
    async fn test_call_with_auth_retry_on_unauthenticated() {
        let c = MockRetryableClient::new();
        let call_count = Arc::new(AtomicI32::new(0));
        let cc = call_count.clone();

        let result = call_with_auth_retry(&c, |_auth_ctx| {
            let cc = cc.clone();
            async move {
                let count = cc.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    Err(tonic::Status::new(
                        tonic::Code::Unauthenticated,
                        "Not authenticated",
                    ))
                } else {
                    Ok::<_, tonic::Status>(3)
                }
            }
        })
        .await
        .unwrap();

        assert_eq!(c.auth_context_call_count.load(Ordering::SeqCst), 2);
        assert_eq!(c.refresh_jwt_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(result, 3);
    }

    #[tokio::test]
    async fn test_call_with_auth_retry_non_unauthenticated() {
        let c = MockRetryableClient::new();

        let result: Result<i32, _> = call_with_auth_retry(&c, |_auth_ctx| async move {
            Err(tonic::Status::new(
                tonic::Code::InvalidArgument,
                "Invalid argument",
            ))
        })
        .await;

        assert!(result.is_err());
        assert_eq!(c.auth_context_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(c.refresh_jwt_call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_call_with_auth_retry_unauthenticated_after_retry() {
        let c = MockRetryableClient::new();

        let result: Result<i32, _> = call_with_auth_retry(&c, |_auth_ctx| async move {
            Err(tonic::Status::new(
                tonic::Code::Unauthenticated,
                "Not authenticated",
            ))
        })
        .await;

        assert!(result.is_err());
        assert_eq!(c.auth_context_call_count.load(Ordering::SeqCst), 2);
        assert_eq!(c.refresh_jwt_call_count.load(Ordering::SeqCst), 1);
    }
}
