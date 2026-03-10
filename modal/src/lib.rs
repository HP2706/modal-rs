//! A lightweight, idiomatic Rust SDK for [Modal](https://modal.com).
//!
//! This crate mirrors the core feature-set of Modal's Python SDK while feeling
//! natural in Rust:
//!
//! - Spin up **Sandboxes** — fast, secure, ephemeral VMs for running code.
//! - Invoke Modal **Functions** and manage their inputs / outputs.
//! - Read, write, and list files in Modal **Volumes**.
//! - Create or inspect containers, streams, and logs.
//! - Push and pop items from **Queues** with pickle-compatible serialization.
//! - Build container **Images** layer by layer.
//!
//! **What it does not do:** deploying Modal Functions. Deployment is still
//! handled in Python; this crate is for calling and orchestrating them
//! from other projects.
//!
//! # Configuration
//!
//! The config file path can be customized via `MODAL_CONFIG_PATH` (defaults to `~/.modal.toml`).
//!
//! ## Authentication
//!
//! At runtime the SDK resolves credentials in this order:
//!
//! 1. Environment variables:
//!    `MODAL_TOKEN_ID`, `MODAL_TOKEN_SECRET`, `MODAL_ENVIRONMENT` (optional)
//! 2. A profile explicitly requested via `MODAL_PROFILE`
//! 3. A profile marked `active = true` in `~/.modal.toml`
//!
//! ## Logging
//!
//! The SDK logging level can be controlled in multiple ways (in order of precedence):
//!
//! 1. `MODAL_LOGLEVEL` environment variable
//! 2. `loglevel` field in the active profile in `~/.modal.toml`
//! 3. Defaults to WARN
//!
//! Supported values are `DEBUG`, `INFO`, `WARN`, and `ERROR` (case-insensitive).
//!
//! Logs are written to stderr.

pub mod error;
pub mod config;
pub mod logger;
pub mod retries;
pub mod app;
pub mod serialization;
pub mod cloud_bucket_mount;
pub mod cls;
pub mod sandbox;
pub mod secret;
pub mod client;
pub mod auth_token_manager;
pub mod task_command_router;
pub mod function;
pub mod function_call;
pub mod image;
pub mod volume;
pub mod queue;
pub mod proxy;
pub mod sandbox_filesystem;
pub mod invocation;
pub mod interceptors;
pub mod ephemeral;
pub mod pickle;
pub mod transport;

#[cfg(test)]
pub(crate) mod grpc_mock;

pub use error::ModalError;
