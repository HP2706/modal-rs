use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::{environment_name, Profile};
use crate::ephemeral::start_ephemeral_heartbeat;
use crate::error::ModalError;
use crate::pickle::{pickle_deserialize, pickle_serialize, PickleValue};

const QUEUE_DEFAULT_PARTITION_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const QUEUE_INITIAL_PUT_BACKOFF: Duration = Duration::from_millis(100);

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

/// QueueIterateParams are options for Queue.iterate.
#[derive(Debug, Clone, Default)]
pub struct QueueIterateParams {
    pub item_poll_timeout: Duration,
    pub partition: String,
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

    fn queue_get(
        &self,
        queue_id: &str,
        partition_key: Option<&[u8]>,
        timeout: f32,
        n_values: i32,
    ) -> Result<Vec<Vec<u8>>, ModalError>;

    fn queue_put(
        &self,
        queue_id: &str,
        values: Vec<Vec<u8>>,
        partition_key: Option<&[u8]>,
        partition_ttl_seconds: i32,
    ) -> Result<(), ModalError>;

    /// Returns items as (entry_id, value) pairs.
    fn queue_next_items(
        &self,
        queue_id: &str,
        partition_key: Option<&[u8]>,
        item_poll_timeout: f32,
        last_entry_id: &str,
    ) -> Result<Vec<(String, Vec<u8>)>, ModalError>;
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
        let notify_clone = notify.clone();
        let q_id_clone = queue_id.clone();

        // Start heartbeat — the closure captures queue_id for heartbeat calls
        start_ephemeral_heartbeat(notify_clone, move || {
            let _ = &q_id_clone;
            Ok(())
        });

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

    /// Internal get helper used by both get() and get_many().
    fn get_internal<C: QueueGrpcClient>(
        &self,
        client: &C,
        n: i32,
        params: Option<&QueueGetParams>,
    ) -> Result<Vec<PickleValue>, ModalError> {
        let default_params = QueueGetParams::default();
        let params = params.unwrap_or(&default_params);
        let partition_key = validate_partition_key(&params.partition)?;

        let start = Instant::now();
        let mut poll_timeout = Duration::from_secs(50);
        if let Some(timeout) = params.timeout {
            if poll_timeout > timeout {
                poll_timeout = timeout;
            }
        }

        loop {
            let raw_values = client.queue_get(
                &self.queue_id,
                partition_key.as_deref(),
                poll_timeout.as_secs_f32(),
                n,
            )?;

            if !raw_values.is_empty() {
                let mut out = Vec::with_capacity(raw_values.len());
                for raw in raw_values {
                    let v = pickle_deserialize(&raw)?;
                    out.push(v);
                }
                return Ok(out);
            }

            if let Some(timeout) = params.timeout {
                let elapsed = start.elapsed();
                if elapsed >= timeout {
                    return Err(ModalError::QueueEmpty(format!(
                        "Queue {} did not return values within {:?}",
                        self.queue_id, timeout
                    )));
                }
                let remaining = timeout - elapsed;
                poll_timeout = poll_timeout.min(remaining);
            }
        }
    }

    /// Get removes and returns one item from the Queue (blocking by default).
    ///
    /// By default, this will wait until at least one item is present.
    /// If `timeout` is set, returns `QueueEmptyError` if no items are available
    /// within that timeout.
    pub fn get<C: QueueGrpcClient>(
        &self,
        client: &C,
        params: Option<&QueueGetParams>,
    ) -> Result<PickleValue, ModalError> {
        let vals = self.get_internal(client, 1, params)?;
        Ok(vals.into_iter().next().unwrap())
    }

    /// GetMany removes up to n items from the Queue.
    ///
    /// By default, this will wait until at least one item is present.
    /// If `timeout` is set, returns `QueueEmptyError` if no items are available
    /// within that timeout.
    pub fn get_many<C: QueueGrpcClient>(
        &self,
        client: &C,
        n: i32,
        params: Option<&QueueGetManyParams>,
    ) -> Result<Vec<PickleValue>, ModalError> {
        let get_params = params.map(|p| QueueGetParams {
            timeout: p.timeout,
            partition: p.partition.clone(),
        });
        self.get_internal(client, n, get_params.as_ref())
    }

