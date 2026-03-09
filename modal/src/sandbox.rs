use std::collections::HashMap;
use std::time::Duration;

use crate::error::ModalError;

const MAX_ARG_LEN: usize = 1 << 16; // 64 KiB

/// I/O stream configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamConfig {
    #[default]
    Pipe,
    Ignore,
}

/// SandboxCreateParams are options for creating a sandbox.
#[derive(Debug, Clone, Default)]
pub struct SandboxCreateParams {
    pub pty: bool,
    pub cpu: f64,
    pub cpu_limit: f64,
    pub memory_mib: i32,
    pub memory_limit_mib: i32,
    pub timeout_secs: Option<u32>,
    pub custom_domain: Option<String>,
}

/// SandboxExecParams are options for executing a command in a sandbox.
#[derive(Debug, Clone, Default)]
pub struct SandboxExecParams {
    pub pty: bool,
    pub stdout: StreamConfig,
    pub stderr: StreamConfig,
    pub workdir: String,
    pub timeout: Duration,
}

/// SandboxFromNameParams are options for finding a sandbox by name.
#[derive(Debug, Clone, Default)]
pub struct SandboxFromNameParams {
    pub environment: String,
}

/// SandboxListParams are options for listing sandboxes.
#[derive(Debug, Clone, Default)]
pub struct SandboxListParams {
    /// Filter by App ID.
    pub app_id: String,
    /// Only include sandboxes that have all these tags.
    pub tags: HashMap<String, String>,
    /// Override environment for this request.
    pub environment: String,
}

/// SandboxTerminateParams are options for Terminate.
#[derive(Debug, Clone, Default)]
pub struct SandboxTerminateParams {
    /// If true, Terminate will wait for the sandbox to finish.
    pub wait: bool,
}

/// SandboxCreateConnectTokenParams are optional parameters for CreateConnectToken.
#[derive(Debug, Clone, Default)]
pub struct SandboxCreateConnectTokenParams {
    /// Optional user-provided metadata string added to headers by the proxy.
    pub user_metadata: String,
}

/// SandboxCreateConnectCredentials contains the URL and token for connecting to a Sandbox.
#[derive(Debug, Clone)]
pub struct SandboxCreateConnectCredentials {
    pub url: String,
    pub token: String,
}

/// Tunnel represents a port forwarded from within a running Modal Sandbox.
#[derive(Debug, Clone)]
pub struct Tunnel {
    pub host: String,
    pub port: i32,
    pub unencrypted_host: String,
    pub unencrypted_port: i32,
}

impl Tunnel {
    /// Get the public HTTPS URL of the forwarded port.
    pub fn url(&self) -> String {
        if self.port == 443 {
            format!("https://{}", self.host)
        } else {
            format!("https://{}:{}", self.host, self.port)
        }
    }

    /// Get the public TLS socket as (host, port).
    pub fn tls_socket(&self) -> (&str, i32) {
        (&self.host, self.port)
    }

    /// Get the public TCP socket as (host, port). Returns an error if the tunnel
    /// is not configured for unencrypted TCP.
    pub fn tcp_socket(&self) -> Result<(&str, i32), ModalError> {
        if self.unencrypted_host.is_empty() || self.unencrypted_port == 0 {
            return Err(ModalError::Invalid(
                "This tunnel is not configured for unencrypted TCP.".to_string(),
            ));
        }
        Ok((&self.unencrypted_host, self.unencrypted_port))
    }
}

/// PTY configuration defaults.
#[derive(Debug, Clone)]
pub struct PtyInfo {
    pub enabled: bool,
    pub winsz_rows: u32,
    pub winsz_cols: u32,
    pub env_term: String,
    pub env_colorterm: String,
    pub pty_type: PtyType,
    pub no_terminate_on_idle_stdin: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PtyType {
    Shell,
}

impl Default for PtyInfo {
    fn default() -> Self {
        Self {
            enabled: true,
            winsz_rows: 24,
            winsz_cols: 80,
            env_term: "xterm-256color".to_string(),
            env_colorterm: "truecolor".to_string(),
            pty_type: PtyType::Shell,
            no_terminate_on_idle_stdin: false,
        }
    }
}

/// Stdout/stderr config proto values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdoutConfig {
    Pipe,
    DevNull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StderrConfig {
    Pipe,
    DevNull,
}

/// SandboxCreateRequest proto representation.
#[derive(Debug, Clone)]
pub struct SandboxCreateRequestProto {
    pub app_id: String,
    pub image_id: String,
    pub timeout_secs: u32,
    pub pty_info: Option<PtyInfo>,
    pub cpu_milli: u32,
    pub cpu_milli_max: u32,
    pub memory_mb: u32,
    pub memory_mb_max: u32,
    pub custom_domain: Option<String>,
}

/// Build sandbox create request proto.
pub fn build_sandbox_create_request_proto(
    app_id: &str,
    image_id: &str,
    params: SandboxCreateParams,
) -> Result<SandboxCreateRequestProto, ModalError> {
    // CPU validation
    if params.cpu < 0.0 {
        return Err(ModalError::Invalid(format!(
            "the CPU request ({}) must be a positive number",
            params.cpu
        )));
    }
    if params.memory_mib < 0 {
        return Err(ModalError::Invalid(format!(
            "the MemoryMiB request ({}) must be a positive number",
            params.memory_mib
        )));
    }
    if params.cpu_limit > 0.0 && params.cpu == 0.0 {
        return Err(ModalError::Invalid(
            "must also specify non-zero CPU request when CPULimit is specified".to_string(),
        ));
    }
    if params.cpu_limit > 0.0 && params.cpu_limit < params.cpu {
        return Err(ModalError::Invalid(format!(
            "the CPU request ({:.*}) cannot be higher than CPULimit ({:.*})",
            6, params.cpu, 6, params.cpu_limit
        )));
    }
    if params.memory_limit_mib > 0 && params.memory_mib == 0 {
        return Err(ModalError::Invalid(
            "must also specify non-zero MemoryMiB request when MemoryLimitMiB is specified"
                .to_string(),
        ));
    }
    if params.memory_limit_mib > 0 && params.memory_limit_mib < params.memory_mib {
        return Err(ModalError::Invalid(format!(
            "the MemoryMiB request ({}) cannot be higher than MemoryLimitMiB ({})",
            params.memory_mib, params.memory_limit_mib
        )));
    }

    let pty_info = if params.pty {
        Some(PtyInfo::default())
    } else {
        None
    };

    Ok(SandboxCreateRequestProto {
        app_id: app_id.to_string(),
        image_id: image_id.to_string(),
        timeout_secs: params.timeout_secs.unwrap_or(300),
        pty_info,
        cpu_milli: (params.cpu * 1000.0) as u32,
        cpu_milli_max: (params.cpu_limit * 1000.0) as u32,
        memory_mb: params.memory_mib as u32,
        memory_mb_max: params.memory_limit_mib as u32,
        custom_domain: params.custom_domain,
    })
}

/// TaskExecStartRequest proto representation.
#[derive(Debug, Clone)]
pub struct TaskExecStartRequestProto {
    pub task_id: String,
    pub exec_id: String,
    pub command: Vec<String>,
    pub pty_info: Option<PtyInfo>,
    pub stdout_config: StdoutConfig,
    pub stderr_config: StderrConfig,
    pub workdir: String,
    pub timeout_secs: Option<u32>,
}

impl TaskExecStartRequestProto {
    pub fn has_timeout_secs(&self) -> bool {
        self.timeout_secs.is_some()
    }

