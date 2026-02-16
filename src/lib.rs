//! HAL Enforcement Service - Policy-based access control for WASMHAL
//! 
//! This library provides a standalone enforcement layer that sits between
//! WASM components and the ELASTIC TEE HAL, enforcing capability-based
//! access control based on YAML policy files.

pub mod config;
pub mod service;
pub mod api;
pub mod error;

pub use config::{PolicyConfig, EntityConfig, CapabilitiesConfig};
pub use service::EnforcementService;
pub use error::{EnforcementError, Result};

/// Version of the enforcement service
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