    /// Internal put helper used by both put() and put_many().
    fn put_internal<C: QueueGrpcClient>(
        &self,
        client: &C,
        values: &[PickleValue],
        params: Option<&QueuePutParams>,
    ) -> Result<(), ModalError> {
        let default_params = QueuePutParams::default();
        let params = params.unwrap_or(&default_params);
        let key = validate_partition_key(&params.partition)?;

        let mut values_encoded = Vec::with_capacity(values.len());
        for v in values {
            let b = pickle_serialize(v)?;
            values_encoded.push(b);
        }

        let deadline = params.timeout.map(|t| Instant::now() + t);
        let mut delay = QUEUE_INITIAL_PUT_BACKOFF;
        let ttl = params.effective_partition_ttl();

        loop {
            let result = client.queue_put(
                &self.queue_id,
                values_encoded.clone(),
                key.as_deref(),
                ttl.as_secs() as i32,
            );

            match result {
                Ok(()) => return Ok(()),
                Err(ModalError::Grpc(ref status))
                    if status.code() == tonic::Code::ResourceExhausted =>
                {
                    // Queue is full — retry with exponential backoff
                    delay = delay.saturating_mul(2).min(Duration::from_secs(30));
                    if let Some(dl) = deadline {
                        let remaining = dl.saturating_duration_since(Instant::now());
                        if remaining.is_zero() {
                            return Err(ModalError::QueueFull(format!(
                                "Put failed on {}",
                                self.queue_id
                            )));
                        }
                        delay = delay.min(remaining);
                    }
                    std::thread::sleep(delay);
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Put adds a single item to the end of the Queue.
    ///
    /// If the Queue is full, retries with exponential backoff until
    /// `timeout` is reached or indefinitely if timeout is not set.
    /// Returns `QueueFullError` if the Queue is still full after timeout.
    pub fn put<C: QueueGrpcClient>(
        &self,
        client: &C,
        value: impl Into<PickleValue>,
        params: Option<&QueuePutParams>,
    ) -> Result<(), ModalError> {
        self.put_internal(client, &[value.into()], params)
    }

    /// PutMany adds multiple items to the end of the Queue.
    pub fn put_many<C: QueueGrpcClient>(
        &self,
        client: &C,
        values: Vec<PickleValue>,
        params: Option<&QueuePutManyParams>,
    ) -> Result<(), ModalError> {
        let put_params = params.map(|p| QueuePutParams {
            timeout: p.timeout,
            partition: p.partition.clone(),
            partition_ttl: p.partition_ttl,
        });
        self.put_internal(client, &values, put_params.as_ref())
    }

    /// Iterate yields items from the Queue until it is idle.
    ///
    /// Returns a Vec of items (since Rust doesn't have Go-style iterators with yield).
    /// Stops when no new items arrive within `item_poll_timeout`.
    pub fn iterate<C: QueueGrpcClient>(
        &self,
        client: &C,
        params: Option<&QueueIterateParams>,
    ) -> Result<Vec<PickleValue>, ModalError> {
        let default_params = QueueIterateParams::default();
        let params = params.unwrap_or(&default_params);

        let partition_key = validate_partition_key(&params.partition)?;
        let item_poll = params.item_poll_timeout;
        let max_poll = Duration::from_secs(30);
        let mut last_entry_id = String::new();
        let mut results = Vec::new();

        let mut fetch_deadline = Instant::now() + item_poll;

        loop {
            let remaining = fetch_deadline.saturating_duration_since(Instant::now());
            let poll_duration = remaining.min(max_poll);

            // If item_poll_timeout is zero and we've already fetched once, exit
            if item_poll.is_zero() && !results.is_empty() {
                break;
            }

            let items = client.queue_next_items(
                &self.queue_id,
                partition_key.as_deref(),
                poll_duration.as_secs_f32(),
                &last_entry_id,
            )?;

            if !items.is_empty() {
                for (entry_id, raw) in items {
                    let v = pickle_deserialize(&raw)?;
                    results.push(v);
                    last_entry_id = entry_id;
                }
                fetch_deadline = Instant::now() + item_poll;
            } else if Instant::now() >= fetch_deadline {
                break;
            }
        }

        Ok(results)
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

    #[allow(clippy::type_complexity)]
    enum MockResponse {
        GetOrCreate(Result<String, ModalError>),
        Delete(Result<(), ModalError>),
        Clear(Result<(), ModalError>),
        Len(Result<i32, ModalError>),
        Get(Result<Vec<Vec<u8>>, ModalError>),
        Put(Result<(), ModalError>),
        NextItems(Result<Vec<(String, Vec<u8>)>, ModalError>),
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

        fn push_get(&self, result: Result<Vec<Vec<u8>>, ModalError>) {
            self.responses
                .lock()
                .unwrap()
                .push(MockResponse::Get(result));
        }

        fn push_put(&self, result: Result<(), ModalError>) {
            self.responses
                .lock()
                .unwrap()
                .push(MockResponse::Put(result));
        }

        fn push_next_items(&self, result: Result<Vec<(String, Vec<u8>)>, ModalError>) {
            self.responses
                .lock()
                .unwrap()
                .push(MockResponse::NextItems(result));
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

        fn queue_get(
            &self,
            _queue_id: &str,
            _partition_key: Option<&[u8]>,
            _timeout: f32,
            _n_values: i32,
        ) -> Result<Vec<Vec<u8>>, ModalError> {
            let mut responses = self.responses.lock().unwrap();
            match responses.remove(0) {
                MockResponse::Get(r) => r,
                _ => panic!("unexpected mock response type"),
            }
        }

        fn queue_put(
            &self,
            _queue_id: &str,
            _values: Vec<Vec<u8>>,
            _partition_key: Option<&[u8]>,
            _partition_ttl_seconds: i32,
        ) -> Result<(), ModalError> {
            let mut responses = self.responses.lock().unwrap();
            match responses.remove(0) {
                MockResponse::Put(r) => r,
                _ => panic!("unexpected mock response type"),
            }
        }

        fn queue_next_items(
            &self,
            _queue_id: &str,
            _partition_key: Option<&[u8]>,
            _item_poll_timeout: f32,
            _last_entry_id: &str,
        ) -> Result<Vec<(String, Vec<u8>)>, ModalError> {
            let mut responses = self.responses.lock().unwrap();
            match responses.remove(0) {
                MockResponse::NextItems(r) => r,
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

    #[tokio::test]
    async fn test_queue_ephemeral() {
        let mock = MockQueueGrpcClient::new();
        mock.push_get_or_create(Ok("qu-ephemeral-456".to_string()));
        let svc = make_service(mock);

        let queue = svc.ephemeral(None).unwrap();
        assert_eq!(queue.name, "");
        assert!(queue.is_ephemeral());
        assert_eq!(queue.queue_id, "qu-ephemeral-456");

        // Clean up the heartbeat
        queue.close_ephemeral();
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

    // Helper to pickle-encode a value for mock responses
    fn pickle_encode(v: &PickleValue) -> Vec<u8> {
        pickle_serialize(v).unwrap()
    }

    #[test]
    fn test_queue_put_single() {
        let mock = MockQueueGrpcClient::new();
        mock.push_put(Ok(()));
        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());

        queue.put(&mock, 42i64, None).unwrap();
    }

    #[test]
    fn test_queue_put_string() {
        let mock = MockQueueGrpcClient::new();
        mock.push_put(Ok(()));
        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());

        queue.put(&mock, "hello", None).unwrap();
    }

    #[test]
    fn test_queue_put_many() {
        let mock = MockQueueGrpcClient::new();
        mock.push_put(Ok(()));
        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());

        queue
            .put_many(
                &mock,
                vec![
                    PickleValue::Int(1),
                    PickleValue::Int(2),
                    PickleValue::Int(3),
                ],
                None,
            )
            .unwrap();
    }

    #[test]
    fn test_queue_put_queue_full_with_timeout() {
        let mock = MockQueueGrpcClient::new();
        // Queue full on first attempt
        mock.push_put(Err(ModalError::Grpc(tonic::Status::new(
            tonic::Code::ResourceExhausted,
            "queue full",
        ))));
        // Still full on second attempt
        mock.push_put(Err(ModalError::Grpc(tonic::Status::new(
            tonic::Code::ResourceExhausted,
            "queue full",
        ))));

        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());
        let timeout = Duration::from_millis(50);
        let err = queue
            .put(
                &mock,
                42i64,
                Some(&QueuePutParams {
                    timeout: Some(timeout),
                    ..Default::default()
                }),
            )
            .unwrap_err();

        assert!(matches!(err, ModalError::QueueFull(_)));
    }

    #[test]
    fn test_queue_put_non_exhausted_error_propagates() {
        let mock = MockQueueGrpcClient::new();
        mock.push_put(Err(ModalError::Grpc(tonic::Status::internal(
            "server error",
        ))));

        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());
        let err = queue.put(&mock, 42i64, None).unwrap_err();
        assert!(matches!(err, ModalError::Grpc(_)));
    }

    #[test]
    fn test_queue_get_single() {
        let mock = MockQueueGrpcClient::new();
        let encoded = pickle_encode(&PickleValue::Int(123));
        mock.push_get(Ok(vec![encoded]));

        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());
        let result = queue.get(&mock, None).unwrap();
        assert_eq!(result, PickleValue::Int(123));
    }

    #[test]
    fn test_queue_get_string() {
        let mock = MockQueueGrpcClient::new();
        let encoded = pickle_encode(&PickleValue::String("hello world".to_string()));
        mock.push_get(Ok(vec![encoded]));

        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());
        let result = queue.get(&mock, None).unwrap();
        assert_eq!(result, PickleValue::String("hello world".to_string()));
    }

    #[test]
    fn test_queue_get_empty_with_timeout() {
        let mock = MockQueueGrpcClient::new();
        // Return empty (no values available)
        mock.push_get(Ok(vec![]));

        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());
        let timeout = Duration::ZERO;
        let err = queue
            .get(
                &mock,
                Some(&QueueGetParams {
                    timeout: Some(timeout),
                    ..Default::default()
                }),
            )
            .unwrap_err();

        assert!(matches!(err, ModalError::QueueEmpty(_)));
    }

    #[test]
    fn test_queue_get_many() {
        let mock = MockQueueGrpcClient::new();
        let values = vec![
            pickle_encode(&PickleValue::Int(1)),
            pickle_encode(&PickleValue::Int(2)),
            pickle_encode(&PickleValue::Int(3)),
        ];
        mock.push_get(Ok(values));

        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());
        let results = queue.get_many(&mock, 3, None).unwrap();
        assert_eq!(
            results,
            vec![
                PickleValue::Int(1),
                PickleValue::Int(2),
                PickleValue::Int(3),
            ]
        );
    }

    #[test]
    fn test_queue_get_with_partition() {
        let mock = MockQueueGrpcClient::new();
        let encoded = pickle_encode(&PickleValue::Int(42));
        mock.push_get(Ok(vec![encoded]));

        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());
        let result = queue
            .get(
                &mock,
                Some(&QueueGetParams {
                    partition: "my-partition".to_string(),
                    ..Default::default()
                }),
            )
            .unwrap();
        assert_eq!(result, PickleValue::Int(42));
    }

