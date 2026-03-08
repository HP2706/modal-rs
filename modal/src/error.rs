use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModalError {
    #[error("FunctionTimeoutError: {0}")]
    FunctionTimeout(String),

    #[error("RemoteError: {0}")]
    Remote(String),

    #[error("InternalFailure: {0}")]
    InternalFailure(String),

    #[error("ExecutionError: {0}")]
    Execution(String),

    #[error("NotFoundError: {0}")]
    NotFound(String),

    #[error("AlreadyExistsError: {0}")]
    AlreadyExists(String),

    #[error("InvalidError: {0}")]
    Invalid(String),

    #[error("QueueEmptyError: {0}")]
    QueueEmpty(String),

    #[error("QueueFullError: {0}")]
    QueueFull(String),

    #[error("SandboxFilesystemError: {0}")]
    SandboxFilesystem(String),

    #[error("SandboxTimeoutError: {0}")]
    SandboxTimeout(String),

    #[error("ClientClosedError: {0}")]
    ClientClosed(String),

    #[error("ExecTimeoutError: {0}")]
    ExecTimeout(String),

    #[error("GrpcError: {0}")]
    Grpc(#[from] tonic::Status),

    #[error("ConfigError: {0}")]
    Config(String),

    #[error("SerializationError: {0}")]
    Serialization(String),

    #[error("{0}")]
    Other(String),
}
