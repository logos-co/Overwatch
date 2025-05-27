#![cfg_attr(doc, doc = include_str!("../README.md"))]

use crate::services::ServiceData;

pub mod overwatch;
pub mod services;
pub mod utils;

pub use overwatch::errors::DynError;

pub type OpaqueServiceRunner<S, RuntimeServiceId> = services::runner::ServiceRunner<
    <S as ServiceData>::Message,
    <S as ServiceData>::Settings,
    <S as ServiceData>::State,
    <S as ServiceData>::StateOperator,
    RuntimeServiceId,
>;
pub type OpaqueServiceHandle<S> = services::service_handle::ServiceHandle<
    <S as ServiceData>::Message,
    <S as ServiceData>::Settings,
    <S as ServiceData>::State,
    <S as ServiceData>::StateOperator,
>;
pub type OpaqueServiceRunnerHandle<S> = services::runner::ServiceRunnerHandle<
    <S as ServiceData>::Message,
    <S as ServiceData>::Settings,
    <S as ServiceData>::State,
    <S as ServiceData>::StateOperator,
>;
pub type OpaqueServiceResourcesHandle<S, RuntimeServiceId> =
    services::resources::ServiceResourcesHandle<
        <S as ServiceData>::Message,
        <S as ServiceData>::Settings,
        <S as ServiceData>::State,
        RuntimeServiceId,
    >;

#[cfg(feature = "derive")]
pub use overwatch_derive::*;
