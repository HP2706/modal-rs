pub mod modal {
    pub mod client {
        tonic::include_proto!("modal.client");
    }
    pub mod task_command_router {
        tonic::include_proto!("modal.task_command_router");
    }
}

// Re-export for convenience
pub use modal::client as modal_proto;
pub use modal::task_command_router;
