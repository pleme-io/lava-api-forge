//! lava-api-forge — cloud API spec → typed tatara-lisp bindings.
//!
//! Skips the Terraform provider middleman. Consume the upstream API
//! spec directly (OpenAPI 3 OR AWS botocore service-2.json) and emit
//! a `.tlisp` source file with one typed `(defapi-operation ...)`
//! form per operation + one `(defapi-shape ...)` form per data type.
//!
//! ## Pipeline
//!
//! ```text
//! <service>.openapi.yaml | <service>.botocore.json
//!         │
//!         ▼  spec::from_openapi | spec::from_botocore
//! Service                          ← typed canonical IR
//!         │
//!         ▼  emit::render_service
//! <service>-api.tlisp              ← consumed by tatara-vm directly
//!         │
//!         ▼  lava-provider-gen + magma
//! tfplugin5/6-compatible provider  ← magma loads it as a plugin
//! ```
//!
//! ## Why this exists
//!
//! - Terraform providers are themselves generated from cloud APIs.
//!   Going API → tlisp directly produces 100% of the operations
//!   (provider only exposes a curated subset).
//! - Operators get *both* declarative (defresource style) and
//!   imperative ((ec2/describe-vpcs :region "us-east-1") style)
//!   surfaces from one source of truth.
//! - lava-provider-gen consumes the same Service IR to mint a full
//!   tfplugin5/6 Rust provider — wire-compatible with magma.

#![allow(clippy::module_name_repetitions)]

pub mod emit;
pub mod spec;

pub use emit::render_service;
pub use spec::{
    from_botocore, from_openapi, Method, Operation, ParseError, Service, Shape, ShapeKind,
    ShapeMember,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ForgeError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse: {0}")]
    Parse(#[from] ParseError),
}

/// Drive lava-api-forge from input file paths.
pub fn run_from_file(
    spec_path: &std::path::Path,
    out_path: &std::path::Path,
    kind: SpecKind,
) -> Result<(), ForgeError> {
    let text = std::fs::read_to_string(spec_path)?;
    let svc = match kind {
        SpecKind::Openapi => from_openapi(&text)?,
        SpecKind::Botocore => from_botocore(&text)?,
    };
    let tlisp = render_service(&svc);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_path, tlisp)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecKind {
    Openapi,
    Botocore,
}
