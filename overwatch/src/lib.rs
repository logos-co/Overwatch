#![cfg_attr(doc, doc = include_str!("../README.md"))]

use crate::services::ServiceData;

pub mod overwatch;
pub mod services;
pub mod utils;

pub type DynError = Box<dyn std::error::Error + Send + Sync + 'static>;
pub type OpaqueServiceHandle<S, RuntimeServiceId> = services::handle::ServiceHandle<
    <S as ServiceData>::Message,
    <S as ServiceData>::Settings,
    <S as ServiceData>::State,
    RuntimeServiceId,
>;
pub type OpaqueServiceStateHandle<S, RuntimeServiceId> = services::handle::ServiceStateHandle<
    <S as ServiceData>::Message,
    <S as ServiceData>::Settings,
    <S as ServiceData>::State,
    RuntimeServiceId,
>;

#[cfg(feature = "derive")]
pub use overwatch_derive::*;
