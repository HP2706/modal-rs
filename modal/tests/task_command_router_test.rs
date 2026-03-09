#![cfg(feature = "integration")]

/// Integration tests for Modal TaskCommandRouter module.
/// Translated from libmodal/modal-go/task_command_router_client_test.go

use modal::auth_token_manager::parse_jwt_expiration;
use modal::error::ModalError;
use modal::task_command_router::{
    call_with_auth_retry, call_with_retries_on_transient_errors, RetryOptions, RetryableClient,
};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

// ── JWT parsing tests ────────────────────────────────────────────────────

fn mock_jwt(exp: Option<serde_json::Value>) -> String {
    use base64::Engine;
    let header =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"{\"alg\":\"HS256\",\"typ\":\"JWT\"}");
    let payload_map = match exp {
        Some(v) => serde_json::json!({"exp": v}),
        None => serde_json::json!({}),
    };
    let payload =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_map.to_string().as_bytes());
    format!("{}.{}.fake-signature", header, payload)
}

#[test]
fn test_parse_jwt_expiration_with_valid_jwt() {
    let exp = chrono::Utc::now().timestamp() + 3600;
    let jwt = mock_jwt(Some(serde_json::json!(exp)));
    let result = parse_jwt_expiration(&jwt).unwrap();
    assert_eq!(result, Some(exp));
}

#[test]
fn test_parse_jwt_expiration_without_exp_claim() {
    let jwt = mock_jwt(None);
    let result = parse_jwt_expiration(&jwt).unwrap();
    assert_eq!(result, None);
}

#[test]
fn test_parse_jwt_expiration_malformed_jwt() {
    let result = parse_jwt_expiration("only.two");
    assert!(result.is_err());
}

#[test]
fn test_parse_jwt_expiration_invalid_base64() {
    let result = parse_jwt_expiration("invalid.!!!invalid!!!.signature");
    assert!(result.is_err());
}

#[test]
fn test_parse_jwt_expiration_non_numeric_exp() {
    let jwt = mock_jwt(Some(serde_json::json!("not-a-number")));
    let result = parse_jwt_expiration(&jwt);
    assert!(result.is_err());
}

// ── Retry on transient errors tests ──────────────────────────────────────

#[tokio::test]
async fn test_retries_success_on_first_attempt() {
    let call_count = AtomicU32::new(0);
    let result = call_with_retries_on_transient_errors(
        || {
            call_count.fetch_add(1, Ordering::SeqCst);
            async { Ok::<_, tonic::Status>("success".to_string()) }
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
async fn test_retries_on_transient_codes() {
    let transient_codes = vec![
        tonic::Code::DeadlineExceeded,
        tonic::Code::Unavailable,
        tonic::Code::Cancelled,
        tonic::Code::Internal,
        tonic::Code::Unknown,
    ];

    for code in transient_codes {
        let call_count = AtomicU32::new(0);
        let result = call_with_retries_on_transient_errors(
            || {
                let count = call_count.fetch_add(1, Ordering::SeqCst);
                async move {
                    if count == 0 {
                        Err(tonic::Status::new(code, "transient"))
                    } else {
                        Ok("success".to_string())
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

        assert_eq!(result, "success", "code: {:?}", code);
        assert_eq!(call_count.load(Ordering::SeqCst), 2, "code: {:?}", code);
    }
}

#[tokio::test]
async fn test_retries_non_retryable_error() {
    let call_count = AtomicU32::new(0);
    let result = call_with_retries_on_transient_errors(
        || {
            call_count.fetch_add(1, Ordering::SeqCst);
            async { Err::<String, _>(tonic::Status::invalid_argument("invalid")) }
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
async fn test_retries_max_retries_exceeded() {
    let call_count = AtomicU32::new(0);
    let max_retries = 3;
    let result = call_with_retries_on_transient_errors(
        || {
            call_count.fetch_add(1, Ordering::SeqCst);
            async { Err::<String, _>(tonic::Status::unavailable("unavailable")) }
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
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        (max_retries + 1) as u32
    );
}

#[tokio::test]
async fn test_retries_deadline_exceeded() {
    let deadline = Instant::now() + Duration::from_millis(50);
    let result = call_with_retries_on_transient_errors(
        || async { Err::<String, _>(tonic::Status::unavailable("unavailable")) },
        RetryOptions {
            base_delay: Duration::from_millis(100),
            delay_factor: 1.0,
            max_retries: None,
            deadline: Some(deadline),
        },
        None,
    )
    .await;

    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("deadline exceeded"),
        "got: {}",
        err
    );
}

#[tokio::test]
async fn test_retries_closed_client() {
    let closed = AtomicBool::new(true);
    let result = call_with_retries_on_transient_errors(
        || async { Err::<String, _>(tonic::Status::cancelled("cancelled")) },
        RetryOptions::default(),
        Some(&closed),
    )
    .await;

    let err = result.unwrap_err();
    assert!(
        err.to_string()
            .contains("Unable to perform operation on a detached sandbox"),
        "got: {}",
        err
    );
}

// ── Auth retry tests ─────────────────────────────────────────────────────

struct MockRetryableClient {
    refresh_jwt_count: AtomicU32,
    auth_context_count: AtomicU32,
}

impl MockRetryableClient {
    fn new() -> Self {
        Self {
            refresh_jwt_count: AtomicU32::new(0),
            auth_context_count: AtomicU32::new(0),
        }
    }
}

impl RetryableClient for MockRetryableClient {
    fn auth_context(&self) -> u32 {
        self.auth_context_count.fetch_add(1, Ordering::SeqCst);
        0
    }

    fn refresh_jwt(&self) -> Result<(), ModalError> {
        self.refresh_jwt_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn test_auth_retry_success_first_attempt() {
    let client = MockRetryableClient::new();
    let result = call_with_auth_retry(&client, |_ctx| async { Ok::<i32, _>(3) })
        .await
        .unwrap();

    assert_eq!(result, 3);
    assert_eq!(client.auth_context_count.load(Ordering::SeqCst), 1);
    assert_eq!(client.refresh_jwt_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_auth_retry_on_unauthenticated() {
    let client = MockRetryableClient::new();
    let call_count = AtomicU32::new(0);

    let result = call_with_auth_retry(&client, |_ctx| {
        let count = call_count.fetch_add(1, Ordering::SeqCst);
        async move {
            if count == 0 {
                Err(tonic::Status::unauthenticated("Not authenticated"))
            } else {
                Ok(3i32)
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(result, 3);
    assert_eq!(client.auth_context_count.load(Ordering::SeqCst), 2);
    assert_eq!(client.refresh_jwt_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_auth_retry_does_not_retry_non_unauthenticated() {
    let client = MockRetryableClient::new();
    let result =
        call_with_auth_retry(&client, |_ctx| async {
            Err::<i32, _>(tonic::Status::invalid_argument("Invalid argument"))
        })
        .await;

    assert!(result.is_err());
    assert_eq!(client.auth_context_count.load(Ordering::SeqCst), 1);
    assert_eq!(client.refresh_jwt_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_auth_retry_fails_if_still_unauthenticated_after_retry() {
    let client = MockRetryableClient::new();
    let result = call_with_auth_retry(&client, |_ctx| async {
        Err::<i32, _>(tonic::Status::unauthenticated("Not authenticated"))
    })
    .await;

    assert!(result.is_err());
    assert_eq!(client.auth_context_count.load(Ordering::SeqCst), 2);
    assert_eq!(client.refresh_jwt_count.load(Ordering::SeqCst), 1);
}
