//! Every error this crate can produce.
//!
//! There is deliberately no crate-level aggregate error enum: nothing in
//! `ferrite-core` returns one, and an aggregate with no caller is exactly the
//! dead machinery R9 exists to prevent. A downstream crate that needs to mix
//! these adds its own `#[from]` variants.

use thiserror::Error;

/// A newtype identifier was handed a value that violates its invariants.
///
/// `kind` is the identifier type's name (`"CaseId"`, `"PrincipalId"`, …) so
/// one enum can serve every identifier without a variant each.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdError {
    /// The value was empty, or whitespace-only.
    #[error("{kind} must not be empty")]
    Empty {
        /// The identifier type that rejected the value.
        kind: &'static str,
    },

    /// The value exceeded the identifier's length ceiling.
    #[error("{kind} is {len} bytes, over the {max}-byte maximum")]
    TooLong {
        /// The identifier type that rejected the value.
        kind: &'static str,
        /// The rejected value's length, in bytes.
        len: usize,
        /// The ceiling, in bytes.
        max: usize,
    },

    /// The value contained a character outside the identifier's alphabet.
    #[error("{kind} contains {ch:?}, which is outside its alphabet ({allowed})")]
    DisallowedCharacter {
        /// The identifier type that rejected the value.
        kind: &'static str,
        /// The first offending character.
        ch: char,
        /// A human-readable description of the permitted alphabet.
        allowed: &'static str,
    },

    /// An [`Origin`](crate::ids::Origin) was not parseable as an absolute URL.
    #[error("origin {value:?} is not an absolute URL: {source}")]
    OriginNotAUrl {
        /// The rejected input.
        value: String,
        /// The underlying parse failure.
        #[source]
        source: url::ParseError,
    },

    /// An [`Origin`](crate::ids::Origin) had a scheme that carries no tuple
    /// origin, so nothing about it can be scoped.
    #[error(
        "origin {value:?} has scheme {scheme:?}; only http and https carry a \
         tuple origin that an OriginScope can admit"
    )]
    OriginUnsupportedScheme {
        /// The rejected input.
        value: String,
        /// The scheme that was found.
        scheme: String,
    },

    /// An [`Origin`](crate::ids::Origin) parsed but carried no host.
    #[error("origin {value:?} has no host")]
    OriginMissingHost {
        /// The rejected input.
        value: String,
    },
}
