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
}
