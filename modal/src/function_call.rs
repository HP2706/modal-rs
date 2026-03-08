/// FunctionCall represents an async function invocation.
#[derive(Debug, Clone)]
pub struct FunctionCall {
    pub function_call_id: String,
}

/// FunctionCallService provides FunctionCall related operations.
pub trait FunctionCallService: Send + Sync {
    // Service methods will be added for integration tests
}
