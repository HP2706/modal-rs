use std::sync::Arc;
use std::time::Duration;

use crate::config::{environment_name, Profile};
use crate::error::ModalError;

const QUEUE_DEFAULT_PARTITION_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Queue is a distributed, FIFO queue for data flow in Modal Apps.
#[derive(Debug, Clone)]
pub struct Queue {
    pub queue_id: String,
    pub name: String,
    cancel_ephemeral: Option<Arc<tokio::sync::Notify>>,
}

impl Queue {
    /// Create a new Queue with the given ID and name.
    pub fn new(queue_id: String, name: String) -> Self {
        Self {
            queue_id,
            name,
            cancel_ephemeral: None,
        }
    }

    /// CloseEphemeral deletes an ephemeral Queue by cancelling its heartbeat.
    /// Panics if the Queue is not ephemeral.
    pub fn close_ephemeral(&self) {
        match &self.cancel_ephemeral {
            Some(notify) => notify.notify_one(),
            None => panic!("Queue {} is not ephemeral", self.queue_id),
        }
    }

    /// Returns true if this queue is ephemeral.
    pub fn is_ephemeral(&self) -> bool {
        self.cancel_ephemeral.is_some()
    }
}

/// Validate a partition key (1-64 bytes, or empty for default partition).
pub fn validate_partition_key(partition: &str) -> Result<Option<Vec<u8>>, ModalError> {
    if partition.is_empty() {
        return Ok(None);
    }
    let b = partition.as_bytes();
    if b.is_empty() || b.len() > 64 {
        return Err(ModalError::Invalid(
            "Queue partition key must be 1–64 bytes long".to_string(),
        ));
    }
    Ok(Some(b.to_vec()))
}

/// QueueFromNameParams are options for client.queues.from_name.
#[derive(Debug, Clone, Default)]
pub struct QueueFromNameParams {
    pub environment: String,
    pub create_if_missing: bool,
}

/// QueueEphemeralParams are options for client.queues.ephemeral.
#[derive(Debug, Clone, Default)]
pub struct QueueEphemeralParams {
    pub environment: String,
}

/// QueueDeleteParams are options for client.queues.delete.
#[derive(Debug, Clone, Default)]
pub struct QueueDeleteParams {
    pub environment: String,
    pub allow_missing: bool,
}

/// QueueClearParams are options for Queue.clear.
#[derive(Debug, Clone, Default)]
pub struct QueueClearParams {
    pub partition: String,
    pub all: bool,
}

/// QueueGetParams are options for Queue.get.
#[derive(Debug, Clone, Default)]
pub struct QueueGetParams {
    pub timeout: Option<Duration>,
    pub partition: String,
}

/// QueueGetManyParams are options for Queue.get_many.
#[derive(Debug, Clone, Default)]
pub struct QueueGetManyParams {
    pub timeout: Option<Duration>,
    pub partition: String,
}

/// QueuePutParams are options for Queue.put.
#[derive(Debug, Clone, Default)]
pub struct QueuePutParams {
    pub timeout: Option<Duration>,
    pub partition: String,
    pub partition_ttl: Duration,
}

impl QueuePutParams {
    /// Returns the effective partition TTL (default 24h).
    pub fn effective_partition_ttl(&self) -> Duration {
        if self.partition_ttl == Duration::ZERO {
            QUEUE_DEFAULT_PARTITION_TTL
        } else {
            self.partition_ttl
        }
    }
}

/// QueuePutManyParams are options for Queue.put_many.
#[derive(Debug, Clone, Default)]
pub struct QueuePutManyParams {
    pub timeout: Option<Duration>,
    pub partition: String,
    pub partition_ttl: Duration,
}

/// QueueLenParams are options for Queue.len.
#[derive(Debug, Clone, Default)]
pub struct QueueLenParams {
    pub partition: String,
    pub total: bool,
}

/// QueueService provides Queue related operations.
pub trait QueueService: Send + Sync {
    fn from_name(
        &self,
        name: &str,
        params: Option<&QueueFromNameParams>,
    ) -> Result<Queue, ModalError>;

    fn ephemeral(&self, params: Option<&QueueEphemeralParams>) -> Result<Queue, ModalError>;

