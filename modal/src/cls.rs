use std::time::Duration;

use crate::error::ModalError;
use crate::retries::Retries;
use crate::secret::Secret;
use crate::volume::Volume;

/// ServiceOptions holds runtime configuration for a Modal class.
#[derive(Debug, Clone, Default)]
pub struct ServiceOptions {
    pub cpu: Option<f64>,
    pub cpu_limit: Option<f64>,
    pub memory_mib: Option<i32>,
    pub memory_limit_mib: Option<i32>,
    pub gpu: Option<String>,
    pub env: Option<std::collections::HashMap<String, String>>,
    pub secrets: Option<Vec<Secret>>,
    pub volumes: Option<std::collections::HashMap<String, Volume>>,
    pub retries: Option<Retries>,
    pub max_containers: Option<i32>,
    pub buffer_containers: Option<i32>,
    pub scaledown_window: Option<Duration>,
    pub timeout: Option<Duration>,
    pub max_concurrent_inputs: Option<i32>,
    pub target_concurrent_inputs: Option<i32>,
    pub batch_max_size: Option<i32>,
    pub batch_wait: Option<Duration>,
}

/// Resources proto representation.
#[derive(Debug, Clone, Default)]
pub struct ResourcesProto {
    pub milli_cpu: u32,
    pub milli_cpu_max: u32,
    pub memory_mb: u32,
    pub memory_mb_max: u32,
}

/// FunctionOptionsProto representation.
#[derive(Debug, Clone, Default)]
pub struct FunctionOptionsProto {
    pub resources: Option<ResourcesProto>,
}

/// Build function options proto from service options.
/// Returns None if options is None or empty.
pub fn build_function_options_proto(
    options: Option<&ServiceOptions>,
) -> Result<Option<FunctionOptionsProto>, ModalError> {
    let options = match options {
        Some(o) if has_options(o) => o,
        _ => return Ok(None),
    };

    let mut proto = FunctionOptionsProto::default();

    if options.cpu.is_some()
        || options.cpu_limit.is_some()
        || options.memory_mib.is_some()
        || options.memory_limit_mib.is_some()
        || options.gpu.is_some()
    {
        let mut resources = ResourcesProto::default();

        // CPU validation
        if options.cpu.is_none() && options.cpu_limit.is_some() {
            return Err(ModalError::Invalid(
                "must also specify non-zero CPU request when CPULimit is specified".to_string(),
            ));
        }
        if let Some(cpu) = options.cpu {
            if cpu <= 0.0 {
                return Err(ModalError::Invalid(format!(
                    "the CPU request ({}) must be a positive number",
                    cpu
                )));
            }
            resources.milli_cpu = (cpu * 1000.0) as u32;

            if let Some(cpu_limit) = options.cpu_limit {
                if cpu_limit < cpu {
                    return Err(ModalError::Invalid(format!(
                        "the CPU request ({:.*}) cannot be higher than CPULimit ({:.*})",
                        6, cpu, 6, cpu_limit
                    )));
                }
                resources.milli_cpu_max = (cpu_limit * 1000.0) as u32;
            }
        }

        // Memory validation
        if options.memory_mib.is_none() && options.memory_limit_mib.is_some() {
            return Err(ModalError::Invalid(
                "must also specify non-zero MemoryMiB request when MemoryLimitMiB is specified"
                    .to_string(),
            ));
        }
        if let Some(memory) = options.memory_mib {
            if memory <= 0 {
                return Err(ModalError::Invalid(format!(
                    "the MemoryMiB request ({}) must be a positive number",
                    memory
                )));
            }
            resources.memory_mb = memory as u32;

            if let Some(memory_limit) = options.memory_limit_mib {
                if memory_limit < memory {
                    return Err(ModalError::Invalid(format!(
                        "the MemoryMiB request ({}) cannot be higher than MemoryLimitMiB ({})",
                        memory, memory_limit
                    )));
                }
                resources.memory_mb_max = memory_limit as u32;
            }
        }

        proto.resources = Some(resources);
    }

    Ok(Some(proto))
}

fn has_options(o: &ServiceOptions) -> bool {
    o.cpu.is_some()
        || o.cpu_limit.is_some()
        || o.memory_mib.is_some()
        || o.memory_limit_mib.is_some()
        || o.gpu.is_some()
        || o.env.is_some()
        || o.secrets.is_some()
        || o.volumes.is_some()
        || o.retries.is_some()
        || o.max_containers.is_some()
        || o.buffer_containers.is_some()
        || o.scaledown_window.is_some()
        || o.timeout.is_some()
        || o.max_concurrent_inputs.is_some()
        || o.target_concurrent_inputs.is_some()
        || o.batch_max_size.is_some()
        || o.batch_wait.is_some()
}

