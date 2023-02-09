//! Overwatch is a framework to easily construct applications that requires of several independent
//! parts that needs communication between them.
//! Everything is self contained and it matches somewhat the advantages of microservices.
//!
//! ## Design Goals
//!
//! - Modularity:
//!     - Components should be self-contained (as possible)
//!     - Communication relations between components should be specifically defined
//!     - Components should be mockable. This is rather important for measurements and testing.
//!
//! - Single responsibility:
//!     - It is easier to isolate problems
//!     - Minimal sharing when unavoidable
//!
//! - Debuggeability
//!     - Easy to track workflow
//!     - Easy to test
//!     - Easy to measure
//!     - Asynchronous Communication
//!
//! ## Main components
//!
//! - Overwatch: the main messenger relay component (internal communications). It is also be responsible of managing other components lifecycle and handling configuration updates.
//! - Services (handled by the *overwatch*)

pub mod overwatch;
pub mod services;
pub mod utils;

pub type DynError = Box<dyn std::error::Error + Send + Sync + 'static>;
