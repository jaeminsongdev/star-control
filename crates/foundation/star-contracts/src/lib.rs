//! Star-Control MCP contract v1.
//!
//! This crate is the only owner of MCP, IPC, registry, manifest, trust, cache,
//! and external-process wire types.  It deliberately has no filesystem,
//! process, or transport dependency.

pub mod canonical;
pub mod fixed_mcp;
pub mod ids;
pub mod ipc;
pub mod manifest;
pub mod registry;
pub mod runtime;
pub mod schema;
pub mod strict_json;
pub mod trust;

pub use canonical::{Sha256Hash, canonical_sha256, jcs_bytes};
pub use ids::{ApprovalId, OperationId, RequestId, ToolTrustId};
pub use manifest::{ToolPackageManifest, parse_manifest_v1};
pub use strict_json::parse_no_duplicate_keys;

/// Frozen MCP contract version from `mcp-implementation-contract.md`.
pub const MCP_CONTRACT_VERSION: u32 = 1;
/// The product supports this protocol and the stated compatibility floor.
pub const MCP_PROTOCOL_CURRENT: &str = "2025-11-25";
pub const MCP_PROTOCOL_COMPATIBILITY_FLOOR: &str = "2025-06-18";