    fn delete(&self, name: &str, params: Option<&QueueDeleteParams>) -> Result<(), ModalError>;
}

/// Trait abstracting the gRPC calls needed for Queue operations.
pub trait QueueGrpcClient: Send + Sync {
    fn queue_get_or_create(
        &self,
        deployment_name: &str,
        environment_name: &str,
        object_creation_type: i32,
    ) -> Result<String, ModalError>;

    fn queue_heartbeat(&self, queue_id: &str) -> Result<(), ModalError>;

    fn queue_delete(&self, queue_id: &str) -> Result<(), ModalError>;

    fn queue_clear(
        &self,
        queue_id: &str,
        partition_key: Option<&[u8]>,
        all_partitions: bool,
    ) -> Result<(), ModalError>;

    fn queue_len(
        &self,
        queue_id: &str,
        partition_key: Option<&[u8]>,
        total: bool,
    ) -> Result<i32, ModalError>;
}

/// Implementation of QueueService backed by a gRPC client.
pub struct QueueServiceImpl<C: QueueGrpcClient> {
    pub client: C,
    pub profile: Profile,
}

impl<C: QueueGrpcClient> QueueService for QueueServiceImpl<C> {
    fn from_name(
        &self,
        name: &str,
        params: Option<&QueueFromNameParams>,
    ) -> Result<Queue, ModalError> {
        let default_params = QueueFromNameParams::default();
        let params = params.unwrap_or(&default_params);

        let creation_type = if params.create_if_missing {
            1 // OBJECT_CREATION_TYPE_CREATE_IF_MISSING
        } else {
            0 // OBJECT_CREATION_TYPE_UNSPECIFIED
        };

        let env = environment_name(&params.environment, &self.profile);

        let queue_id =
            self.client
                .queue_get_or_create(name, &env, creation_type)
                .map_err(|e| {
                    if is_not_found_error(&e) {
                        ModalError::NotFound(format!("Queue '{}' not found", name))
                    } else {
                        e
                    }
                })?;

        Ok(Queue::new(queue_id, name.to_string()))
    }

    fn ephemeral(&self, params: Option<&QueueEphemeralParams>) -> Result<Queue, ModalError> {
        let default_params = QueueEphemeralParams::default();
        let params = params.unwrap_or(&default_params);

        let env = environment_name(&params.environment, &self.profile);

        let queue_id = self.client.queue_get_or_create(
            "",
            &env,
            3, // OBJECT_CREATION_TYPE_EPHEMERAL
        )?;

        let notify = Arc::new(tokio::sync::Notify::new());

        Ok(Queue {
            queue_id,
            name: String::new(),
            cancel_ephemeral: Some(notify),
        })
    }