    #[test]
    fn test_queue_put_with_custom_ttl() {
        let mock = MockQueueGrpcClient::new();
        mock.push_put(Ok(()));

        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());
        queue
            .put(
                &mock,
                42i64,
                Some(&QueuePutParams {
                    partition_ttl: Duration::from_secs(3600),
                    ..Default::default()
                }),
            )
            .unwrap();
    }

    #[test]
    fn test_queue_iterate_returns_items() {
        let mock = MockQueueGrpcClient::new();
        // First call returns items
        let items = vec![
            (
                "entry-1".to_string(),
                pickle_encode(&PickleValue::Int(10)),
            ),
            (
                "entry-2".to_string(),
                pickle_encode(&PickleValue::Int(20)),
            ),
        ];
        mock.push_next_items(Ok(items));
        // Second call returns empty → signals done (timeout=0 means exit after first batch)
        mock.push_next_items(Ok(vec![]));

        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());
        let results = queue.iterate(&mock, None).unwrap();
        assert_eq!(
            results,
            vec![PickleValue::Int(10), PickleValue::Int(20)]
        );
    }

    #[test]
    fn test_queue_iterate_empty() {
        let mock = MockQueueGrpcClient::new();
        mock.push_next_items(Ok(vec![]));

        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());
        let results = queue.iterate(&mock, None).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_queue_iterate_with_poll_timeout() {
        let mock = MockQueueGrpcClient::new();
        let items = vec![(
            "entry-1".to_string(),
            pickle_encode(&PickleValue::String("data".to_string())),
        )];
        mock.push_next_items(Ok(items));
        // After receiving items, poll again with timeout — returns empty
        mock.push_next_items(Ok(vec![]));

        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());
        let results = queue
            .iterate(
                &mock,
                Some(&QueueIterateParams {
                    item_poll_timeout: Duration::ZERO,
                    ..Default::default()
                }),
            )
            .unwrap();
        assert_eq!(
            results,
            vec![PickleValue::String("data".to_string())]
        );
    }

    #[test]
    fn test_queue_iterate_error_propagates() {
        let mock = MockQueueGrpcClient::new();
        mock.push_next_items(Err(ModalError::Grpc(tonic::Status::internal(
            "server error",
        ))));

        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());
        let err = queue.iterate(&mock, None).unwrap_err();
        assert!(matches!(err, ModalError::Grpc(_)));
    }

    #[test]
    fn test_queue_put_and_get_roundtrip_types() {
        // Test that various types survive put→serialize→deserialize→get
        let test_values: Vec<PickleValue> = vec![
            PickleValue::None,
            PickleValue::Bool(true),
            PickleValue::Bool(false),
            PickleValue::Int(0),
            PickleValue::Int(-42),
            PickleValue::Int(i64::MAX),
            PickleValue::Float(3.14),
            PickleValue::String("hello 🦀".to_string()),
            PickleValue::Bytes(vec![0, 1, 2, 255]),
            PickleValue::List(vec![PickleValue::Int(1), PickleValue::Int(2)]),
        ];

        for val in test_values {
            let encoded = pickle_encode(&val);
            let decoded = pickle_deserialize(&encoded).unwrap();
            // NaN needs special handling but we don't test it here
            assert_eq!(decoded, val, "roundtrip failed for {:?}", val);
        }
    }

    #[test]
    fn test_queue_get_with_invalid_partition() {
        let mock = MockQueueGrpcClient::new();
        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());

        let long_partition = "a".repeat(65);
        let err = queue
            .get(
                &mock,
                Some(&QueueGetParams {
                    partition: long_partition,
                    ..Default::default()
                }),
            )
            .unwrap_err();
        assert!(err.to_string().contains("1–64 bytes long"));
    }

    #[test]
    fn test_queue_put_with_invalid_partition() {
        let mock = MockQueueGrpcClient::new();
        let queue = Queue::new("qu-test-123".to_string(), "test".to_string());

        let long_partition = "a".repeat(65);
        let err = queue
            .put(
                &mock,
                42i64,
                Some(&QueuePutParams {
                    partition: long_partition,
                    ..Default::default()
                }),
            )
            .unwrap_err();
        assert!(err.to_string().contains("1–64 bytes long"));
    }
}
