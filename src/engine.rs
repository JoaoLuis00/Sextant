//! Business logic — stateless and deterministic.
//!
//! The Engine owns every calculation that could reasonably be done more than
//! one way (average cost vs. FIFO vs. LIFO), and holds no state between calls.

pub mod dispatch;
pub mod errors;
pub mod portfolio_engine;
pub mod repository;