    pub fn get_timeout_secs(&self) -> u32 {
        self.timeout_secs.unwrap_or(0)
    }
}

/// Build task exec start request proto.
pub fn build_task_exec_start_request_proto(
    task_id: &str,
    exec_id: &str,
    command: Vec<String>,
    params: SandboxExecParams,
) -> Result<TaskExecStartRequestProto, ModalError> {
    // Timeout validation
    if params.timeout != Duration::ZERO {
        // Duration is always non-negative in Rust
        if params.timeout.subsec_nanos() != 0 {
            return Err(ModalError::Invalid(
                "timeout must be a whole number of seconds".to_string(),
            ));
        }
    }

    let pty_info = if params.pty {
        Some(PtyInfo {
            no_terminate_on_idle_stdin: true,
            ..Default::default()
        })
    } else {
        None
    };

    let stdout_config = match params.stdout {
        StreamConfig::Pipe => StdoutConfig::Pipe,
        StreamConfig::Ignore => StdoutConfig::DevNull,
    };
    let stderr_config = match params.stderr {
        StreamConfig::Pipe => StderrConfig::Pipe,
        StreamConfig::Ignore => StderrConfig::DevNull,
    };

    let timeout_secs = if params.timeout != Duration::ZERO {
        Some(params.timeout.as_secs() as u32)
    } else {
        None
    };

    Ok(TaskExecStartRequestProto {
        task_id: task_id.to_string(),
        exec_id: exec_id.to_string(),
        command,
        pty_info,
        stdout_config,
        stderr_config,
        workdir: params.workdir,
        timeout_secs,
    })
}

/// Result of waiting on a sandbox.
#[derive(Debug, Clone)]
pub struct SandboxWaitResult {
    /// Exit code of the sandbox process.
    pub exit_code: i32,
    /// Whether the sandbox completed successfully.
    pub success: bool,
    /// Exception message if the sandbox failed.
    pub exception: Option<String>,
}

/// Result of waiting for an exec command.
#[derive(Debug, Clone)]
pub struct ExecWaitResult {
    /// Exit code of the executed command.
    pub exit_code: Option<i32>,
    /// Whether the command has completed.
    pub completed: bool,
}

/// Result status from a generic result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenericResultStatus {
    Unspecified,
    Success,
    Failure,
    Timeout,
    Terminated,
}

/// Get the return code from a generic result status and exit code.
/// Matches Go's getReturnCode behavior.
pub fn get_return_code(status: GenericResultStatus, exit_code: i32) -> Option<i32> {
    match status {
        GenericResultStatus::Unspecified => None,
        GenericResultStatus::Timeout => Some(124),
        GenericResultStatus::Terminated => Some(137),
        _ => Some(exit_code),
    }
}

/// Poll result from sandbox_poll.
#[derive(Debug, Clone)]
pub struct SandboxPollResult {
    /// The exit code if the sandbox has finished. None if still running.
    pub exit_code: Option<i32>,
}

/// Sandbox represents a running Modal Sandbox.
#[derive(Debug, Clone)]
pub struct Sandbox {
    pub sandbox_id: String,
    pub task_id: Option<String>,
}

impl Sandbox {
    pub fn new(sandbox_id: String) -> Self {
        Self {
            sandbox_id,
            task_id: None,
        }
    }

    pub fn with_task_id(sandbox_id: String, task_id: String) -> Self {
        Self {
            sandbox_id,
            task_id: Some(task_id),
        }
    }
}

/// Listing entry from sandbox_list.
#[derive(Debug, Clone)]
pub struct SandboxListEntry {
    pub sandbox_id: String,
    pub created_at: f64,
}

/// SandboxService provides Sandbox lifecycle operations.
pub trait SandboxService: Send + Sync {
    /// Create a new sandbox.
    fn create(
        &self,
        app_id: &str,
        image_id: &str,
        params: SandboxCreateParams,
    ) -> Result<Sandbox, ModalError>;

    /// Look up a sandbox by ID.
    fn from_id(&self, sandbox_id: &str) -> Result<Sandbox, ModalError>;

    /// Look up a sandbox by name from a deployed App.
    fn from_name(
        &self,
        app_name: &str,
        name: &str,
        params: Option<&SandboxFromNameParams>,
    ) -> Result<Sandbox, ModalError>;

    /// List sandboxes, optionally filtered by tags and app ID.
    fn list(&self, params: Option<&SandboxListParams>) -> Result<Vec<Sandbox>, ModalError>;

    /// Get the task ID for a sandbox (waits for the sandbox to be scheduled).
    fn get_task_id(&self, sandbox_id: &str) -> Result<String, ModalError>;

    /// Execute a command in a sandbox. Returns an exec_id.
    fn exec(
        &self,
        sandbox: &Sandbox,
        command: Vec<String>,
        params: SandboxExecParams,
    ) -> Result<String, ModalError>;

    /// Wait for an exec command to complete.
    fn exec_wait(&self, exec_id: &str, timeout: f32) -> Result<ExecWaitResult, ModalError>;

    /// Wait for a sandbox to complete.
    fn wait(&self, sandbox_id: &str, timeout: f32) -> Result<SandboxWaitResult, ModalError>;

    /// Check if the sandbox has finished (non-blocking).
    /// Returns Some(exit_code) if finished, None if still running.
    fn poll(&self, sandbox_id: &str) -> Result<SandboxPollResult, ModalError>;

    /// Terminate a sandbox.
    fn terminate(&self, sandbox_id: &str) -> Result<(), ModalError>;

    /// Set tags on a sandbox.
    fn set_tags(
        &self,
        sandbox_id: &str,
        tags: &HashMap<String, String>,
    ) -> Result<(), ModalError>;

    /// Get tags from a sandbox.
    fn get_tags(&self, sandbox_id: &str) -> Result<HashMap<String, String>, ModalError>;

    /// Get tunnel metadata for the sandbox.
    fn tunnels(
        &self,
        sandbox_id: &str,
        timeout: f32,
    ) -> Result<HashMap<i32, Tunnel>, ModalError>;

    /// Take a snapshot of the sandbox's filesystem. Returns an image_id.
    fn snapshot_filesystem(
        &self,
        sandbox_id: &str,
        timeout: f32,
    ) -> Result<String, ModalError>;

    /// Snapshot a directory from the sandbox as a new image. Returns an image_id.
    fn snapshot_directory(
        &self,
        sandbox: &Sandbox,
        path: &str,
    ) -> Result<String, ModalError>;

    /// Mount an image at a path in the sandbox filesystem.
    fn mount_image(
        &self,
        sandbox: &Sandbox,
        path: &str,
        image_id: Option<&str>,
    ) -> Result<(), ModalError>;

    /// Create a connect token for HTTP connections to the sandbox.
    fn create_connect_token(
        &self,
        sandbox_id: &str,
        params: Option<&SandboxCreateConnectTokenParams>,
    ) -> Result<SandboxCreateConnectCredentials, ModalError>;
}

