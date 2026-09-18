//! Ferrite's core contracts: identifiers, the capability/primitive taxonomy,
//! origin scoping, time, and the error types those need.
//!
//! This crate holds **types and their invariants only** — no business logic.
//! It sits at the bottom of the dependency graph
//! (`core ← {model, audit, engine} ← ipi ← agent ← {ui, eval, cli}`) and must
//! never depend on another `ferrite-*` crate.

#![deny(dead_code)]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![deny(rustdoc::broken_intra_doc_links)]

pub mod error;
pub mod ids;
pub mod scope;

pub use error::{IdError, ScopeError};
pub use ids::{CaseId, ExecId, Origin, PrincipalId};
pub use scope::{DomainSuffix, OriginScope, Specificity};
