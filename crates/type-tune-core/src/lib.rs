//! Fast, deterministic analysis primitives for type-tune.

pub mod ai;
pub mod compiler;
pub mod config;
pub mod destination;
pub mod edit;
pub mod error;
pub mod glob;
pub mod hash;
pub mod model;
pub mod module_path;
pub mod module_specifier;
pub mod options;
pub mod parser;
pub mod path;
pub mod plan;
pub mod render;
pub mod review;
pub mod rewrite;
pub mod scan;
pub mod sort;
pub mod source;
pub mod transaction;

pub use error::{Result, TypeTuneError};

/// Returns the crate's semantic-version string.
#[must_use]
pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::version;

    #[test]
    fn exposes_the_package_version() {
        assert_eq!(version(), "1.0.0");
    }
}
