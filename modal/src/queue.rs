/// Queue represents a Modal distributed queue.
#[derive(Debug, Clone)]
pub struct Queue {
    pub queue_id: String,
}

/// QueueService provides Queue related operations.
pub trait QueueService: Send + Sync {
    // Service methods will be added for integration tests
}
