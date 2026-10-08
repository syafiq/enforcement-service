//! HAL Enforcement Service - Policy-based access control for WASMHAL
//!
//! This library provides a standalone enforcement layer that sits between
//! WASM components and the ELASTIC TEE HAL, enforcing capability-based
//! access control based on YAML policy files.

pub mod analyzer;
pub mod api;
pub mod config;
pub mod enforcer;
pub mod error;
pub mod service;

pub use analyzer::{LlmAnalyzer, MockAnalyzer};
pub use config::{CapabilitiesConfig, EnforcementMode, EntityConfig, PolicyConfig};
pub use enforcer::{check_workload, extract_imports, import_to_capability};
pub use error::{EnforcementError, Result};
pub use service::{EnforcementService, PolicySource, Session};

/// Version of the enforcement service
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
