//! Overwatch is a framework to easily construct applications that are composed of several independent
//! parts requiring communication between them.
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
//! - Debuggability
//!     - Easy to track workflow
//!     - Easy to test
//!     - Easy to measure
//!     - Asynchronous communication
//!
//! ## Main components
//!
//! - Overwatch: the main messenger relay component (internal communications). It is also responsible for managing other components lifecycle and handling configuration updates.
//! - Services (handled by the *overwatch*)

use crate::services::ServiceData;

pub mod overwatch;
pub mod services;
pub mod utils;

pub type DynError = Box<dyn std::error::Error + Send + Sync + 'static>;
pub type ServiceHandle<S> = crate::services::handle::ServiceHandle<
    <S as ServiceData>::Message,
    <S as ServiceData>::Settings,
    S,
    <S as ServiceData>::State,
>;
pub type ServiceStateHandle<S> = crate::services::handle::ServiceStateHandle<
    <S as ServiceData>::Message,
    <S as ServiceData>::Settings,
    S,
    <S as ServiceData>::State,
>;

#[cfg(feature = "derive")]
pub use overwatch_derive::*;