/// Trait abstracting the gRPC calls needed for Sandbox operations.
pub trait SandboxGrpcClient: Send + Sync {
    fn sandbox_create(
        &self,
        app_id: &str,
        image_id: &str,
        params: &SandboxCreateParams,
    ) -> Result<String, ModalError>;

    fn sandbox_get_task_id(
        &self,
        sandbox_id: &str,
    ) -> Result<(Option<String>, bool), ModalError>;

    fn container_exec(
        &self,
        task_id: &str,
        command: Vec<String>,
        params: &SandboxExecParams,
    ) -> Result<String, ModalError>;

    fn container_exec_wait(
        &self,
        exec_id: &str,
        timeout: f32,
    ) -> Result<ExecWaitResult, ModalError>;

    fn sandbox_wait(
        &self,
        sandbox_id: &str,
        timeout: f32,
    ) -> Result<SandboxWaitResult, ModalError>;

    fn sandbox_terminate(&self, sandbox_id: &str) -> Result<(), ModalError>;

    /// Look up sandbox by ID (SandboxWait with timeout=0).
    fn sandbox_from_id(&self, sandbox_id: &str) -> Result<(), ModalError>;

    /// Look up sandbox by name (SandboxGetFromName RPC).
    fn sandbox_from_name(
        &self,
        app_name: &str,
        name: &str,
        environment: &str,
    ) -> Result<String, ModalError>;

    /// List sandboxes (SandboxList RPC).
    /// Returns a page of sandbox entries. Pagination is handled by the caller.
    fn sandbox_list(
        &self,
        app_id: &str,
        environment: &str,
        tags: &HashMap<String, String>,
        before_timestamp: f64,
    ) -> Result<Vec<SandboxListEntry>, ModalError>;

    /// Poll sandbox (SandboxWait with timeout=0, returns exit code if finished).
    fn sandbox_poll(&self, sandbox_id: &str) -> Result<SandboxPollResult, ModalError>;

    /// Set tags on a sandbox.
    fn sandbox_tags_set(
        &self,
        sandbox_id: &str,
        tags: &HashMap<String, String>,
    ) -> Result<(), ModalError>;

    /// Get tags from a sandbox.
    fn sandbox_tags_get(
        &self,
        sandbox_id: &str,
    ) -> Result<HashMap<String, String>, ModalError>;

    /// Get tunnel metadata for the sandbox.
    fn sandbox_get_tunnels(
        &self,
        sandbox_id: &str,
        timeout: f32,
    ) -> Result<SandboxTunnelsResult, ModalError>;

    /// Take filesystem snapshot.
    fn sandbox_snapshot_fs(
        &self,
        sandbox_id: &str,
        timeout: f32,
    ) -> Result<SandboxSnapshotResult, ModalError>;

    /// Snapshot a directory from the sandbox.
    fn sandbox_snapshot_directory(
        &self,
        task_id: &str,
        path: &str,
    ) -> Result<String, ModalError>;

    /// Mount an image at a path in the sandbox.
    fn sandbox_mount_image(
        &self,
        task_id: &str,
        path: &str,
        image_id: &str,
    ) -> Result<(), ModalError>;

    /// Create a connect token for the sandbox.
    fn sandbox_create_connect_token(
        &self,
        sandbox_id: &str,
        user_metadata: &str,
    ) -> Result<SandboxCreateConnectCredentials, ModalError>;
}

/// Result from sandbox_get_tunnels.
#[derive(Debug, Clone)]
pub struct SandboxTunnelsResult {
    /// Whether a timeout occurred.
    pub timed_out: bool,
    /// Map of container port -> Tunnel.
    pub tunnels: HashMap<i32, Tunnel>,
}

/// Result from sandbox_snapshot_fs.
#[derive(Debug, Clone)]
pub struct SandboxSnapshotResult {
    pub image_id: String,
    pub success: bool,
    pub exception: Option<String>,
}

/// Implementation of SandboxService backed by a gRPC client.
pub struct SandboxServiceImpl<C: SandboxGrpcClient> {
    pub client: C,
}

impl<C: SandboxGrpcClient> SandboxService for SandboxServiceImpl<C> {
    fn create(
        &self,
        app_id: &str,
        image_id: &str,
        params: SandboxCreateParams,
    ) -> Result<Sandbox, ModalError> {
        // Validate params (reuse existing validation)
        let _ = build_sandbox_create_request_proto(app_id, image_id, params.clone())?;

        let sandbox_id = self.client.sandbox_create(app_id, image_id, &params)?;
        Ok(Sandbox::new(sandbox_id))
    }

    fn from_id(&self, sandbox_id: &str) -> Result<Sandbox, ModalError> {
        self.client.sandbox_from_id(sandbox_id).map_err(|e| {
            if matches!(&e, ModalError::Grpc(s) if s.code() == tonic::Code::NotFound) {
                ModalError::NotFound(format!("Sandbox with id: '{}' not found", sandbox_id))
            } else {
                e
            }
        })?;
        Ok(Sandbox::new(sandbox_id.to_string()))
    }

    fn from_name(
        &self,
        app_name: &str,
        name: &str,
        params: Option<&SandboxFromNameParams>,
    ) -> Result<Sandbox, ModalError> {
        let default_params = SandboxFromNameParams::default();
        let params = params.unwrap_or(&default_params);

        let sandbox_id = self
            .client
            .sandbox_from_name(app_name, name, &params.environment)
            .map_err(|e| {
                if matches!(&e, ModalError::Grpc(s) if s.code() == tonic::Code::NotFound) {
                    ModalError::NotFound(format!(
                        "Sandbox with name '{}' not found in app '{}'",
                        name, app_name
                    ))
                } else {
                    e
                }
            })?;

        Ok(Sandbox::new(sandbox_id))
    }

    fn list(&self, params: Option<&SandboxListParams>) -> Result<Vec<Sandbox>, ModalError> {
        let default_params = SandboxListParams::default();
        let params = params.unwrap_or(&default_params);

        let mut all_sandboxes = Vec::new();
        let mut before_timestamp: f64 = 0.0;

        loop {
            let entries = self.client.sandbox_list(
                &params.app_id,
                &params.environment,
                &params.tags,
                before_timestamp,
            )?;

            if entries.is_empty() {
                break;
            }

            before_timestamp = entries.last().unwrap().created_at;

            for entry in entries {
                all_sandboxes.push(Sandbox::new(entry.sandbox_id));
            }
        }

        Ok(all_sandboxes)
    }

    fn get_task_id(&self, sandbox_id: &str) -> Result<String, ModalError> {
        let (task_id, _exited) = self.client.sandbox_get_task_id(sandbox_id)?;
        task_id.ok_or_else(|| {
            ModalError::Other(format!(
                "Sandbox '{}' was terminated before a task could be scheduled",
                sandbox_id
            ))
        })
    }

    fn exec(
        &self,
        sandbox: &Sandbox,
        command: Vec<String>,
        params: SandboxExecParams,
    ) -> Result<String, ModalError> {
        validate_exec_args(&command)?;

        let task_id = sandbox.task_id.as_deref().ok_or_else(|| {
            ModalError::Other("sandbox has no task_id; call get_task_id first".to_string())
        })?;

        self.client.container_exec(task_id, command, &params)
    }

