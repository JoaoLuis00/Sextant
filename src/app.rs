//! Concrete adapters for the ports defined in `domain`/`engine` — the only
//! place things outside pure Rust (SQL, HTTP, CLI args) get touched.

#[cfg(feature = "storage")]
pub mod storage;