/// Merge two service options, with `new` taking precedence over `base`.
pub fn merge_service_options(
    base: Option<&ServiceOptions>,
    new: Option<&ServiceOptions>,
) -> ServiceOptions {
    let base = match base {
        Some(b) => b.clone(),
        None => {
            return new.cloned().unwrap_or_default();
        }
    };
    let new = match new {
        Some(n) => n,
        None => return base,
    };

    ServiceOptions {
        cpu: new.cpu.or(base.cpu),
        cpu_limit: new.cpu_limit.or(base.cpu_limit),
        memory_mib: new.memory_mib.or(base.memory_mib),
        memory_limit_mib: new.memory_limit_mib.or(base.memory_limit_mib),
        gpu: new.gpu.clone().or(base.gpu),
        env: new.env.clone().or(base.env),
        secrets: new.secrets.clone().or(base.secrets),
        volumes: new.volumes.clone().or(base.volumes),
        retries: new.retries.clone().or(base.retries),
        max_containers: new.max_containers.or(base.max_containers),
        buffer_containers: new.buffer_containers.or(base.buffer_containers),
        scaledown_window: new.scaledown_window.or(base.scaledown_window),
        timeout: new.timeout.or(base.timeout),
        max_concurrent_inputs: new.max_concurrent_inputs.or(base.max_concurrent_inputs),
        target_concurrent_inputs: new
            .target_concurrent_inputs
            .or(base.target_concurrent_inputs),
        batch_max_size: new.batch_max_size.or(base.batch_max_size),
        batch_wait: new.batch_wait.or(base.batch_wait),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_function_options_proto_nil() {
        let result = build_function_options_proto(None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_build_function_options_proto_with_cpu_and_cpu_limit() {
        let opts = ServiceOptions {
            cpu: Some(2.0),
            cpu_limit: Some(4.5),
            ..Default::default()
        };
        let result = build_function_options_proto(Some(&opts)).unwrap().unwrap();
        let resources = result.resources.unwrap();
        assert_eq!(resources.milli_cpu, 2000);
        assert_eq!(resources.milli_cpu_max, 4500);
    }

    #[test]
    fn test_build_function_options_proto_cpu_limit_lower_than_cpu() {
        let opts = ServiceOptions {
            cpu: Some(4.0),
            cpu_limit: Some(2.0),
            ..Default::default()
        };
        let err = build_function_options_proto(Some(&opts)).unwrap_err();
        assert!(
            err.to_string()
                .contains("the CPU request (4.000000) cannot be higher than CPULimit (2.000000)"),
            "got: {}",
            err
        );
    }

    #[test]
    fn test_build_function_options_proto_cpu_limit_without_cpu() {
        let opts = ServiceOptions {
            cpu_limit: Some(4.0),
            ..Default::default()
        };
        let err = build_function_options_proto(Some(&opts)).unwrap_err();
        assert!(
            err.to_string()
                .contains("must also specify non-zero CPU request when CPULimit is specified"),
            "got: {}",
            err
        );
    }

    #[test]
    fn test_build_function_options_proto_with_memory_and_memory_limit() {
        let opts = ServiceOptions {
            memory_mib: Some(1024),
            memory_limit_mib: Some(2048),
            ..Default::default()
        };
        let result = build_function_options_proto(Some(&opts)).unwrap().unwrap();
        let resources = result.resources.unwrap();
        assert_eq!(resources.memory_mb, 1024);
        assert_eq!(resources.memory_mb_max, 2048);
    }

    #[test]
    fn test_build_function_options_proto_memory_limit_lower_than_memory() {
        let opts = ServiceOptions {
            memory_mib: Some(2048),
            memory_limit_mib: Some(1024),
            ..Default::default()
        };
        let err = build_function_options_proto(Some(&opts)).unwrap_err();
        assert!(
            err.to_string()
                .contains("the MemoryMiB request (2048) cannot be higher than MemoryLimitMiB (1024)"),
            "got: {}",
            err
        );
    }

    #[test]
    fn test_build_function_options_proto_memory_limit_without_memory() {
        let opts = ServiceOptions {
            memory_limit_mib: Some(2048),
            ..Default::default()
        };
        let err = build_function_options_proto(Some(&opts)).unwrap_err();
        assert!(
            err.to_string()
                .contains("must also specify non-zero MemoryMiB request when MemoryLimitMiB is specified"),
            "got: {}",
            err
        );
    }

    #[test]
    fn test_build_function_options_proto_negative_cpu() {
        let opts = ServiceOptions {
            cpu: Some(-1.0),
            ..Default::default()
        };
        let err = build_function_options_proto(Some(&opts)).unwrap_err();
        assert!(err.to_string().contains("must be a positive number"));
    }

    #[test]
    fn test_build_function_options_proto_zero_cpu() {
        let opts = ServiceOptions {
            cpu: Some(0.0),
            ..Default::default()
        };
        let err = build_function_options_proto(Some(&opts)).unwrap_err();
        assert!(err.to_string().contains("must be a positive number"));
    }

    #[test]
    fn test_build_function_options_proto_negative_memory() {
        let opts = ServiceOptions {
            memory_mib: Some(-100),
            ..Default::default()
        };
        let err = build_function_options_proto(Some(&opts)).unwrap_err();
        assert!(err.to_string().contains("must be a positive number"));
    }

    #[test]
    fn test_build_function_options_proto_zero_memory() {
        let opts = ServiceOptions {
            memory_mib: Some(0),
            ..Default::default()
        };
        let err = build_function_options_proto(Some(&opts)).unwrap_err();
        assert!(err.to_string().contains("must be a positive number"));
    }
}