    fn exec_wait(&self, exec_id: &str, timeout: f32) -> Result<ExecWaitResult, ModalError> {
        self.client.container_exec_wait(exec_id, timeout)
    }

    fn wait(&self, sandbox_id: &str, timeout: f32) -> Result<SandboxWaitResult, ModalError> {
        self.client.sandbox_wait(sandbox_id, timeout)
    }

    fn poll(&self, sandbox_id: &str) -> Result<SandboxPollResult, ModalError> {
        self.client.sandbox_poll(sandbox_id)
    }

    fn terminate(&self, sandbox_id: &str) -> Result<(), ModalError> {
        self.client.sandbox_terminate(sandbox_id)
    }

    fn set_tags(
        &self,
        sandbox_id: &str,
        tags: &HashMap<String, String>,
    ) -> Result<(), ModalError> {
        self.client.sandbox_tags_set(sandbox_id, tags)
    }

    fn get_tags(&self, sandbox_id: &str) -> Result<HashMap<String, String>, ModalError> {
        self.client.sandbox_tags_get(sandbox_id)
    }

    fn tunnels(
        &self,
        sandbox_id: &str,
        timeout: f32,
    ) -> Result<HashMap<i32, Tunnel>, ModalError> {
        let result = self.client.sandbox_get_tunnels(sandbox_id, timeout)?;
        if result.timed_out {
            return Err(ModalError::SandboxTimeout(
                "Sandbox operation timed out".to_string(),
            ));
        }
        Ok(result.tunnels)
    }

    fn snapshot_filesystem(
        &self,
        sandbox_id: &str,
        timeout: f32,
    ) -> Result<String, ModalError> {
        let result = self.client.sandbox_snapshot_fs(sandbox_id, timeout)?;
        if !result.success {
            return Err(ModalError::Execution(format!(
                "Sandbox snapshot failed: {}",
                result.exception.unwrap_or_else(|| "unknown".to_string())
            )));
        }
        if result.image_id.is_empty() {
            return Err(ModalError::Execution(
                "Sandbox snapshot response missing image ID".to_string(),
            ));
        }
        Ok(result.image_id)
    }

    fn snapshot_directory(
        &self,
        sandbox: &Sandbox,
        path: &str,
    ) -> Result<String, ModalError> {
        let task_id = sandbox.task_id.as_deref().ok_or_else(|| {
            ModalError::Other("sandbox has no task_id; call get_task_id first".to_string())
        })?;

        let image_id = self.client.sandbox_snapshot_directory(task_id, path)?;
        if image_id.is_empty() {
            return Err(ModalError::Execution(
                "Sandbox snapshot directory response missing `imageId`".to_string(),
            ));
        }
        Ok(image_id)
    }

    fn mount_image(
        &self,
        sandbox: &Sandbox,
        path: &str,
        image_id: Option<&str>,
    ) -> Result<(), ModalError> {
        let task_id = sandbox.task_id.as_deref().ok_or_else(|| {
            ModalError::Other("sandbox has no task_id; call get_task_id first".to_string())
        })?;

        self.client
            .sandbox_mount_image(task_id, path, image_id.unwrap_or(""))
    }

    fn create_connect_token(
        &self,
        sandbox_id: &str,
        params: Option<&SandboxCreateConnectTokenParams>,
    ) -> Result<SandboxCreateConnectCredentials, ModalError> {
        let default_params = SandboxCreateConnectTokenParams::default();
        let params = params.unwrap_or(&default_params);

        self.client
            .sandbox_create_connect_token(sandbox_id, &params.user_metadata)
    }
}

