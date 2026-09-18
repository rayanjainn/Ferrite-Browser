//! The concrete backends.
//!
//! Each one's only job is to speak its provider's wire format and hand the
//! raw result to [`crate::guard::finalize`]. Caching, throttling and
//! budgeting are **not** here — they are decorators (§10.5), so that a new
//! backend gets all three for free and cannot accidentally implement one of
//! them slightly differently.

mod mock;

pub use mock::{MockProvider, MockStep};