    fn delete(&self, name: &str, params: Option<&QueueDeleteParams>) -> Result<(), ModalError> {
        let default_params = QueueDeleteParams::default();
        let params = params.unwrap_or(&default_params);

        let queue = self.from_name(
            name,
            Some(&QueueFromNameParams {
                environment: params.environment.clone(),
                create_if_missing: false,
            }),
        );

        let queue = match queue {
            Ok(q) => q,
            Err(e) => {
                if is_not_found_error(&e) && params.allow_missing {
                    return Ok(());
                }
                return Err(e);
            }
        };

        match self.client.queue_delete(&queue.queue_id) {
            Ok(()) => Ok(()),
            Err(e) => {
                if is_grpc_not_found(&e) && params.allow_missing {
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }
}

/// Queue instance methods that require a gRPC client.
impl Queue {
    /// Clear removes all objects from a Queue partition.
    pub fn clear<C: QueueGrpcClient>(
        &self,
        client: &C,
        params: Option<&QueueClearParams>,
    ) -> Result<(), ModalError> {
        let default_params = QueueClearParams::default();
        let params = params.unwrap_or(&default_params);

        if !params.partition.is_empty() && params.all {
            return Err(ModalError::Invalid(
                "Partition must be \"\" when clearing all partitions".to_string(),
            ));
        }

        let key = validate_partition_key(&params.partition)?;
        client.queue_clear(&self.queue_id, key.as_deref(), params.all)
    }

    /// Len returns the number of objects in the Queue.
    pub fn len<C: QueueGrpcClient>(
        &self,
        client: &C,
        params: Option<&QueueLenParams>,
    ) -> Result<i32, ModalError> {
        let default_params = QueueLenParams::default();
        let params = params.unwrap_or(&default_params);

        if !params.partition.is_empty() && params.total {
            return Err(ModalError::Invalid(
                "partition must be empty when requesting total length".to_string(),
            ));
        }

        let key = validate_partition_key(&params.partition)?;
        client.queue_len(&self.queue_id, key.as_deref(), params.total)
    }
}

fn is_not_found_error(err: &ModalError) -> bool {
    match err {
        ModalError::NotFound(_) => true,
        ModalError::Grpc(status) => status.code() == tonic::Code::NotFound,
        _ => false,
    }
}

fn is_grpc_not_found(err: &ModalError) -> bool {
    matches!(err, ModalError::Grpc(s) if s.code() == tonic::Code::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockQueueGrpcClient {
        responses: Mutex<Vec<MockResponse>>,
    }

    enum MockResponse {
        GetOrCreate(Result<String, ModalError>),
        Delete(Result<(), ModalError>),
        Clear(Result<(), ModalError>),
        Len(Result<i32, ModalError>),
    }

    impl MockQueueGrpcClient {
        fn new() -> Self {
            Self {
                responses: Mutex::new(Vec::new()),
            }
        }

        fn push_get_or_create(&self, result: Result<String, ModalError>) {
            self.responses
                .lock()
                .unwrap()
                .push(MockResponse::GetOrCreate(result));
        }

        fn push_delete(&self, result: Result<(), ModalError>) {
            self.responses
                .lock()
                .unwrap()
                .push(MockResponse::Delete(result));
        }

        fn push_clear(&self, result: Result<(), ModalError>) {
            self.responses
                .lock()
                .unwrap()
                .push(MockResponse::Clear(result));
        }

        fn push_len(&self, result: Result<i32, ModalError>) {
            self.responses
                .lock()
                .unwrap()
                .push(MockResponse::Len(result));
        }
    }

    impl QueueGrpcClient for MockQueueGrpcClient {
        fn queue_get_or_create(
            &self,
            _deployment_name: &str,
            _environment_name: &str,
            _object_creation_type: i32,
        ) -> Result<String, ModalError> {
            let mut responses = self.responses.lock().unwrap();
            match responses.remove(0) {
                MockResponse::GetOrCreate(r) => r,
                _ => panic!("unexpected mock response type"),
            }
        }

        fn queue_heartbeat(&self, _queue_id: &str) -> Result<(), ModalError> {
            Ok(())
        }

        fn queue_delete(&self, _queue_id: &str) -> Result<(), ModalError> {
            let mut responses = self.responses.lock().unwrap();
            match responses.remove(0) {
                MockResponse::Delete(r) => r,
                _ => panic!("unexpected mock response type"),
            }
        }

        fn queue_clear(
            &self,
            _queue_id: &str,
            _partition_key: Option<&[u8]>,
            _all_partitions: bool,
        ) -> Result<(), ModalError> {
            let mut responses = self.responses.lock().unwrap();
            match responses.remove(0) {
                MockResponse::Clear(r) => r,
                _ => panic!("unexpected mock response type"),
            }
        }

        fn queue_len(
            &self,
            _queue_id: &str,
            _partition_key: Option<&[u8]>,
            _total: bool,
        ) -> Result<i32, ModalError> {
            let mut responses = self.responses.lock().unwrap();
            match responses.remove(0) {
                MockResponse::Len(r) => r,
                _ => panic!("unexpected mock response type"),
            }
        }
    }

    fn make_service(mock: MockQueueGrpcClient) -> QueueServiceImpl<MockQueueGrpcClient> {
        QueueServiceImpl {
            client: mock,
            profile: Profile::default(),
        }
    }

    #[test]
    fn test_queue_from_name() {
        let mock = MockQueueGrpcClient::new();
        mock.push_get_or_create(Ok("qu-test-123".to_string()));
        let svc = make_service(mock);

        let queue = svc
            .from_name(
                "my-queue",
                Some(&QueueFromNameParams {
                    create_if_missing: true,
                    ..Default::default()
                }),
            )
            .unwrap();

        assert_eq!(queue.queue_id, "qu-test-123");
        assert_eq!(queue.name, "my-queue");
    }

    #[test]
    fn test_queue_from_name_not_found() {
        let mock = MockQueueGrpcClient::new();
        mock.push_get_or_create(Err(ModalError::Grpc(tonic::Status::not_found(
            "not found",
        ))));
        let svc = make_service(mock);

        let err = svc.from_name("missing-queue", None).unwrap_err();
        assert!(err.to_string().contains("Queue 'missing-queue' not found"));
    }

    #[test]
    fn test_queue_ephemeral() {
        let mock = MockQueueGrpcClient::new();
        mock.push_get_or_create(Ok("qu-ephemeral-456".to_string()));
        let svc = make_service(mock);

        let queue = svc.ephemeral(None).unwrap();
        assert_eq!(queue.name, "");
        assert!(queue.is_ephemeral());
        assert_eq!(queue.queue_id, "qu-ephemeral-456");
    }

    #[test]
    #[should_panic(expected = "is not ephemeral")]
    fn test_queue_close_ephemeral_panics_on_non_ephemeral() {
        let queue = Queue::new("qu-123".to_string(), "test".to_string());
        queue.close_ephemeral();
    }

    #[test]
    fn test_queue_delete_success() {
        let mock = MockQueueGrpcClient::new();
        mock.push_get_or_create(Ok("qu-test-123".to_string()));
        mock.push_delete(Ok(()));
        let svc = make_service(mock);

        svc.delete("test-queue", None).unwrap();
    }

    #[test]
    fn test_queue_delete_allow_missing() {
        let mock = MockQueueGrpcClient::new();
        mock.push_get_or_create(Err(ModalError::NotFound(
            "Queue 'missing' not found".to_string(),
        )));
        let svc = make_service(mock);

        svc.delete(
            "missing",
            Some(&QueueDeleteParams {
                allow_missing: true,
                ..Default::default()
            }),
        )
        .unwrap();
    }

    #[test]
    fn test_queue_clear() {
        let mock = MockQueueGrpcClient::new();
        mock.push_clear(Ok(()));
        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());

        queue.clear(&mock, None).unwrap();
    }

    #[test]
    fn test_queue_clear_all_with_partition_errors() {
        let mock = MockQueueGrpcClient::new();
        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());

        let err = queue
            .clear(
                &mock,
                Some(&QueueClearParams {
                    partition: "my-partition".to_string(),
                    all: true,
                }),
            )
            .unwrap_err();
        assert!(err.to_string().contains("Partition must be"));
    }

    #[test]
    fn test_queue_len() {
        let mock = MockQueueGrpcClient::new();
        mock.push_len(Ok(42));
        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());

        let len = queue.len(&mock, None).unwrap();
        assert_eq!(len, 42);
    }

    #[test]
    fn test_queue_len_total_with_partition_errors() {
        let mock = MockQueueGrpcClient::new();
        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());

        let err = queue
            .len(
                &mock,
                Some(&QueueLenParams {
                    partition: "my-partition".to_string(),
                    total: true,
                }),
            )
            .unwrap_err();
        assert!(err.to_string().contains("partition must be empty"));
    }

    #[test]
    fn test_validate_partition_key_empty() {
        assert!(validate_partition_key("").unwrap().is_none());
    }

    #[test]
    fn test_validate_partition_key_valid() {
        let key = validate_partition_key("my-partition").unwrap().unwrap();
        assert_eq!(key, b"my-partition");
    }

    #[test]
    fn test_validate_partition_key_too_long() {
        let long_key = "a".repeat(65);
        let err = validate_partition_key(&long_key).unwrap_err();
        assert!(err.to_string().contains("1–64 bytes long"));
    }

    #[test]
    fn test_queue_put_params_effective_ttl() {
        let params = QueuePutParams::default();
        assert_eq!(params.effective_partition_ttl(), QUEUE_DEFAULT_PARTITION_TTL);

        let params = QueuePutParams {
            partition_ttl: Duration::from_secs(3600),
            ..Default::default()
        };
        assert_eq!(params.effective_partition_ttl(), Duration::from_secs(3600));
    }
}