/// Validate exec arguments don't exceed system limits.
pub fn validate_exec_args(args: &[String]) -> Result<(), ModalError> {
    let total_len: usize = args.iter().map(|a| a.len()).sum();
    if total_len > MAX_ARG_LEN {
        return Err(ModalError::Invalid(format!(
            "Total length of CMD arguments must be less than {} bytes, got {}",
            MAX_ARG_LEN, total_len
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Proto builder tests ---

    #[test]
    fn test_sandbox_create_without_pty() {
        let req = build_sandbox_create_request_proto(
            "app-123",
            "img-456",
            SandboxCreateParams::default(),
        )
        .unwrap();
        assert!(req.pty_info.is_none());
    }

    #[test]
    fn test_sandbox_create_with_pty() {
        let req = build_sandbox_create_request_proto(
            "app-123",
            "img-456",
            SandboxCreateParams {
                pty: true,
                ..Default::default()
            },
        )
        .unwrap();
        let pty = req.pty_info.unwrap();
        assert!(pty.enabled);
        assert_eq!(pty.winsz_rows, 24);
        assert_eq!(pty.winsz_cols, 80);
        assert_eq!(pty.env_term, "xterm-256color");
        assert_eq!(pty.env_colorterm, "truecolor");
        assert_eq!(pty.pty_type, PtyType::Shell);
    }

    #[test]
    fn test_task_exec_start_without_pty() {
        let req = build_task_exec_start_request_proto(
            "task-123",
            "exec-456",
            vec!["bash".to_string()],
            SandboxExecParams::default(),
        )
        .unwrap();
        assert!(req.pty_info.is_none());
    }

    #[test]
    fn test_task_exec_start_with_pty() {
        let req = build_task_exec_start_request_proto(
            "task-123",
            "exec-456",
            vec!["bash".to_string()],
            SandboxExecParams {
                pty: true,
                ..Default::default()
            },
        )
        .unwrap();
        let pty = req.pty_info.unwrap();
        assert!(pty.enabled);
        assert_eq!(pty.winsz_rows, 24);
        assert_eq!(pty.winsz_cols, 80);
        assert_eq!(pty.env_term, "xterm-256color");
        assert_eq!(pty.env_colorterm, "truecolor");
        assert_eq!(pty.pty_type, PtyType::Shell);
        assert!(pty.no_terminate_on_idle_stdin);
    }

    #[test]
    fn test_task_exec_start_defaults() {
        let req = build_task_exec_start_request_proto(
            "task-123",
            "exec-456",
            vec!["bash".to_string()],
            SandboxExecParams::default(),
        )
        .unwrap();

        assert_eq!(req.workdir, "");
        assert!(!req.has_timeout_secs());
        assert!(req.pty_info.is_none());
        assert_eq!(req.stdout_config, StdoutConfig::Pipe);
        assert_eq!(req.stderr_config, StderrConfig::Pipe);
    }

    #[test]
    fn test_task_exec_start_with_stdout_ignore() {
        let req = build_task_exec_start_request_proto(
            "task-123",
            "exec-456",
            vec!["bash".to_string()],
            SandboxExecParams {
                stdout: StreamConfig::Ignore,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(req.stdout_config, StdoutConfig::DevNull);
        assert_eq!(req.stderr_config, StderrConfig::Pipe);
    }

    #[test]
    fn test_task_exec_start_with_stderr_ignore() {
        let req = build_task_exec_start_request_proto(
            "task-123",
            "exec-456",
            vec!["bash".to_string()],
            SandboxExecParams {
                stderr: StreamConfig::Ignore,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(req.stdout_config, StdoutConfig::Pipe);
        assert_eq!(req.stderr_config, StderrConfig::DevNull);
    }

    #[test]
    fn test_task_exec_start_with_workdir() {
        let req = build_task_exec_start_request_proto(
            "task-123",
            "exec-456",
            vec!["pwd".to_string()],
            SandboxExecParams {
                workdir: "/tmp".to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(req.workdir, "/tmp");
    }

    #[test]
    fn test_task_exec_start_with_timeout() {
        let req = build_task_exec_start_request_proto(
            "task-123",
            "exec-456",
            vec!["sleep".to_string(), "10".to_string()],
            SandboxExecParams {
                timeout: Duration::from_secs(30),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(req.has_timeout_secs());
        assert_eq!(req.get_timeout_secs(), 30);
    }

    #[test]
    fn test_task_exec_start_invalid_timeout_not_whole_seconds() {
        let err = build_task_exec_start_request_proto(
            "task-123",
            "exec-456",
            vec!["echo".to_string(), "hi".to_string()],
            SandboxExecParams {
                timeout: Duration::from_millis(1500),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("whole number of seconds"));
    }

    #[test]
    fn test_validate_exec_args_within_limit() {
        assert!(validate_exec_args(&["echo".to_string(), "hello".to_string()]).is_ok());
    }

    #[test]
    fn test_validate_exec_args_exceeding_limit() {
        let large_arg = "a".repeat(MAX_ARG_LEN + 1);
        let err = validate_exec_args(&[large_arg]).unwrap_err();
        assert!(err
            .to_string()
            .contains("Total length of CMD arguments must be less than"));
    }

    #[test]
    fn test_sandbox_create_with_cpu_and_cpu_limit() {
        let req = build_sandbox_create_request_proto(
            "app-123",
            "img-456",
            SandboxCreateParams {
                cpu: 2.0,
                cpu_limit: 4.5,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(req.cpu_milli, 2000);
        assert_eq!(req.cpu_milli_max, 4500);
    }

    #[test]
    fn test_sandbox_create_cpu_limit_lower_than_cpu() {
        let err = build_sandbox_create_request_proto(
            "app-123",
            "img-456",
            SandboxCreateParams {
                cpu: 4.0,
                cpu_limit: 2.0,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("the CPU request (4.000000) cannot be higher than CPULimit (2.000000)"));
    }

    #[test]
    fn test_sandbox_create_cpu_limit_without_cpu() {
        let err = build_sandbox_create_request_proto(
            "app-123",
            "img-456",
            SandboxCreateParams {
                cpu_limit: 4.0,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("must also specify non-zero CPU request when CPULimit is specified"));
    }

    #[test]
    fn test_sandbox_create_with_memory_and_memory_limit() {
        let req = build_sandbox_create_request_proto(
            "app-123",
            "img-456",
            SandboxCreateParams {
                memory_mib: 1024,
                memory_limit_mib: 2048,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(req.memory_mb, 1024);
        assert_eq!(req.memory_mb_max, 2048);
    }

    #[test]
    fn test_sandbox_create_memory_limit_lower_than_memory() {
        let err = build_sandbox_create_request_proto(
            "app-123",
            "img-456",
            SandboxCreateParams {
                memory_mib: 2048,
                memory_limit_mib: 1024,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("the MemoryMiB request (2048) cannot be higher than MemoryLimitMiB (1024)"));
    }

    #[test]
    fn test_sandbox_create_memory_limit_without_memory() {
        let err = build_sandbox_create_request_proto(
            "app-123",
            "img-456",
            SandboxCreateParams {
                memory_limit_mib: 2048,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains(
            "must also specify non-zero MemoryMiB request when MemoryLimitMiB is specified"
        ));
    }

    #[test]
    fn test_sandbox_create_negative_cpu() {
        let err = build_sandbox_create_request_proto(
            "app-123",
            "img-456",
            SandboxCreateParams {
                cpu: -1.0,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("must be a positive number"));
    }

    #[test]
    fn test_sandbox_create_negative_memory() {
        let err = build_sandbox_create_request_proto(
            "app-123",
            "img-456",
            SandboxCreateParams {
                memory_mib: -100,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("must be a positive number"));
    }

    #[test]
    fn test_sandbox_create_defaults() {
        let req = build_sandbox_create_request_proto(
            "app-123",
            "img-456",
            SandboxCreateParams::default(),
        )
        .unwrap();
        assert_eq!(req.timeout_secs, 300);
        assert!(req.pty_info.is_none());
        assert_eq!(req.cpu_milli, 0);
        assert_eq!(req.memory_mb, 0);
    }

    #[test]
    fn test_sandbox_create_custom_domain() {
        let req = build_sandbox_create_request_proto(
            "app-123",
            "img-456",
            SandboxCreateParams {
                custom_domain: Some("example.com".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(req.custom_domain.as_deref(), Some("example.com"));
    }

    // --- Tunnel tests ---

    #[test]
    fn test_tunnel_url_default_port() {
        let t = Tunnel {
            host: "example.modal.run".to_string(),
            port: 443,
            unencrypted_host: String::new(),
            unencrypted_port: 0,
        };
        assert_eq!(t.url(), "https://example.modal.run");
    }

    #[test]
    fn test_tunnel_url_custom_port() {
        let t = Tunnel {
            host: "example.modal.run".to_string(),
            port: 8080,
            unencrypted_host: String::new(),
            unencrypted_port: 0,
        };
        assert_eq!(t.url(), "https://example.modal.run:8080");
    }

    #[test]
    fn test_tunnel_tls_socket() {
        let t = Tunnel {
            host: "example.modal.run".to_string(),
            port: 443,
            unencrypted_host: String::new(),
            unencrypted_port: 0,
        };
        let (host, port) = t.tls_socket();
        assert_eq!(host, "example.modal.run");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_tunnel_tcp_socket_ok() {
        let t = Tunnel {
            host: "example.modal.run".to_string(),
            port: 443,
            unencrypted_host: "tcp.example.modal.run".to_string(),
            unencrypted_port: 12345,
        };
        let (host, port) = t.tcp_socket().unwrap();
        assert_eq!(host, "tcp.example.modal.run");
        assert_eq!(port, 12345);
    }

    #[test]
    fn test_tunnel_tcp_socket_not_configured() {
        let t = Tunnel {
            host: "example.modal.run".to_string(),
            port: 443,
            unencrypted_host: String::new(),
            unencrypted_port: 0,
        };
        let err = t.tcp_socket().unwrap_err();
        assert!(err.to_string().contains("not configured for unencrypted TCP"));
    }

    // --- get_return_code tests ---

    #[test]
    fn test_get_return_code_unspecified() {
        assert!(get_return_code(GenericResultStatus::Unspecified, 0).is_none());
    }

    #[test]
    fn test_get_return_code_success() {
        assert_eq!(get_return_code(GenericResultStatus::Success, 0), Some(0));
    }

    #[test]
    fn test_get_return_code_failure() {
        assert_eq!(get_return_code(GenericResultStatus::Failure, 1), Some(1));
    }

    #[test]
    fn test_get_return_code_timeout() {
        assert_eq!(get_return_code(GenericResultStatus::Timeout, 0), Some(124));
    }

    #[test]
    fn test_get_return_code_terminated() {
        assert_eq!(get_return_code(GenericResultStatus::Terminated, 0), Some(137));
    }

    // --- Sandbox struct tests ---

    #[test]
    fn test_sandbox_new() {
        let sb = Sandbox::new("sb-1".to_string());
        assert_eq!(sb.sandbox_id, "sb-1");
        assert!(sb.task_id.is_none());
    }

    #[test]
    fn test_sandbox_with_task_id() {
        let sb = Sandbox::with_task_id("sb-1".to_string(), "task-1".to_string());
        assert_eq!(sb.sandbox_id, "sb-1");
        assert_eq!(sb.task_id.as_deref(), Some("task-1"));
    }

    // --- SandboxService tests ---

    struct MockSandboxGrpcClient {
        responses: std::sync::Mutex<Vec<MockSbResponse>>,
    }

    #[allow(clippy::large_enum_variant)]
    enum MockSbResponse {
        Create(Result<String, ModalError>),
        GetTaskId(Result<(Option<String>, bool), ModalError>),
        Exec(Result<String, ModalError>),
        ExecWait(Result<ExecWaitResult, ModalError>),
        Wait(Result<SandboxWaitResult, ModalError>),
        Terminate(Result<(), ModalError>),
        FromId(Result<(), ModalError>),
        FromName(Result<String, ModalError>),
        List(Result<Vec<SandboxListEntry>, ModalError>),
        Poll(Result<SandboxPollResult, ModalError>),
        TagsSet(Result<(), ModalError>),
        TagsGet(Result<HashMap<String, String>, ModalError>),
        GetTunnels(Result<SandboxTunnelsResult, ModalError>),
        SnapshotFs(Result<SandboxSnapshotResult, ModalError>),
        SnapshotDir(Result<String, ModalError>),
        MountImage(Result<(), ModalError>),
        ConnectToken(Result<SandboxCreateConnectCredentials, ModalError>),
    }

    impl MockSandboxGrpcClient {
        fn new() -> Self {
            Self {
                responses: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn push(&self, resp: MockSbResponse) {
            self.responses.lock().unwrap().push(resp);
        }
    }

    impl SandboxGrpcClient for MockSandboxGrpcClient {
        fn sandbox_create(
            &self,
            _app_id: &str,
            _image_id: &str,
            _params: &SandboxCreateParams,
        ) -> Result<String, ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::Create(r) => r,
                _ => panic!("expected Create"),
            }
        }

        fn sandbox_get_task_id(
            &self,
            _sandbox_id: &str,
        ) -> Result<(Option<String>, bool), ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::GetTaskId(r) => r,
                _ => panic!("expected GetTaskId"),
            }
        }

        fn container_exec(
            &self,
            _task_id: &str,
            _command: Vec<String>,
            _params: &SandboxExecParams,
        ) -> Result<String, ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::Exec(r) => r,
                _ => panic!("expected Exec"),
            }
        }

        fn container_exec_wait(
            &self,
            _exec_id: &str,
            _timeout: f32,
        ) -> Result<ExecWaitResult, ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::ExecWait(r) => r,
                _ => panic!("expected ExecWait"),
            }
        }

        fn sandbox_wait(
            &self,
            _sandbox_id: &str,
            _timeout: f32,
        ) -> Result<SandboxWaitResult, ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::Wait(r) => r,
                _ => panic!("expected Wait"),
            }
        }

        fn sandbox_terminate(&self, _sandbox_id: &str) -> Result<(), ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::Terminate(r) => r,
                _ => panic!("expected Terminate"),
            }
        }

        fn sandbox_from_id(&self, _sandbox_id: &str) -> Result<(), ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::FromId(r) => r,
                _ => panic!("expected FromId"),
            }
        }

        fn sandbox_from_name(
            &self,
            _app_name: &str,
            _name: &str,
            _environment: &str,
        ) -> Result<String, ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::FromName(r) => r,
                _ => panic!("expected FromName"),
            }
        }

        fn sandbox_list(
            &self,
            _app_id: &str,
            _environment: &str,
            _tags: &HashMap<String, String>,
            _before_timestamp: f64,
        ) -> Result<Vec<SandboxListEntry>, ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::List(r) => r,
                _ => panic!("expected List"),
            }
        }

        fn sandbox_poll(&self, _sandbox_id: &str) -> Result<SandboxPollResult, ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::Poll(r) => r,
                _ => panic!("expected Poll"),
            }
        }

        fn sandbox_tags_set(
            &self,
            _sandbox_id: &str,
            _tags: &HashMap<String, String>,
        ) -> Result<(), ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::TagsSet(r) => r,
                _ => panic!("expected TagsSet"),
            }
        }

        fn sandbox_tags_get(
            &self,
            _sandbox_id: &str,
        ) -> Result<HashMap<String, String>, ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::TagsGet(r) => r,
                _ => panic!("expected TagsGet"),
            }
        }

        fn sandbox_get_tunnels(
            &self,
            _sandbox_id: &str,
            _timeout: f32,
        ) -> Result<SandboxTunnelsResult, ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::GetTunnels(r) => r,
                _ => panic!("expected GetTunnels"),
            }
        }

        fn sandbox_snapshot_fs(
            &self,
            _sandbox_id: &str,
            _timeout: f32,
        ) -> Result<SandboxSnapshotResult, ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::SnapshotFs(r) => r,
                _ => panic!("expected SnapshotFs"),
            }
        }

        fn sandbox_snapshot_directory(
            &self,
            _task_id: &str,
            _path: &str,
        ) -> Result<String, ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::SnapshotDir(r) => r,
                _ => panic!("expected SnapshotDir"),
            }
        }

        fn sandbox_mount_image(
            &self,
            _task_id: &str,
            _path: &str,
            _image_id: &str,
        ) -> Result<(), ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::MountImage(r) => r,
                _ => panic!("expected MountImage"),
            }
        }

        fn sandbox_create_connect_token(
            &self,
            _sandbox_id: &str,
            _user_metadata: &str,
        ) -> Result<SandboxCreateConnectCredentials, ModalError> {
            match self.responses.lock().unwrap().remove(0) {
                MockSbResponse::ConnectToken(r) => r,
                _ => panic!("expected ConnectToken"),
            }
        }
    }

    fn make_sandbox_service(
        mock: MockSandboxGrpcClient,
    ) -> SandboxServiceImpl<MockSandboxGrpcClient> {
        SandboxServiceImpl { client: mock }
    }

    #[test]
    fn test_sandbox_service_create() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::Create(Ok("sb-123".to_string())));
        let svc = make_sandbox_service(mock);

        let sb = svc
            .create("app-1", "img-1", SandboxCreateParams::default())
            .unwrap();
        assert_eq!(sb.sandbox_id, "sb-123");
        assert!(sb.task_id.is_none());
    }

    #[test]
    fn test_sandbox_service_create_invalid_params() {
        let mock = MockSandboxGrpcClient::new();
        let svc = make_sandbox_service(mock);

        let err = svc
            .create(
                "app-1",
                "img-1",
                SandboxCreateParams {
                    cpu: -1.0,
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert!(err.to_string().contains("positive number"));
    }

    #[test]
    fn test_sandbox_service_from_id() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::FromId(Ok(())));
        let svc = make_sandbox_service(mock);

        let sb = svc.from_id("sb-123").unwrap();
        assert_eq!(sb.sandbox_id, "sb-123");
    }

    #[test]
    fn test_sandbox_service_from_id_not_found() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::FromId(Err(ModalError::Grpc(
            tonic::Status::not_found("not found"),
        ))));
        let svc = make_sandbox_service(mock);

        let err = svc.from_id("sb-nonexistent").unwrap_err();
        assert!(matches!(err, ModalError::NotFound(_)));
        assert!(err.to_string().contains("sb-nonexistent"));
    }

    #[test]
    fn test_sandbox_service_from_name() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::FromName(Ok("sb-named-1".to_string())));
        let svc = make_sandbox_service(mock);

        let sb = svc.from_name("my-app", "my-sandbox", None).unwrap();
        assert_eq!(sb.sandbox_id, "sb-named-1");
    }

    #[test]
    fn test_sandbox_service_from_name_not_found() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::FromName(Err(ModalError::Grpc(
            tonic::Status::not_found("not found"),
        ))));
        let svc = make_sandbox_service(mock);

        let err = svc.from_name("my-app", "nonexistent", None).unwrap_err();
        assert!(matches!(err, ModalError::NotFound(_)));
        assert!(err.to_string().contains("nonexistent"));
        assert!(err.to_string().contains("my-app"));
    }

    #[test]
    fn test_sandbox_service_list_empty() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::List(Ok(vec![])));
        let svc = make_sandbox_service(mock);

        let sandboxes = svc.list(None).unwrap();
        assert!(sandboxes.is_empty());
    }

    #[test]
    fn test_sandbox_service_list_single_page() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::List(Ok(vec![
            SandboxListEntry {
                sandbox_id: "sb-1".to_string(),
                created_at: 1000.0,
            },
            SandboxListEntry {
                sandbox_id: "sb-2".to_string(),
                created_at: 999.0,
            },
        ])));
        mock.push(MockSbResponse::List(Ok(vec![])));
        let svc = make_sandbox_service(mock);

        let sandboxes = svc.list(None).unwrap();
        assert_eq!(sandboxes.len(), 2);
        assert_eq!(sandboxes[0].sandbox_id, "sb-1");
        assert_eq!(sandboxes[1].sandbox_id, "sb-2");
    }

    #[test]
    fn test_sandbox_service_list_pagination() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::List(Ok(vec![SandboxListEntry {
            sandbox_id: "sb-1".to_string(),
            created_at: 1000.0,
        }])));
        mock.push(MockSbResponse::List(Ok(vec![SandboxListEntry {
            sandbox_id: "sb-2".to_string(),
            created_at: 500.0,
        }])));
        mock.push(MockSbResponse::List(Ok(vec![])));
        let svc = make_sandbox_service(mock);

        let sandboxes = svc.list(None).unwrap();
        assert_eq!(sandboxes.len(), 2);
    }

    #[test]
    fn test_sandbox_service_get_task_id() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::GetTaskId(Ok((
            Some("task-abc".to_string()),
            false,
        ))));
        let svc = make_sandbox_service(mock);

        let task_id = svc.get_task_id("sb-123").unwrap();
        assert_eq!(task_id, "task-abc");
    }

    #[test]
    fn test_sandbox_service_get_task_id_terminated() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::GetTaskId(Ok((None, true))));
        let svc = make_sandbox_service(mock);

        let err = svc.get_task_id("sb-123").unwrap_err();
        assert!(err.to_string().contains("terminated"));
    }

    #[test]
    fn test_sandbox_service_exec() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::Exec(Ok("exec-456".to_string())));
        let svc = make_sandbox_service(mock);

        let sb = Sandbox::with_task_id("sb-123".to_string(), "task-abc".to_string());
        let exec_id = svc
            .exec(&sb, vec!["echo".to_string(), "hello".to_string()], SandboxExecParams::default())
            .unwrap();
        assert_eq!(exec_id, "exec-456");
    }

    #[test]
    fn test_sandbox_service_exec_no_task_id() {
        let mock = MockSandboxGrpcClient::new();
        let svc = make_sandbox_service(mock);

        let sb = Sandbox::new("sb-123".to_string());
        let err = svc
            .exec(&sb, vec!["echo".to_string()], SandboxExecParams::default())
            .unwrap_err();
        assert!(err.to_string().contains("no task_id"));
    }

    #[test]
    fn test_sandbox_service_exec_args_too_large() {
        let mock = MockSandboxGrpcClient::new();
        let svc = make_sandbox_service(mock);

        let sb = Sandbox::with_task_id("sb-123".to_string(), "task-abc".to_string());
        let large_arg = "a".repeat(MAX_ARG_LEN + 1);
        let err = svc
            .exec(&sb, vec![large_arg], SandboxExecParams::default())
            .unwrap_err();
        assert!(err.to_string().contains("Total length"));
    }

    #[test]
    fn test_sandbox_service_exec_wait() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::ExecWait(Ok(ExecWaitResult {
            exit_code: Some(0),
            completed: true,
        })));
        let svc = make_sandbox_service(mock);

        let result = svc.exec_wait("exec-456", 55.0).unwrap();
        assert!(result.completed);
        assert_eq!(result.exit_code, Some(0));
    }

    #[test]
    fn test_sandbox_service_exec_wait_not_completed() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::ExecWait(Ok(ExecWaitResult {
            exit_code: None,
            completed: false,
        })));
        let svc = make_sandbox_service(mock);

        let result = svc.exec_wait("exec-456", 5.0).unwrap();
        assert!(!result.completed);
        assert!(result.exit_code.is_none());
    }

    #[test]
    fn test_sandbox_service_wait_success() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::Wait(Ok(SandboxWaitResult {
            exit_code: 0,
            success: true,
            exception: None,
        })));
        let svc = make_sandbox_service(mock);

        let result = svc.wait("sb-123", 300.0).unwrap();
        assert!(result.success);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_sandbox_service_wait_failure() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::Wait(Ok(SandboxWaitResult {
            exit_code: 1,
            success: false,
            exception: Some("process exited with code 1".to_string()),
        })));
        let svc = make_sandbox_service(mock);

        let result = svc.wait("sb-123", 300.0).unwrap();
        assert!(!result.success);
        assert_eq!(result.exit_code, 1);
        assert!(result.exception.is_some());
    }

    #[test]
    fn test_sandbox_service_poll_running() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::Poll(Ok(SandboxPollResult {
            exit_code: None,
        })));
        let svc = make_sandbox_service(mock);

        let result = svc.poll("sb-123").unwrap();
        assert!(result.exit_code.is_none());
    }

    #[test]
    fn test_sandbox_service_poll_finished() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::Poll(Ok(SandboxPollResult {
            exit_code: Some(0),
        })));
        let svc = make_sandbox_service(mock);

        let result = svc.poll("sb-123").unwrap();
        assert_eq!(result.exit_code, Some(0));
    }

    #[test]
    fn test_sandbox_service_terminate() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::Terminate(Ok(())));
        let svc = make_sandbox_service(mock);

        svc.terminate("sb-123").unwrap();
    }

    #[test]
    fn test_sandbox_service_terminate_error() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::Terminate(Err(ModalError::Grpc(
            tonic::Status::not_found("sandbox not found"),
        ))));
        let svc = make_sandbox_service(mock);

        let err = svc.terminate("sb-missing").unwrap_err();
        assert!(matches!(err, ModalError::Grpc(_)));
    }

    #[test]
    fn test_sandbox_service_set_tags() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::TagsSet(Ok(())));
        let svc = make_sandbox_service(mock);

        let mut tags = HashMap::new();
        tags.insert("key".to_string(), "value".to_string());
        svc.set_tags("sb-123", &tags).unwrap();
    }

    #[test]
    fn test_sandbox_service_get_tags() {
        let mock = MockSandboxGrpcClient::new();
        let mut expected = HashMap::new();
        expected.insert("env".to_string(), "prod".to_string());
        expected.insert("team".to_string(), "ml".to_string());
        mock.push(MockSbResponse::TagsGet(Ok(expected.clone())));
        let svc = make_sandbox_service(mock);

        let tags = svc.get_tags("sb-123").unwrap();
        assert_eq!(tags, expected);
    }

    #[test]
    fn test_sandbox_service_tunnels() {
        let mock = MockSandboxGrpcClient::new();
        let mut tunnel_map = HashMap::new();
        tunnel_map.insert(
            8080,
            Tunnel {
                host: "tunnel.modal.run".to_string(),
                port: 443,
                unencrypted_host: String::new(),
                unencrypted_port: 0,
            },
        );
        mock.push(MockSbResponse::GetTunnels(Ok(SandboxTunnelsResult {
            timed_out: false,
            tunnels: tunnel_map,
        })));
        let svc = make_sandbox_service(mock);

        let tunnels = svc.tunnels("sb-123", 30.0).unwrap();
        assert_eq!(tunnels.len(), 1);
        assert!(tunnels.contains_key(&8080));
        assert_eq!(tunnels[&8080].host, "tunnel.modal.run");
    }

    #[test]
    fn test_sandbox_service_tunnels_timeout() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::GetTunnels(Ok(SandboxTunnelsResult {
            timed_out: true,
            tunnels: HashMap::new(),
        })));
        let svc = make_sandbox_service(mock);

        let err = svc.tunnels("sb-123", 1.0).unwrap_err();
        assert!(matches!(err, ModalError::SandboxTimeout(_)));
    }

    #[test]
    fn test_sandbox_service_snapshot_filesystem() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::SnapshotFs(Ok(SandboxSnapshotResult {
            image_id: "im-snapshot-1".to_string(),
            success: true,
            exception: None,
        })));
        let svc = make_sandbox_service(mock);

        let image_id = svc.snapshot_filesystem("sb-123", 60.0).unwrap();
        assert_eq!(image_id, "im-snapshot-1");
    }

    #[test]
    fn test_sandbox_service_snapshot_filesystem_failure() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::SnapshotFs(Ok(SandboxSnapshotResult {
            image_id: String::new(),
            success: false,
            exception: Some("disk full".to_string()),
        })));
        let svc = make_sandbox_service(mock);

        let err = svc.snapshot_filesystem("sb-123", 60.0).unwrap_err();
        assert!(err.to_string().contains("disk full"));
    }

    #[test]
    fn test_sandbox_service_snapshot_filesystem_missing_id() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::SnapshotFs(Ok(SandboxSnapshotResult {
            image_id: String::new(),
            success: true,
            exception: None,
        })));
        let svc = make_sandbox_service(mock);

        let err = svc.snapshot_filesystem("sb-123", 60.0).unwrap_err();
        assert!(err.to_string().contains("missing image ID"));
    }

    #[test]
    fn test_sandbox_service_snapshot_directory() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::SnapshotDir(Ok("im-dir-1".to_string())));
        let svc = make_sandbox_service(mock);

        let sb = Sandbox::with_task_id("sb-123".to_string(), "task-1".to_string());
        let image_id = svc.snapshot_directory(&sb, "/app").unwrap();
        assert_eq!(image_id, "im-dir-1");
    }

    #[test]
    fn test_sandbox_service_snapshot_directory_no_task_id() {
        let mock = MockSandboxGrpcClient::new();
        let svc = make_sandbox_service(mock);

        let sb = Sandbox::new("sb-123".to_string());
        let err = svc.snapshot_directory(&sb, "/app").unwrap_err();
        assert!(err.to_string().contains("no task_id"));
    }

    #[test]
    fn test_sandbox_service_snapshot_directory_empty_id() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::SnapshotDir(Ok(String::new())));
        let svc = make_sandbox_service(mock);

        let sb = Sandbox::with_task_id("sb-123".to_string(), "task-1".to_string());
        let err = svc.snapshot_directory(&sb, "/app").unwrap_err();
        assert!(err.to_string().contains("missing `imageId`"));
    }

    #[test]
    fn test_sandbox_service_mount_image() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::MountImage(Ok(())));
        let svc = make_sandbox_service(mock);

        let sb = Sandbox::with_task_id("sb-123".to_string(), "task-1".to_string());
        svc.mount_image(&sb, "/mnt/data", Some("im-123")).unwrap();
    }

    #[test]
    fn test_sandbox_service_mount_image_empty_dir() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::MountImage(Ok(())));
        let svc = make_sandbox_service(mock);

        let sb = Sandbox::with_task_id("sb-123".to_string(), "task-1".to_string());
        svc.mount_image(&sb, "/mnt/empty", None).unwrap();
    }

    #[test]
    fn test_sandbox_service_mount_image_no_task_id() {
        let mock = MockSandboxGrpcClient::new();
        let svc = make_sandbox_service(mock);

        let sb = Sandbox::new("sb-123".to_string());
        let err = svc.mount_image(&sb, "/mnt/data", Some("im-123")).unwrap_err();
        assert!(err.to_string().contains("no task_id"));
    }

    #[test]
    fn test_sandbox_service_create_connect_token() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::ConnectToken(Ok(
            SandboxCreateConnectCredentials {
                url: "https://sb-123.modal.run".to_string(),
                token: "tok-abc".to_string(),
            },
        )));
        let svc = make_sandbox_service(mock);

        let creds = svc.create_connect_token("sb-123", None).unwrap();
        assert_eq!(creds.url, "https://sb-123.modal.run");
        assert_eq!(creds.token, "tok-abc");
    }

    #[test]
    fn test_sandbox_service_create_connect_token_with_metadata() {
        let mock = MockSandboxGrpcClient::new();
        mock.push(MockSbResponse::ConnectToken(Ok(
            SandboxCreateConnectCredentials {
                url: "https://sb-123.modal.run".to_string(),
                token: "tok-xyz".to_string(),
            },
        )));
        let svc = make_sandbox_service(mock);

        let creds = svc
            .create_connect_token(
                "sb-123",
                Some(&SandboxCreateConnectTokenParams {
                    user_metadata: "user-data".to_string(),
                }),
            )
            .unwrap();
        assert_eq!(creds.token, "tok-xyz");
    }
}
