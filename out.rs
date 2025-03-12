#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
use overwatch::{overwatch::OverwatchRunner, OpaqueServiceHandle};
use overwatch_derive::Services;
use crate::{
    service_ping::PingService, service_pong::PongService, settings::PingSettings,
};
mod messages {
    use overwatch::services::relay::RelayMessage;
    pub enum PingMessage {
        Pong,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for PingMessage {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(f, "Pong")
        }
    }
    impl RelayMessage for PingMessage {}
    pub enum PongMessage {
        Ping,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for PongMessage {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::write_str(f, "Ping")
        }
    }
    impl RelayMessage for PongMessage {}
}
mod operators {
    use std::fmt::Debug;
    use overwatch::services::state::StateOperator;
    use crate::{settings::PingSettings, states::PingState};
    pub struct StateSaveOperator {
        save_path: String,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for StateSaveOperator {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field1_finish(
                f,
                "StateSaveOperator",
                "save_path",
                &&self.save_path,
            )
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for StateSaveOperator {
        #[inline]
        fn clone(&self) -> StateSaveOperator {
            StateSaveOperator {
                save_path: ::core::clone::Clone::clone(&self.save_path),
            }
        }
    }
    impl StateOperator for StateSaveOperator {
        type StateInput = PingState;
        type Settings = PingSettings;
        type LoadError = std::io::Error;
        fn try_load(
            settings: &Self::Settings,
        ) -> Result<Option<Self::StateInput>, Self::LoadError> {
            let state_string = std::fs::read_to_string(&settings.state_save_path)?;
            serde_json::from_str(&state_string)
                .map_err(|error| std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    error,
                ))
        }
        fn from_settings(settings: Self::Settings) -> Self {
            Self {
                save_path: settings.state_save_path,
            }
        }
        #[allow(
            elided_named_lifetimes,
            clippy::async_yields_async,
            clippy::diverging_sub_expression,
            clippy::let_unit_value,
            clippy::needless_arbitrary_self_type,
            clippy::no_effect_underscore_binding,
            clippy::shadow_same,
            clippy::type_complexity,
            clippy::type_repetition_in_bounds,
            clippy::used_underscore_binding
        )]
        fn run<'life0, 'async_trait>(
            &'life0 mut self,
            state: Self::StateInput,
        ) -> ::core::pin::Pin<
            Box<
                dyn ::core::future::Future<
                    Output = (),
                > + ::core::marker::Send + 'async_trait,
            >,
        >
        where
            'life0: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                let mut __self = self;
                let state = state;
                let () = {
                    let json_state = serde_json::to_string(&state)
                        .expect("Failed to serialize state");
                    std::fs::write(&__self.save_path, json_state).unwrap();
                };
            })
        }
    }
}
mod service_ping {
    use std::time::Duration;
    use overwatch::{
        services::{ServiceCore, ServiceData, ServiceId},
        DynError, OpaqueServiceStateHandle,
    };
    use tokio::time::sleep;
    use crate::{
        messages::{PingMessage, PongMessage},
        operators::StateSaveOperator, service_pong::PongService, settings::PingSettings,
        states::PingState,
    };
    pub struct PingService {
        service_state_handle: OpaqueServiceStateHandle<Self>,
        initial_state: <Self as ServiceData>::State,
    }
    impl ServiceData for PingService {
        const SERVICE_ID: ServiceId = "ping";
        type Settings = PingSettings;
        type State = PingState;
        type StateOperator = StateSaveOperator;
        type Message = PingMessage;
    }
    impl ServiceCore for PingService {
        fn init(
            service_state_handle: OpaqueServiceStateHandle<Self>,
            initial_state: Self::State,
        ) -> Result<Self, DynError> {
            Ok(Self {
                service_state_handle,
                initial_state,
            })
        }
        #[allow(
            elided_named_lifetimes,
            clippy::async_yields_async,
            clippy::diverging_sub_expression,
            clippy::let_unit_value,
            clippy::needless_arbitrary_self_type,
            clippy::no_effect_underscore_binding,
            clippy::shadow_same,
            clippy::type_complexity,
            clippy::type_repetition_in_bounds,
            clippy::used_underscore_binding
        )]
        fn run<'async_trait>(
            self,
        ) -> ::core::pin::Pin<
            Box<
                dyn ::core::future::Future<
                    Output = Result<(), DynError>,
                > + ::core::marker::Send + 'async_trait,
            >,
        >
        where
            Self: 'async_trait,
        {
            Box::pin(async move {
                if let ::core::option::Option::Some(__ret) = ::core::option::Option::None::<
                    Result<(), DynError>,
                > {
                    #[allow(unreachable_code)] return __ret;
                }
                let __self = self;
                let __ret: Result<(), DynError> = {
                    let Self { service_state_handle, initial_state } = __self;
                    let mut inbound_relay = service_state_handle.inbound_relay;
                    let pong_outbound_relay = service_state_handle
                        .overwatch_handle
                        .relay::<PongService>()
                        .connect()
                        .await?;
                    let Self::State { mut pong_count } = initial_state;
                    loop {
                        {
                            #[doc(hidden)]
                            mod __tokio_select_util {
                                pub(super) enum Out<_0, _1, _2> {
                                    _0(_0),
                                    _1(_1),
                                    _2(_2),
                                    Disabled,
                                }
                                pub(super) type Mask = u8;
                            }
                            use ::tokio::macros::support::Future;
                            use ::tokio::macros::support::Pin;
                            use ::tokio::macros::support::Poll::{Ready, Pending};
                            const BRANCHES: u32 = 3;
                            let mut disabled: __tokio_select_util::Mask = Default::default();
                            if !true {
                                let mask: __tokio_select_util::Mask = 1 << 0;
                                disabled |= mask;
                            }
                            if !true {
                                let mask: __tokio_select_util::Mask = 1 << 1;
                                disabled |= mask;
                            }
                            if !true {
                                let mask: __tokio_select_util::Mask = 1 << 2;
                                disabled |= mask;
                            }
                            let mut output = {
                                let futures_init = (
                                    sleep(Duration::from_secs(1)),
                                    inbound_relay.recv(),
                                    async { pong_count >= 30 },
                                );
                                let mut futures = (
                                    ::tokio::macros::support::IntoFuture::into_future(
                                        futures_init.0,
                                    ),
                                    ::tokio::macros::support::IntoFuture::into_future(
                                        futures_init.1,
                                    ),
                                    ::tokio::macros::support::IntoFuture::into_future(
                                        futures_init.2,
                                    ),
                                );
                                let mut futures = &mut futures;
                                ::tokio::macros::support::poll_fn(|cx| {
                                        let mut is_pending = false;
                                        let start = {
                                            ::tokio::macros::support::thread_rng_n(BRANCHES)
                                        };
                                        for i in 0..BRANCHES {
                                            let branch;
                                            #[allow(clippy::modulo_one)]
                                            {
                                                branch = (start + i) % BRANCHES;
                                            }
                                            match branch {
                                                #[allow(unreachable_code)]
                                                0 => {
                                                    let mask = 1 << branch;
                                                    if disabled & mask == mask {
                                                        continue;
                                                    }
                                                    let (fut, ..) = &mut *futures;
                                                    let mut fut = unsafe { Pin::new_unchecked(fut) };
                                                    let out = match Future::poll(fut, cx) {
                                                        Ready(out) => out,
                                                        Pending => {
                                                            is_pending = true;
                                                            continue;
                                                        }
                                                    };
                                                    disabled |= mask;
                                                    #[allow(unused_variables)] #[allow(unused_mut)]
                                                    match &out {
                                                        () => {}
                                                        _ => continue,
                                                    }
                                                    return Ready(__tokio_select_util::Out::_0(out));
                                                }
                                                #[allow(unreachable_code)]
                                                1 => {
                                                    let mask = 1 << branch;
                                                    if disabled & mask == mask {
                                                        continue;
                                                    }
                                                    let (_, fut, ..) = &mut *futures;
                                                    let mut fut = unsafe { Pin::new_unchecked(fut) };
                                                    let out = match Future::poll(fut, cx) {
                                                        Ready(out) => out,
                                                        Pending => {
                                                            is_pending = true;
                                                            continue;
                                                        }
                                                    };
                                                    disabled |= mask;
                                                    #[allow(unused_variables)] #[allow(unused_mut)]
                                                    match &out {
                                                        Some(message) => {}
                                                        _ => continue,
                                                    }
                                                    return Ready(__tokio_select_util::Out::_1(out));
                                                }
                                                #[allow(unreachable_code)]
                                                2 => {
                                                    let mask = 1 << branch;
                                                    if disabled & mask == mask {
                                                        continue;
                                                    }
                                                    let (_, _, fut, ..) = &mut *futures;
                                                    let mut fut = unsafe { Pin::new_unchecked(fut) };
                                                    let out = match Future::poll(fut, cx) {
                                                        Ready(out) => out,
                                                        Pending => {
                                                            is_pending = true;
                                                            continue;
                                                        }
                                                    };
                                                    disabled |= mask;
                                                    #[allow(unused_variables)] #[allow(unused_mut)]
                                                    match &out {
                                                        true => {}
                                                        _ => continue,
                                                    }
                                                    return Ready(__tokio_select_util::Out::_2(out));
                                                }
                                                _ => {
                                                    ::core::panicking::panic_fmt(
                                                        format_args!(
                                                            "internal error: entered unreachable code: {0}",
                                                            format_args!(
                                                                "reaching this means there probably is an off by one bug",
                                                            ),
                                                        ),
                                                    );
                                                }
                                            }
                                        }
                                        if is_pending {
                                            Pending
                                        } else {
                                            Ready(__tokio_select_util::Out::Disabled)
                                        }
                                    })
                                    .await
                            };
                            match output {
                                __tokio_select_util::Out::_0(()) => {
                                    {
                                        ::std::io::_print(format_args!("Sending Ping\n"));
                                    };
                                    pong_outbound_relay.send(PongMessage::Ping).await.unwrap();
                                }
                                __tokio_select_util::Out::_1(Some(message)) => {
                                    match message {
                                        PingMessage::Pong => {
                                            pong_count += 1;
                                            service_state_handle
                                                .state_updater
                                                .update(Self::State { pong_count });
                                            {
                                                ::std::io::_print(
                                                    format_args!("Received Pong. Total: {0}\n", pong_count),
                                                );
                                            };
                                        }
                                    }
                                }
                                __tokio_select_util::Out::_2(true) => {
                                    {
                                        ::std::io::_print(
                                            format_args!("Received {0} Pongs. Exiting...\n", pong_count),
                                        );
                                    };
                                    break;
                                }
                                __tokio_select_util::Out::Disabled => {
                                    ::core::panicking::panic_fmt(
                                        format_args!(
                                            "all branches are disabled and there is no else branch",
                                        ),
                                    );
                                }
                                _ => {
                                    ::core::panicking::panic_fmt(
                                        format_args!(
                                            "internal error: entered unreachable code: {0}",
                                            format_args!("failed to match bind"),
                                        ),
                                    );
                                }
                            }
                        }
                    }
                    service_state_handle.overwatch_handle.shutdown().await;
                    Ok(())
                };
                #[allow(unreachable_code)] __ret
            })
        }
    }
}
mod service_pong {
    use overwatch::{
        services::{
            state::{NoOperator, NoState},
            ServiceCore, ServiceData, ServiceId,
        },
        DynError, OpaqueServiceStateHandle,
    };
    use crate::{
        messages::{PingMessage, PongMessage},
        service_ping::PingService,
    };
    pub struct PongService {
        service_state_handle: OpaqueServiceStateHandle<Self>,
    }
    impl ServiceData for PongService {
        const SERVICE_ID: ServiceId = "pong";
        type Settings = ();
        type State = NoState<Self::Settings>;
        type StateOperator = NoOperator<Self::State, Self::Settings>;
        type Message = PongMessage;
    }
    impl ServiceCore for PongService {
        fn init(
            service_state_handle: OpaqueServiceStateHandle<Self>,
            _initial_state: Self::State,
        ) -> Result<Self, DynError> {
            Ok(Self { service_state_handle })
        }
        #[allow(
            elided_named_lifetimes,
            clippy::async_yields_async,
            clippy::diverging_sub_expression,
            clippy::let_unit_value,
            clippy::needless_arbitrary_self_type,
            clippy::no_effect_underscore_binding,
            clippy::shadow_same,
            clippy::type_complexity,
            clippy::type_repetition_in_bounds,
            clippy::used_underscore_binding
        )]
        fn run<'async_trait>(
            self,
        ) -> ::core::pin::Pin<
            Box<
                dyn ::core::future::Future<
                    Output = Result<(), DynError>,
                > + ::core::marker::Send + 'async_trait,
            >,
        >
        where
            Self: 'async_trait,
        {
            Box::pin(async move {
                if let ::core::option::Option::Some(__ret) = ::core::option::Option::None::<
                    Result<(), DynError>,
                > {
                    #[allow(unreachable_code)] return __ret;
                }
                let __self = self;
                let __ret: Result<(), DynError> = {
                    let Self { service_state_handle } = __self;
                    let mut inbound_relay = service_state_handle.inbound_relay;
                    let ping_outbound_relay = service_state_handle
                        .overwatch_handle
                        .relay::<PingService>()
                        .connect()
                        .await?;
                    while let Some(message) = inbound_relay.recv().await {
                        match message {
                            PongMessage::Ping => {
                                {
                                    ::std::io::_print(
                                        format_args!("Received Ping. Sending Pong.\n"),
                                    );
                                };
                                ping_outbound_relay.send(PingMessage::Pong).await.unwrap();
                            }
                        }
                    }
                    Ok(())
                };
                #[allow(unreachable_code)] __ret
            })
        }
    }
}
mod settings {
    pub struct PingSettings {
        pub(crate) state_save_path: String,
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for PingSettings {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field1_finish(
                f,
                "PingSettings",
                "state_save_path",
                &&self.state_save_path,
            )
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for PingSettings {
        #[inline]
        fn clone(&self) -> PingSettings {
            PingSettings {
                state_save_path: ::core::clone::Clone::clone(&self.state_save_path),
            }
        }
    }
}
mod states {
    use overwatch::services::state::ServiceState;
    use serde::{Deserialize, Serialize};
    use crate::settings::PingSettings;
    pub enum PingStateError {}
    #[allow(unused_qualifications)]
    #[automatically_derived]
    impl ::thiserror::__private::Error for PingStateError {}
    #[allow(unused_qualifications)]
    #[automatically_derived]
    impl ::core::fmt::Display for PingStateError {
        fn fmt(&self, __formatter: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            #[allow(unused_variables, deprecated, clippy::used_underscore_binding)]
            match *self {}
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for PingStateError {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {}
        }
    }
    pub struct PingState {
        pub pong_count: u32,
    }
    #[automatically_derived]
    impl ::core::default::Default for PingState {
        #[inline]
        fn default() -> PingState {
            PingState {
                pong_count: ::core::default::Default::default(),
            }
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for PingState {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            ::core::fmt::Formatter::debug_struct_field1_finish(
                f,
                "PingState",
                "pong_count",
                &&self.pong_count,
            )
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for PingState {
        #[inline]
        fn clone(&self) -> PingState {
            PingState {
                pong_count: ::core::clone::Clone::clone(&self.pong_count),
            }
        }
    }
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl _serde::Serialize for PingState {
            fn serialize<__S>(
                &self,
                __serializer: __S,
            ) -> _serde::__private::Result<__S::Ok, __S::Error>
            where
                __S: _serde::Serializer,
            {
                let mut __serde_state = _serde::Serializer::serialize_struct(
                    __serializer,
                    "PingState",
                    false as usize + 1,
                )?;
                _serde::ser::SerializeStruct::serialize_field(
                    &mut __serde_state,
                    "pong_count",
                    &self.pong_count,
                )?;
                _serde::ser::SerializeStruct::end(__serde_state)
            }
        }
    };
    #[doc(hidden)]
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        #[allow(unused_extern_crates, clippy::useless_attribute)]
        extern crate serde as _serde;
        #[automatically_derived]
        impl<'de> _serde::Deserialize<'de> for PingState {
            fn deserialize<__D>(
                __deserializer: __D,
            ) -> _serde::__private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                #[allow(non_camel_case_types)]
                #[doc(hidden)]
                enum __Field {
                    __field0,
                    __ignore,
                }
                #[doc(hidden)]
                struct __FieldVisitor;
                #[automatically_derived]
                impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
                    type Value = __Field;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(
                            __formatter,
                            "field identifier",
                        )
                    }
                    fn visit_u64<__E>(
                        self,
                        __value: u64,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            0u64 => _serde::__private::Ok(__Field::__field0),
                            _ => _serde::__private::Ok(__Field::__ignore),
                        }
                    }
                    fn visit_str<__E>(
                        self,
                        __value: &str,
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            "pong_count" => _serde::__private::Ok(__Field::__field0),
                            _ => _serde::__private::Ok(__Field::__ignore),
                        }
                    }
                    fn visit_bytes<__E>(
                        self,
                        __value: &[u8],
                    ) -> _serde::__private::Result<Self::Value, __E>
                    where
                        __E: _serde::de::Error,
                    {
                        match __value {
                            b"pong_count" => _serde::__private::Ok(__Field::__field0),
                            _ => _serde::__private::Ok(__Field::__ignore),
                        }
                    }
                }
                #[automatically_derived]
                impl<'de> _serde::Deserialize<'de> for __Field {
                    #[inline]
                    fn deserialize<__D>(
                        __deserializer: __D,
                    ) -> _serde::__private::Result<Self, __D::Error>
                    where
                        __D: _serde::Deserializer<'de>,
                    {
                        _serde::Deserializer::deserialize_identifier(
                            __deserializer,
                            __FieldVisitor,
                        )
                    }
                }
                #[doc(hidden)]
                struct __Visitor<'de> {
                    marker: _serde::__private::PhantomData<PingState>,
                    lifetime: _serde::__private::PhantomData<&'de ()>,
                }
                #[automatically_derived]
                impl<'de> _serde::de::Visitor<'de> for __Visitor<'de> {
                    type Value = PingState;
                    fn expecting(
                        &self,
                        __formatter: &mut _serde::__private::Formatter,
                    ) -> _serde::__private::fmt::Result {
                        _serde::__private::Formatter::write_str(
                            __formatter,
                            "struct PingState",
                        )
                    }
                    #[inline]
                    fn visit_seq<__A>(
                        self,
                        mut __seq: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::SeqAccess<'de>,
                    {
                        let __field0 = match _serde::de::SeqAccess::next_element::<
                            u32,
                        >(&mut __seq)? {
                            _serde::__private::Some(__value) => __value,
                            _serde::__private::None => {
                                return _serde::__private::Err(
                                    _serde::de::Error::invalid_length(
                                        0usize,
                                        &"struct PingState with 1 element",
                                    ),
                                );
                            }
                        };
                        _serde::__private::Ok(PingState { pong_count: __field0 })
                    }
                    #[inline]
                    fn visit_map<__A>(
                        self,
                        mut __map: __A,
                    ) -> _serde::__private::Result<Self::Value, __A::Error>
                    where
                        __A: _serde::de::MapAccess<'de>,
                    {
                        let mut __field0: _serde::__private::Option<u32> = _serde::__private::None;
                        while let _serde::__private::Some(__key) = _serde::de::MapAccess::next_key::<
                            __Field,
                        >(&mut __map)? {
                            match __key {
                                __Field::__field0 => {
                                    if _serde::__private::Option::is_some(&__field0) {
                                        return _serde::__private::Err(
                                            <__A::Error as _serde::de::Error>::duplicate_field(
                                                "pong_count",
                                            ),
                                        );
                                    }
                                    __field0 = _serde::__private::Some(
                                        _serde::de::MapAccess::next_value::<u32>(&mut __map)?,
                                    );
                                }
                                _ => {
                                    let _ = _serde::de::MapAccess::next_value::<
                                        _serde::de::IgnoredAny,
                                    >(&mut __map)?;
                                }
                            }
                        }
                        let __field0 = match __field0 {
                            _serde::__private::Some(__field0) => __field0,
                            _serde::__private::None => {
                                _serde::__private::de::missing_field("pong_count")?
                            }
                        };
                        _serde::__private::Ok(PingState { pong_count: __field0 })
                    }
                }
                #[doc(hidden)]
                const FIELDS: &'static [&'static str] = &["pong_count"];
                _serde::Deserializer::deserialize_struct(
                    __deserializer,
                    "PingState",
                    FIELDS,
                    __Visitor {
                        marker: _serde::__private::PhantomData::<PingState>,
                        lifetime: _serde::__private::PhantomData,
                    },
                )
            }
        }
    };
    impl ServiceState for PingState {
        type Settings = PingSettings;
        type Error = PingStateError;
        fn from_settings(_settings: &Self::Settings) -> Result<Self, Self::Error> {
            Ok(Self::default())
        }
    }
}
struct PingPong {
    ping: OpaqueServiceHandle<PingService>,
    pong: OpaqueServiceHandle<PongService>,
}
impl PingPong {
    const __PINGPONG__CONST_CHECK_UNIQUE_SERVICES_IDS: () = if !::overwatch::utils::const_checks::unique_ids(
        &[
            <PingService as ::overwatch::services::ServiceData>::SERVICE_ID,
            <PongService as ::overwatch::services::ServiceData>::SERVICE_ID,
        ],
    ) {
        ::core::panicking::panic(
            "assertion failed: ::overwatch::utils::const_checks::unique_ids(&[<PingService as\n                    ::overwatch::services::ServiceData>::SERVICE_ID,\n                <PongService as\n                    ::overwatch::services::ServiceData>::SERVICE_ID])",
        )
    };
}
pub struct PingPongServiceSettings {
    pub ping: <PingService as ::overwatch::services::ServiceData>::Settings,
    pub pong: <PongService as ::overwatch::services::ServiceData>::Settings,
}
#[automatically_derived]
impl ::core::clone::Clone for PingPongServiceSettings {
    #[inline]
    fn clone(&self) -> PingPongServiceSettings {
        PingPongServiceSettings {
            ping: ::core::clone::Clone::clone(&self.ping),
            pong: ::core::clone::Clone::clone(&self.pong),
        }
    }
}
#[automatically_derived]
impl ::core::fmt::Debug for PingPongServiceSettings {
    #[inline]
    fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        ::core::fmt::Formatter::debug_struct_field2_finish(
            f,
            "PingPongServiceSettings",
            "ping",
            &self.ping,
            "pong",
            &&self.pong,
        )
    }
}
impl ::overwatch::overwatch::Services for PingPong {
    type Settings = PingPongServiceSettings;
    fn new(
        settings: Self::Settings,
        overwatch_handle: ::overwatch::overwatch::handle::OverwatchHandle,
    ) -> ::std::result::Result<Self, ::overwatch::DynError> {
        let Self::Settings { ping: ping_settings, pong: pong_settings } = settings;
        let app = Self {
            ping: {
                let manager = ::overwatch::OpaqueServiceHandle::<
                    PingService,
                >::new::<
                    <PingService as ::overwatch::services::ServiceData>::StateOperator,
                >(
                    ping_settings,
                    overwatch_handle.clone(),
                    <PingService as ::overwatch::services::ServiceData>::SERVICE_RELAY_BUFFER_SIZE,
                )?;
                manager
            },
            pong: {
                let manager = ::overwatch::OpaqueServiceHandle::<
                    PongService,
                >::new::<
                    <PongService as ::overwatch::services::ServiceData>::StateOperator,
                >(
                    pong_settings,
                    overwatch_handle.clone(),
                    <PongService as ::overwatch::services::ServiceData>::SERVICE_RELAY_BUFFER_SIZE,
                )?;
                manager
            },
        };
        ::std::result::Result::Ok(app)
    }
    fn start_all(
        &mut self,
    ) -> Result<
        ::overwatch::overwatch::ServicesLifeCycleHandle,
        ::overwatch::overwatch::Error,
    > {
        ::std::result::Result::Ok(
            [
                self
                    .ping
                    .service_runner::<
                        <PingService as ::overwatch::services::ServiceData>::StateOperator,
                    >()
                    .run::<PingService>()?,
                self
                    .pong
                    .service_runner::<
                        <PongService as ::overwatch::services::ServiceData>::StateOperator,
                    >()
                    .run::<PongService>()?,
            ]
                .try_into()?,
        )
    }
    fn start(
        &mut self,
        service_id: ::overwatch::services::ServiceId,
    ) -> Result<(), ::overwatch::overwatch::Error> {
        match service_id {
            <PingService as ::overwatch::services::ServiceData>::SERVICE_ID => {
                self.ping
                    .service_runner::<
                        <PingService as ::overwatch::services::ServiceData>::StateOperator,
                    >()
                    .run::<PingService>()?;
                ::std::result::Result::Ok(())
            }
            <PongService as ::overwatch::services::ServiceData>::SERVICE_ID => {
                self.pong
                    .service_runner::<
                        <PongService as ::overwatch::services::ServiceData>::StateOperator,
                    >()
                    .run::<PongService>()?;
                ::std::result::Result::Ok(())
            }
            service_id => {
                ::std::result::Result::Err(::overwatch::overwatch::Error::Unavailable {
                    service_id,
                })
            }
        }
    }
    fn stop(
        &mut self,
        service_id: ::overwatch::services::ServiceId,
    ) -> Result<(), ::overwatch::overwatch::Error> {
        match service_id {
            <PingService as ::overwatch::services::ServiceData>::SERVICE_ID => {
                ::core::panicking::panic("not implemented")
            }
            <PongService as ::overwatch::services::ServiceData>::SERVICE_ID => {
                ::core::panicking::panic("not implemented")
            }
            service_id => {
                ::std::result::Result::Err(::overwatch::overwatch::Error::Unavailable {
                    service_id,
                })
            }
        }
    }
    fn request_relay(
        &mut self,
        service_id: ::overwatch::services::ServiceId,
    ) -> ::overwatch::services::relay::RelayResult {
        match service_id {
            <PingService as ::overwatch::services::ServiceData>::SERVICE_ID => {
                ::std::result::Result::Ok(
                    ::std::boxed::Box::new(
                        self
                            .ping
                            .relay_with()
                            .ok_or(
                                ::overwatch::services::relay::RelayError::AlreadyConnected,
                            )?,
                    ) as ::overwatch::services::relay::AnyMessage,
                )
            }
            <PongService as ::overwatch::services::ServiceData>::SERVICE_ID => {
                ::std::result::Result::Ok(
                    ::std::boxed::Box::new(
                        self
                            .pong
                            .relay_with()
                            .ok_or(
                                ::overwatch::services::relay::RelayError::AlreadyConnected,
                            )?,
                    ) as ::overwatch::services::relay::AnyMessage,
                )
            }
            service_id => {
                ::std::result::Result::Err(::overwatch::services::relay::RelayError::Unavailable {
                    service_id,
                })
            }
        }
    }
    fn request_status_watcher(
        &self,
        service_id: ::overwatch::services::ServiceId,
    ) -> ::overwatch::services::status::ServiceStatusResult {
        {}
        let __tracing_attr_span;
        let __tracing_attr_guard;
        if tracing::Level::INFO <= ::tracing::level_filters::STATIC_MAX_LEVEL
            && tracing::Level::INFO <= ::tracing::level_filters::LevelFilter::current()
            || { false }
        {
            __tracing_attr_span = {
                use ::tracing::__macro_support::Callsite as _;
                static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                    static META: ::tracing::Metadata<'static> = {
                        ::tracing_core::metadata::Metadata::new(
                            "request_status_watcher",
                            "ping_pong",
                            tracing::Level::INFO,
                            ::tracing_core::__macro_support::Option::Some(
                                "examples/ping_pong/src/main.rs",
                            ),
                            ::tracing_core::__macro_support::Option::Some(13u32),
                            ::tracing_core::__macro_support::Option::Some("ping_pong"),
                            ::tracing_core::field::FieldSet::new(
                                &["service_id"],
                                ::tracing_core::callsite::Identifier(&__CALLSITE),
                            ),
                            ::tracing::metadata::Kind::SPAN,
                        )
                    };
                    ::tracing::callsite::DefaultCallsite::new(&META)
                };
                let mut interest = ::tracing::subscriber::Interest::never();
                if tracing::Level::INFO <= ::tracing::level_filters::STATIC_MAX_LEVEL
                    && tracing::Level::INFO
                        <= ::tracing::level_filters::LevelFilter::current()
                    && {
                        interest = __CALLSITE.interest();
                        !interest.is_never()
                    }
                    && ::tracing::__macro_support::__is_enabled(
                        __CALLSITE.metadata(),
                        interest,
                    )
                {
                    let meta = __CALLSITE.metadata();
                    ::tracing::Span::new(
                        meta,
                        &{
                            #[allow(unused_imports)]
                            use ::tracing::field::{debug, display, Value};
                            let mut iter = meta.fields().iter();
                            meta.fields()
                                .value_set(
                                    &[
                                        (
                                            &::tracing::__macro_support::Iterator::next(&mut iter)
                                                .expect("FieldSet corrupted (this is a bug)"),
                                            ::tracing::__macro_support::Option::Some(
                                                &tracing::field::debug(&service_id) as &dyn Value,
                                            ),
                                        ),
                                    ],
                                )
                        },
                    )
                } else {
                    let span = ::tracing::__macro_support::__disabled_span(
                        __CALLSITE.metadata(),
                    );
                    {};
                    span
                }
            };
            __tracing_attr_guard = __tracing_attr_span.enter();
        }
        #[allow(clippy::redundant_closure_call)]
        match (move || {
            #[allow(
                unknown_lints,
                unreachable_code,
                clippy::diverging_sub_expression,
                clippy::let_unit_value,
                clippy::unreachable,
                clippy::let_with_type_underscore,
                clippy::empty_loop
            )]
            if false {
                let __tracing_attr_fake_return: ::overwatch::services::status::ServiceStatusResult = loop {};
                return __tracing_attr_fake_return;
            }
            {
                {
                    match service_id {
                        <PingService as ::overwatch::services::ServiceData>::SERVICE_ID => {
                            ::std::result::Result::Ok(self.ping.status_watcher())
                        }
                        <PongService as ::overwatch::services::ServiceData>::SERVICE_ID => {
                            ::std::result::Result::Ok(self.pong.status_watcher())
                        }
                        service_id => {
                            ::std::result::Result::Err(::overwatch::services::status::ServiceStatusError::Unavailable {
                                service_id,
                            })
                        }
                    }
                }
            }
        })() {
            #[allow(clippy::unit_arg)]
            Ok(x) => Ok(x),
            Err(e) => {
                {
                    use ::tracing::__macro_support::Callsite as _;
                    static __CALLSITE: ::tracing::callsite::DefaultCallsite = {
                        static META: ::tracing::Metadata<'static> = {
                            ::tracing_core::metadata::Metadata::new(
                                "event examples/ping_pong/src/main.rs:13",
                                "ping_pong",
                                tracing::Level::ERROR,
                                ::tracing_core::__macro_support::Option::Some(
                                    "examples/ping_pong/src/main.rs",
                                ),
                                ::tracing_core::__macro_support::Option::Some(13u32),
                                ::tracing_core::__macro_support::Option::Some("ping_pong"),
                                ::tracing_core::field::FieldSet::new(
                                    &["error"],
                                    ::tracing_core::callsite::Identifier(&__CALLSITE),
                                ),
                                ::tracing::metadata::Kind::EVENT,
                            )
                        };
                        ::tracing::callsite::DefaultCallsite::new(&META)
                    };
                    let enabled = tracing::Level::ERROR
                        <= ::tracing::level_filters::STATIC_MAX_LEVEL
                        && tracing::Level::ERROR
                            <= ::tracing::level_filters::LevelFilter::current()
                        && {
                            let interest = __CALLSITE.interest();
                            !interest.is_never()
                                && ::tracing::__macro_support::__is_enabled(
                                    __CALLSITE.metadata(),
                                    interest,
                                )
                        };
                    if enabled {
                        (|value_set: ::tracing::field::ValueSet| {
                            let meta = __CALLSITE.metadata();
                            ::tracing::Event::dispatch(meta, &value_set);
                        })({
                            #[allow(unused_imports)]
                            use ::tracing::field::{debug, display, Value};
                            let mut iter = __CALLSITE.metadata().fields().iter();
                            __CALLSITE
                                .metadata()
                                .fields()
                                .value_set(
                                    &[
                                        (
                                            &::tracing::__macro_support::Iterator::next(&mut iter)
                                                .expect("FieldSet corrupted (this is a bug)"),
                                            ::tracing::__macro_support::Option::Some(
                                                &display(&e) as &dyn Value,
                                            ),
                                        ),
                                    ],
                                )
                        });
                    } else {
                    }
                };
                Err(e)
            }
        }
    }
    fn update_settings(
        &mut self,
        settings: Self::Settings,
    ) -> Result<(), ::overwatch::overwatch::Error> {
        let Self::Settings { ping: ping_settings, pong: pong_settings } = settings;
        self.ping.update_settings(ping_settings);
        self.pong.update_settings(pong_settings);
        ::std::result::Result::Ok(())
    }
}
const PING_STATE_SAVE_PATH: &str = ::const_format::pmr::__AssertStr {
    x: {
        use ::const_format::__cf_osRcTFl4A;
        ({
            #[doc(hidden)]
            #[allow(unused_mut, non_snake_case)]
            const CONCATP_NHPMWYD3NJA: &[__cf_osRcTFl4A::pmr::PArgument] = {
                let mut len = 0usize;
                let __const_fmt_local_0 = "/Users/antonio/Developer/Logos/Overwatch/examples/ping_pong";
                &[
                    __cf_osRcTFl4A::pmr::PConvWrapper(__const_fmt_local_0)
                        .to_pargument_display(
                            __cf_osRcTFl4A::pmr::FormattingFlags::__REG,
                        ),
                    __cf_osRcTFl4A::pmr::PConvWrapper("/saved_states/ping_state.json")
                        .to_pargument_display(__cf_osRcTFl4A::pmr::FormattingFlags::NEW),
                ]
            };
            {
                #[doc(hidden)]
                const ARR_LEN: usize = ::const_format::pmr::PArgument::calc_len(
                    CONCATP_NHPMWYD3NJA,
                );
                #[doc(hidden)]
                const CONCAT_ARR: &::const_format::pmr::LenAndArray<[u8; ARR_LEN]> = &::const_format::pmr::__priv_concatenate(
                    CONCATP_NHPMWYD3NJA,
                );
                #[doc(hidden)]
                #[allow(clippy::transmute_ptr_to_ptr)]
                const CONCAT_STR: &str = unsafe {
                    let slice = ::const_format::pmr::transmute::<
                        &[u8; ARR_LEN],
                        &[u8; CONCAT_ARR.len],
                    >(&CONCAT_ARR.array);
                    {
                        let bytes: &'static [::const_format::pmr::u8] = slice;
                        let string: &'static ::const_format::pmr::str = {
                            ::const_format::__hidden_utils::PtrToRef {
                                ptr: bytes as *const [::const_format::pmr::u8] as *const str,
                            }
                                .reff
                        };
                        string
                    }
                };
                CONCAT_STR
            }
        })
    },
}
    .x;
fn main() {
    let ping_settings = PingSettings {
        state_save_path: String::from(PING_STATE_SAVE_PATH),
    };
    let ping_pong_settings = PingPongServiceSettings {
        ping: ping_settings,
        pong: (),
    };
    let ping_pong = OverwatchRunner::<PingPong>::run(ping_pong_settings, None)
        .expect("OverwatchRunner failed");
    ping_pong.wait_finished();
}
