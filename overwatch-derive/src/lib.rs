//! Procedural macros for generating service-related boilerplate in the
//! Overwatch framework.
//!
//! This crate provides macros to derive service-related traits and
//! implementations to ensure compile-time validation and structured lifecycle
//! management for services.
//!
//! # Provided Macros
//!
//! - `#[derive_services]`: Modifies a struct by changing its fields to
//!   `OpaqueServiceHandle<T>` and automatically derives the `Services` trait.
//! - `#[derive(Services)]`: Implements the `Services` trait for a struct,
//!   generating necessary service lifecycle methods and runtime service ID
//!   management. **This derive macro is not meant to be used directly**.
//!
//! # Features
//!
//! - Ensures that all services are registered at compile-time, avoiding runtime
//!   checks and panics.
//! - Provides compile-time validation for service settings and runtime service
//!   identifiers.

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use proc_macro_error2::{abort_call_site, proc_macro_error};
use quote::{format_ident, quote};
use syn::{
    parse, parse_macro_input, parse_str, punctuated::Punctuated, token::Comma, Data, DeriveInput,
    Field, Fields, GenericArgument, Generics, ItemStruct, PathArguments, Type,
};

mod utils;

/// Procedural macro to derive service-related implementations for a struct.
///
/// This macro modifies a struct by converting its fields from `T` to
/// `OpaqueServiceHandle<T>` and deriving the `Services` trait
/// to manage service lifecycle operations.
///
/// # Example
/// ```rust,ignore
/// use overwatch_derive::derive_services;
///
/// #[derive_services]
/// struct MyServices {
///     database: DatabaseService,
///     cache: CacheService,
/// }
/// ```
/// This expands to:
/// ```rust,ignore
/// use overwatch::OpaqueServiceRunnerHandle;
/// use async_trait::async_trait;
///
/// struct MyServices {
///     database: OpaqueServiceHandle<DatabaseService>,
///     cache: OpaqueServiceHandle<CacheService>,
/// }
///
/// #[async_trait]
/// impl Services for MyServices { /* service lifecycle methods */ }
/// ```
#[expect(
    clippy::missing_panics_doc,
    reason = "We will add docs to this macro later on."
)]
#[proc_macro_attribute]
pub fn derive_services(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let struct_name = &input.ident;
    let visibility = &input.vis;
    let generics = &input.generics;

    let Fields::Named(named_fields) = input.fields else {
        panic!("`derive_services` macro only supports structs with named fields");
    };
    let fields = named_fields.named;

    let modified_fields = fields.iter().map(|field| {
        let field_name = &field.ident;
        let field_type = &field.ty;
        let field_attrs = &field.attrs; // Preserve attributes (including feature flags)

        let new_field_type = quote! {
            ::overwatch::OpaqueServiceRunnerHandle<#field_type>
        };

        quote! {
            #(#field_attrs)*
            #field_name: #new_field_type
        }
    });

    // Generate the modified struct with #[derive(Services)]
    let modified_struct = quote! {
        #[derive(::overwatch::Services)]
        #visibility struct #struct_name #generics {
            #(#modified_fields),*
        }
    };

    modified_struct.into()
}

/// Returns default instrumentation settings if the `instrumentation` feature is
/// enabled.
///
/// The output of this function is to be used in places that want to add tracing
/// capabilities to non `Result` types. For `Result` types, use
/// [`get_default_instrumentation_for_result`] instead.
fn get_default_instrumentation() -> proc_macro2::TokenStream {
    #[cfg(feature = "instrumentation")]
    quote! {
        #[tracing::instrument(skip(self))]
    }

    #[cfg(not(feature = "instrumentation"))]
    quote! {}
}

/// Returns instrumentation settings that track errors if `instrumentation` is
/// enabled.
///
/// The output of this function is to be used in places that want to add tracing
/// capabilities to `Result` types. For non `Result` types, use
/// [`get_default_instrumentation`] instead.
fn get_default_instrumentation_for_result() -> proc_macro2::TokenStream {
    #[cfg(feature = "instrumentation")]
    quote! {
        #[tracing::instrument(skip(self), err)]
    }

    #[cfg(not(feature = "instrumentation"))]
    quote! {}
}

/// Returns instrumentation settings that ignore `settings` in traces.
fn get_default_instrumentation_without_settings() -> proc_macro2::TokenStream {
    #[cfg(feature = "instrumentation")]
    quote! {
        #[tracing::instrument(skip(self, settings))]
    }

    #[cfg(not(feature = "instrumentation"))]
    quote! {}
}

/// Derives the `Services` trait for a struct, implementing service lifecycle
/// operations.
///
/// This macro generates the necessary implementations to manage services,
/// including:
/// - Initializing services.
/// - Starting/stopping services.
/// - Handling relays and status updates.
///
/// **THIS MACRO IS NOT MEANT TO BE USED DIRECTLY BY DEVELOPERS, WHO SHOULD
/// RATHER USE THE `derive_services` MACRO**.
///
/// # Example
/// ```rust,ignore
/// use overwatch::OpaqueServiceHandle;
///
/// #[derive(Services)]
/// struct MyServices {
///     database: OpaqueServiceHandle<DatabaseService>,
///     cache: OpaqueServiceHandle<CacheService>,
/// }
/// ```
#[proc_macro_derive(Services)]
#[proc_macro_error]
pub fn services_derive(input: TokenStream) -> TokenStream {
    let parsed_input: DeriveInput = parse(input).expect("A syn parseable token stream");
    let derived = impl_services(&parsed_input);
    derived.into()
}

/// Creates a service settings identifier from a services identifier.
///
/// This function takes a services identifier and appends `"ServiceSettings"` to
/// create the corresponding settings type name.
///
/// # Arguments
///
/// * `services_identifier` - The identifier of the services struct
///
/// # Examples
///
/// ```rust,ignore
/// use quote::format_ident;
///
/// let service_id = format_ident!("AppServices");
/// let settings_id = service_settings_identifier_from(&service_id);
/// // settings_id will be "AppServicesServiceSettings"
/// ```
fn service_settings_identifier_from(
    services_identifier: &proc_macro2::Ident,
) -> proc_macro2::Ident {
    format_ident!("{}ServiceSettings", services_identifier)
}

/// Creates a service settings field identifier from a field identifier.
///
/// This function takes a field identifier and appends "_settings" to create
/// the corresponding settings field name.
///
/// # Arguments
///
/// * `field_identifier` - The identifier of the service field
///
/// # Examples
///
/// ```rust,ignore
/// use quote::format_ident;
///
/// let field_id = format_ident!("database");
/// let settings_field_id = service_settings_field_identifier_from(&field_id);
/// // settings_field_id will be "database_settings"
/// ```
fn service_settings_field_identifier_from(
    field_identifier: &proc_macro2::Ident,
) -> proc_macro2::Ident {
    format_ident!("{}_settings", field_identifier)
}

/// Implements the [`overwatch::overwatch::Services`] trait for the given input.
///
/// This function examines the input structure and generates the appropriate
/// implementation of the trait based on the structure's fields.
///
/// # Arguments
///
/// * `input` - The parsed derive input
///
/// # Returns
///
/// A token stream containing the Services trait implementation
///
/// # Panics
///
/// This function will abort compilation if the input is not a struct with named
/// fields.
fn impl_services(input: &DeriveInput) -> proc_macro2::TokenStream {
    use syn::DataStruct;

    let struct_identifier = &input.ident;
    let data = &input.data;
    let generics = &input.generics;
    match data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => impl_services_for_struct(struct_identifier, generics, &fields.named),
        _ => {
            abort_call_site!(
                "Deriving Services is only supported for named structs with at least one field."
            );
        }
    }
}

/// Implements the [`overwatch::overwatch::Services`] trait for a struct with
/// named fields.
///
/// This function generates all necessary code for implementing the Services
/// trait, including runtime service types, settings, and implementation
/// methods.
///
/// # Arguments
///
/// * `identifier` - The struct identifier
/// * `generics` - The struct's generic parameters
/// * `fields` - The struct's fields
///
/// # Returns
///
/// A token stream containing the combined implementations.
fn impl_services_for_struct(
    identifier: &proc_macro2::Ident,
    generics: &Generics,
    fields: &Punctuated<Field, Comma>,
) -> proc_macro2::TokenStream {
    let runtime_service_type = generate_runtime_service_types(fields);
    let settings = generate_services_settings(identifier, generics, fields);
    let services_impl = generate_services_impl(identifier, generics, fields);

    quote! {
        #runtime_service_type

        #settings

        #services_impl
    }
}

/// Generates the services settings struct for a given service.
///
/// This function creates a new struct that holds the settings for each service
/// field in the original struct. The generated struct will have the same
/// generics as the original struct.
///
/// # Arguments
///
/// * `services_identifier` - The identifier of the services struct
/// * `generics` - The generic parameters of the services struct
/// * `fields` - The fields of the services struct
///
/// # Returns
///
/// A token stream containing the settings struct definition.
fn generate_services_settings(
    services_identifier: &proc_macro2::Ident,
    generics: &Generics,
    fields: &Punctuated<Field, Comma>,
) -> proc_macro2::TokenStream {
    let services_settings = fields.iter().map(|field| {
        let service_name = field.ident.as_ref().expect("A named struct attribute");
        let _type = utils::extract_type_from(&field.ty);

        quote!(pub #service_name: <#_type as ::overwatch::services::ServiceData>::Settings)
    });
    let services_settings_identifier = service_settings_identifier_from(services_identifier);
    let where_clause = &generics.where_clause;
    quote! {
        #[derive(::core::clone::Clone, ::core::fmt::Debug)]
        pub struct #services_settings_identifier #generics #where_clause {
            #( #services_settings ),*
        }
    }
}

const RUNTIME_SERVICE_ID_TYPE_NAME: &str = "RuntimeServiceId";
fn get_runtime_service_id_type_name() -> Type {
    parse_str(RUNTIME_SERVICE_ID_TYPE_NAME)
        .expect("Runtime service ID type is a valid type token stream.")
}

/// Generates the [`overwatch::overwatch::Services`] trait implementation for a
/// struct.
///
/// This function creates the full implementation of the `Services` trait,
/// including all required methods like `new`, `start_all`, `start`, `stop`,
/// etc.
///
/// # Arguments
///
/// * `services_identifier` - The identifier of the services struct
/// * `generics` - The generic parameters of the services struct
/// * `fields` - The fields of the services struct
///
/// # Returns
///
/// A token stream containing the Services trait implementation.
fn generate_services_impl(
    services_identifier: &proc_macro2::Ident,
    generics: &Generics,
    fields: &Punctuated<Field, Comma>,
) -> proc_macro2::TokenStream {
    let services_settings_identifier = service_settings_identifier_from(services_identifier);
    let impl_new = generate_new_impl(fields);
    let impl_start = generate_start_impl(fields);
    let impl_start_sequence = generate_start_sequence_impl(fields);
    let impl_start_all = generate_start_all_impl(fields);
    let impl_stop = generate_stop_impl(fields);
    let impl_stop_sequence = generate_stop_sequence_impl(fields);
    let impl_stop_all = generate_stop_all_impl(fields);
    let impl_teardown = generate_teardown_impl(fields);
    let impl_ids = generate_ids_impl(fields);
    let impl_relay = generate_request_relay_impl(fields);
    let impl_status = generate_request_status_watcher_impl(fields);
    let impl_update_settings = generate_update_settings_impl(fields);
    let impl_get_service_lifecycle_notifier = generate_get_service_lifecycle_notifier_impl(fields);

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let runtime_service_id_type_name = get_runtime_service_id_type_name();
    quote! {
        #[::async_trait::async_trait]
        impl #impl_generics ::overwatch::overwatch::Services for #services_identifier #ty_generics #where_clause {
            type Settings = #services_settings_identifier #ty_generics;
            type RuntimeServiceId = #runtime_service_id_type_name;

            #impl_new

            #impl_start

            #impl_start_sequence

            #impl_start_all

            #impl_stop

            #impl_stop_sequence

            #impl_stop_all

            #impl_teardown

            #impl_ids

            #impl_relay

            #impl_status

            #impl_update_settings

            #impl_get_service_lifecycle_notifier
        }
    }
}

/// Generates the `new` method implementation for the `Services` trait.
///
/// This function creates the code to initialize each service field with its
/// corresponding settings and wrap it in an `OpaqueServiceHandle`.
///
/// # Arguments
///
/// * `fields` - The fields of the services struct
///
/// # Returns
///
/// A token stream containing the new method implementation.
fn generate_new_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let fields_settings = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let settings_field_identifier = service_settings_field_identifier_from(field_identifier);
        quote! {
            #field_identifier: #settings_field_identifier
        }
    });

    let managers = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let service_type = utils::extract_type_from(&field.ty);
        let settings_field_identifier = service_settings_field_identifier_from(field_identifier);
        quote! {
            #field_identifier: {
                let runner =
                    ::overwatch::OpaqueServiceRunner::<#service_type, Self::RuntimeServiceId>::new(
                        #settings_field_identifier, overwatch_handle.clone(), <#service_type as ::overwatch::services::ServiceData>::SERVICE_RELAY_BUFFER_SIZE
                );
                let service_runner_handle = runner.run::<#service_type>();
                service_runner_handle
            }
        }
    });

    quote! {
        fn new(settings: Self::Settings, overwatch_handle: ::overwatch::overwatch::handle::OverwatchHandle<Self::RuntimeServiceId>) -> ::core::result::Result<Self, ::overwatch::DynError> {
            let Self::Settings {
                #( #fields_settings ),*
            } = settings;

            let app = Self {
                #( #managers ),*
            };

            ::core::result::Result::Ok(app)
        }
    }
}

/// Generates the `start` method implementation for the `Services` trait.
///
/// This function creates code to start a specific service identified by its
/// `RuntimeServiceId`. It generates a match expression that maps each service
/// ID to the corresponding field's service runner.
///
/// # Arguments
///
/// * `fields` - The fields of the services struct
///
/// # Returns
///
/// A token stream containing the start method implementation.
fn generate_start_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let instrumentation = get_default_instrumentation_for_result();

    let cases = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        quote! {
            &<Self::RuntimeServiceId as ::overwatch::services::AsServiceId<#type_id>>::SERVICE_ID => {
                self.#field_identifier.service_handle().lifecycle_notifier().send(
                    ::overwatch::services::lifecycle::LifecycleMessage::Start(sender)
                ).await?;
            }
        }
    });

    quote! {
        #instrumentation
        async fn start(&mut self, service_id: &Self::RuntimeServiceId) -> ::core::result::Result<(), ::overwatch::overwatch::Error> {
            let (sender, mut receiver) = ::overwatch::utils::finished_signal::channel();
            match service_id {
                #( #cases ),*
            };
            receiver.await.map_err(|error| {
                let dyn_error: ::overwatch::DynError = Box::new(error);
                ::overwatch::overwatch::Error::from(dyn_error)
            })
        }
    }
}

/// Generates the `start_sequence` method implementation for the `Services`
/// trait.
///
/// This function creates code to start a list of services identified by their
/// `RuntimeServiceId`.
///
/// # Arguments
///
/// * `fields` - The fields of the services struct
///
/// # Returns
///
/// A token stream containing the `start_sequence` method implementation.
fn generate_start_sequence_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let instrumentation = get_default_instrumentation();

    let var_services_len = Ident::new("services_len", Span::call_site());
    let call_create_finished_signal_channels =
        create_finished_signal_channels_from_variable(&var_services_len);

    let var_service_ids = Ident::new("service_ids", Span::call_site());
    let var_service_id = Ident::new("service_id", Span::call_site());
    let match_cases = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        let call_send_start = send_start_lifecycle_message_over_senders(field_identifier);
        quote! {
            &<Self::RuntimeServiceId as ::overwatch::services::AsServiceId<#type_id>>::SERVICE_ID => {
                #call_send_start
            }
        }
    });
    let loop_match = quote! {
        for #var_service_id in #var_service_ids {
            match #var_service_id {
                #( #match_cases ),*
            }
        }
    };

    let call_await_finished_signal_receivers = await_finished_signal_receivers();

    quote! {
        #instrumentation
        async fn start_sequence(&mut self, service_ids: &[Self::RuntimeServiceId]) -> ::core::result::Result<(), ::overwatch::overwatch::Error> {
            let #var_services_len = service_ids.len();
            #call_create_finished_signal_channels;

            #loop_match;

            #call_await_finished_signal_receivers;

            Ok(())
        }
    }
}

/// Generates the `start_all` method implementation for the `Services` trait.
///
/// This function creates code to start all service runners and return a
/// combined lifecycle handle that can be used to manage the running services.
///
/// # Arguments
///
/// * `fields` - The fields of the services struct
///
/// # Returns
///
/// A token stream containing the `start_all` method implementation.
fn generate_start_all_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let instrumentation = get_default_instrumentation();

    let fields_len = fields.len();
    let call_create_channels = create_finished_signal_channels_from_amount(fields_len);

    let call_send_start_message = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        send_start_lifecycle_message_over_senders(field_identifier)
    });

    let call_recv_finished_signals = await_finished_signal_receivers();

    quote! {
        #instrumentation
        async fn start_all(&mut self) -> ::core::result::Result<(), ::overwatch::overwatch::Error> {
            #call_create_channels

            #( #call_send_start_message )*

            #call_recv_finished_signals

            Ok::<(), ::overwatch::overwatch::Error>(())
        }
    }
}

/// Generates the `stop` method implementation for the `Services` trait.
///
/// This function creates code to stop a specific service identified by its
/// `RuntimeServiceId`. Currently, this generates unimplemented stubs as the
/// service lifecycle is not yet fully implemented.
///
/// # Arguments
///
/// * `fields` - The fields of the services struct
///
/// # Returns
///
/// A token stream containing the stop method implementation.
fn generate_stop_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let instrumentation = get_default_instrumentation();

    let cases = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        quote! {
            &<Self::RuntimeServiceId as ::overwatch::services::AsServiceId<#type_id>>::SERVICE_ID => {
                self.#field_identifier.service_handle().lifecycle_notifier().send(
                    ::overwatch::services::lifecycle::LifecycleMessage::Stop(sender)
                ).await?;
            }
        }
    });

    quote! {
        #instrumentation
        async fn stop(&mut self, service_id: &Self::RuntimeServiceId) -> ::core::result::Result<(), ::overwatch::overwatch::Error> {
            let (sender, mut receiver) = ::overwatch::utils::finished_signal::channel();
            match service_id {
                #( #cases ),*
            };
            receiver.await.map_err(|error| {
                let dyn_error: ::overwatch::DynError = Box::new(error);
                ::overwatch::overwatch::Error::from(dyn_error)
            })
        }
    }
}

/// Generates the `stop_sequence` method implementation for the `Services`
/// trait.
///
/// This function creates code to stop a list of services identified by their
/// `RuntimeServiceId`.
///
/// # Arguments
///
/// * `fields` - The fields of the services struct
///
/// # Returns
///
/// A token stream containing the `stop_sequence` method implementation.
fn generate_stop_sequence_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let instrumentation = get_default_instrumentation();

    let var_services_len = Ident::new("services_len", Span::call_site());
    let call_create_finished_signal_channels =
        create_finished_signal_channels_from_variable(&var_services_len);

    let var_service_ids = Ident::new("service_ids", Span::call_site());
    let var_service_id = Ident::new("service_id", Span::call_site());
    let match_cases = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        let call_send_stop = send_stop_lifecycle_message_over_senders(field_identifier);
        quote! {
            &<Self::RuntimeServiceId as ::overwatch::services::AsServiceId<#type_id>>::SERVICE_ID => {
                #call_send_stop
            }
        }
    });
    let loop_match = quote! {
        for #var_service_id in #var_service_ids {
            match #var_service_id {
                #( #match_cases ),*
            }
        }
    };

    let call_await_finished_signal_receivers = await_finished_signal_receivers();

    quote! {
        #instrumentation
        async fn stop_sequence(&mut self, service_ids: &[Self::RuntimeServiceId]) -> ::core::result::Result<(), ::overwatch::overwatch::Error> {
            let #var_services_len = service_ids.len();
            #call_create_finished_signal_channels;

            #loop_match;

            #call_await_finished_signal_receivers;

            Ok(())
        }
    }
}

/// Generates the `stop_all` method implementation for the `Services` trait.
///
/// This function creates code to stop all service runners.
///
/// # Arguments
///
/// * `fields` - The fields of the services struct
///
/// # Returns
///
/// A token stream containing the `stop_all` method implementation.
fn generate_stop_all_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let instrumentation = get_default_instrumentation();

    let fields_len = fields.len();
    let call_create_channels = create_finished_signal_channels_from_amount(fields_len);

    let call_send_stop_message_to_services = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        send_stop_lifecycle_message_over_senders(field_identifier)
    });

    let call_recv_finished_signals = await_finished_signal_receivers();

    quote! {
        #instrumentation
        async fn stop_all(&mut self) -> Result<(), ::overwatch::overwatch::Error> {
            #call_create_channels

            #( #call_send_stop_message_to_services )*

            #call_recv_finished_signals

            Ok::<(), ::overwatch::overwatch::Error>(())
        }
    }
}

/// Generates the `teardown` method implementation for the `Services` trait.
///
/// This function creates code to teardown the `Services` struct.
///
/// # Arguments
///
/// * `fields` - The fields of the services struct
///
/// # Returns
///
/// A token stream containing the `teardown` method implementation.
fn generate_teardown_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let instrumentation = get_default_instrumentation();

    let call_abort_service_runner_join_handles = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        quote! {
            self.#field_identifier.runner_join_handle().abort();
        }
    });

    let call_await_service_runner_join_handles = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        quote! {
            if let Err(error) = self.#field_identifier.runner_join_handle_owned().await {
                ::tracing::error!("Error while awaiting ServiceRunner's JoinHandle: {error}");
            }
        }
    });

    quote! {
        #instrumentation
        async fn teardown(self) -> Result<(), ::overwatch::overwatch::Error> {
            # (#call_abort_service_runner_join_handles)*

            # (#call_await_service_runner_join_handles)*

            Ok::<(), ::overwatch::overwatch::Error>(())
        }
    }
}

/// Generates the `ids` method implementation for the `Services` trait.
///
/// This function creates code to retrieve the `RuntimeServiceId` for each
/// service defined in the struct. It returns a [`Vec`] of `RuntimeServiceId`s.
///
/// # Arguments
///
/// * `fields` - The fields of the services struct
///
/// # Returns
///
/// A token stream containing the `ids` method implementation.
fn generate_ids_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let instrumentation = get_default_instrumentation();

    let service_ids = fields.iter().map(|field| {
        let type_id = utils::extract_type_from(&field.ty);
        quote! {
            <Self::RuntimeServiceId as ::overwatch::services::AsServiceId<#type_id>>::SERVICE_ID
        }
    });

    quote! {
        #instrumentation
        fn ids(&self) -> Vec<Self::RuntimeServiceId> {
            vec![ #( #service_ids ),* ]
        }
    }
}

/// Generates the `request_relay` method implementation for the `Services`
/// trait.
///
/// This function creates code to request a message relay for a specific service
/// identified by its `RuntimeServiceId`.
///
/// # Arguments
///
/// * `fields` - The fields of the services struct
///
/// # Returns
///
/// A token stream containing the `request_relay` method implementation.
fn generate_request_relay_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let instrumentation = get_default_instrumentation();

    let cases = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        quote! {
            &<Self::RuntimeServiceId as ::overwatch::services::AsServiceId<#type_id>>::SERVICE_ID => {
                ::std::boxed::Box::new(self.#field_identifier.service_handle().relay_with())
            }
        }
    });

    quote! {
        #instrumentation
        fn request_relay(&mut self, service_id: &Self::RuntimeServiceId) -> ::overwatch::services::relay::AnyMessage {
            match service_id {
                #( #cases )*
            }
        }
    }
}

/// Generates the `request_status_watcher` method implementation for the
/// `Services` trait.
///
/// This function creates code to request a status watcher for a specific
/// service identified by its `RuntimeServiceId`. The status watcher can be used
/// to monitor the service's status changes.
///
/// # Arguments
///
/// * `fields` - The fields of the services struct
///
/// # Returns
///
/// A token stream containing the `request_status_watcher` method
/// implementation.
fn generate_request_status_watcher_impl(
    fields: &Punctuated<Field, Comma>,
) -> proc_macro2::TokenStream {
    let instrumentation = get_default_instrumentation();

    let cases = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        quote! {
            &<Self::RuntimeServiceId as ::overwatch::services::AsServiceId<#type_id>>::SERVICE_ID => {
                self.#field_identifier.service_handle().status_watcher().clone()
            }
        }
    });

    quote! {
        #instrumentation
        fn request_status_watcher(&self, service_id: &Self::RuntimeServiceId) -> ::overwatch::services::status::StatusWatcher {
            match service_id {
                #( #cases )*
            }
        }
    }
}

/// Generates the `update_settings` method implementation for the `Services`
/// trait.
///
/// This function creates code to update the settings for all services. It
/// destructures the settings struct and passes each field's settings to the
/// corresponding service handle.
///
/// # Arguments
///
/// * `fields` - The fields of the services struct
///
/// # Returns
///
/// A token stream containing the `update_settings` method implementation.
fn generate_update_settings_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let instrumentation = get_default_instrumentation_without_settings();

    let fields_settings = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let settings_field_identifier = service_settings_field_identifier_from(field_identifier);
        quote! {
            #field_identifier: #settings_field_identifier
        }
    });

    let update_settings_call = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let settings_field_identifier = service_settings_field_identifier_from(field_identifier);
        quote! {
            self.#field_identifier.service_handle().update_settings(#settings_field_identifier);
        }
    });

    quote! {
        #instrumentation
        fn update_settings(&mut self, settings: Self::Settings) {
            let Self::Settings {
                #( #fields_settings ),*
            } = settings;

            #( #update_settings_call )*
        }
    }
}

/// Generates the `get_service_lifecycle_notifier` method implementation for the
/// `Services` trait.
///
/// This function creates code to retrieve the lifecycle handle for a specific
/// service identified by its `RuntimeServiceId`. The lifecycle handle can be
/// used to manage the service's lifecycle events.
///
/// # Arguments
///
/// * `fields` - The fields of the services struct
///
/// # Returns
///
/// A token stream containing the `get_service_lifecycle_notifier` method
/// implementation.
fn generate_get_service_lifecycle_notifier_impl(
    fields: &Punctuated<Field, Comma>,
) -> proc_macro2::TokenStream {
    let instrumentation = get_default_instrumentation();

    let cases = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        quote! {
            &<Self::RuntimeServiceId as ::overwatch::services::AsServiceId<#type_id>>::SERVICE_ID => {
                self.#field_identifier.service_handle().lifecycle_notifier()
            }
        }
    });

    quote! {
        #instrumentation
        fn get_service_lifecycle_notifier(&self, service_id: &Self::RuntimeServiceId) -> &::overwatch::services::lifecycle::LifecycleNotifier {
            match service_id {
                #( #cases ),*
            }
        }
    }
}

/// Generates the runtime service type definitions.
///
/// This function creates the `RuntimeServiceId` enum, service ID trait
/// implementations, and `AsServiceId` trait implementations for each service
/// type that is part of the specified runtime.
///
/// # Arguments
///
/// * `fields` - The fields of the services struct, indicating the different
///   services that are part of the runtime.
///
/// # Returns
///
/// A token stream containing all runtime service type definitions.
fn generate_runtime_service_types(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let runtime_service_id = generate_runtime_service_id(fields);
    let service_id_trait_impls = generate_service_id_trait_impls(fields);
    let as_service_id_impl = generate_as_service_id_impl(fields);

    quote! {
        #runtime_service_id

        #service_id_trait_impls

        #as_service_id_impl
    }
}

/// Generates a runtime service ID enum from the fields of a service container
/// struct.
///
/// This function creates an enum named `RuntimeServiceId` where each variant
/// corresponds to a service defined in the service container struct. The enum
/// is automatically derived with useful traits including `Debug`, `Clone`,
/// `Copy`, `PartialEq` and `Eq`.
///
/// The service names from the struct fields are converted to `PascalCase` for
/// the enum variants.
///
/// # Arguments
///
/// * `fields` - A punctuated list of fields from the service container struct
///
/// # Returns
///
/// A `TokenStream` containing the definition of the `RuntimeServiceId` enum
///
/// # Example
///
/// For a service container struct like:
///
/// ```rust,ignore
/// struct MyServices {
///     database: OpaqueServiceHandle<DatabaseService>,
///     api_gateway: OpaqueServiceHandle<ApiGatewayService>,
///     user_cache: OpaqueServiceHandle<CacheService<User>>,
/// }
/// ```
///
/// This function will generate:
///
/// ```rust
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// pub enum RuntimeServiceId {
///     Database,
///     ApiGateway,
///     UserCache,
/// }
/// ```
///
/// The generated enum serves as a unique identifier for each service in the
/// application, enabling service lookup, lifecycle management, and message
/// routing throughout the Overwatch framework.
fn generate_runtime_service_id(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let services_names = fields
        .iter()
        .clone()
        .map(|field| (&field.ident, &field.attrs));
    let enum_variants = services_names.map(|(service_name, service_attrs)| {
        let capitalized_service_name = format_ident!(
            "{}",
            utils::field_name_to_type_name(
                &service_name
                    .clone()
                    .expect("Expected struct named fields.")
                    .to_string()
            )
        );

        quote! { #(#service_attrs),* #capitalized_service_name }
    });
    let runtime_service_id_type_name = get_runtime_service_id_type_name();
    let expanded = quote! {
        #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::marker::Copy, ::core::cmp::PartialEq, ::core::cmp::Eq)]
        pub enum #runtime_service_id_type_name {
            #(#enum_variants),*
        }
    };

    quote! {
        #expanded
    }
}

/// Generates different trait implementations, e.g. `Display`, for
/// `RuntimeServiceId`.
///
/// # Returns
///
/// A token stream containing the Display trait implementation
fn generate_service_id_trait_impls(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let runtime_service_id_type_name = get_runtime_service_id_type_name();

    let runtime_service_id_from_str_impl = generate_runtime_service_id_from_str_impl(fields);

    quote! {
        impl ::core::fmt::Display for #runtime_service_id_type_name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                <Self as ::core::fmt::Debug>::fmt(self, f)
            }
        }

        #runtime_service_id_from_str_impl
    }
}

/// Generates the `RuntimeServiceId` enum from a string representation.
///
/// # Returns
///
/// A token stream containing the implementation of the `From<Into<String>>`
/// trait
fn generate_runtime_service_id_from_str_impl(
    fields: &Punctuated<Field, Comma>,
) -> proc_macro2::TokenStream {
    let runtime_service_id_type_name = get_runtime_service_id_type_name();

    let available_services = fields
        .iter()
        .map(|field| {
            let field_identifier = field
                .ident
                .as_ref()
                .expect("Expected struct named fields.")
                .to_string();
            utils::field_name_to_type_name(&field_identifier)
        })
        .collect::<Vec<_>>()
        .join(", ");

    let string_to_variant_pairs = fields.iter().map(|field| {
        let field_ident = field.ident.as_ref().expect("Expected struct named fields.");
        let type_name_capitalized = utils::field_name_to_type_name(&field_ident.to_string());
        let type_identifier_capitalized = format_ident!("{}", type_name_capitalized);
        let runtime_service_id_variant =
            quote! { #runtime_service_id_type_name::#type_identifier_capitalized };
        (type_name_capitalized, runtime_service_id_variant)
    });

    let arms = string_to_variant_pairs.map(|(name, variant)| {
        quote! {
            #name => { Ok(#variant) }
        }
    });

    quote! {
        impl ::std::str::FromStr for #runtime_service_id_type_name {
            type Err = ::overwatch::overwatch::Error;

            fn from_str(value: &str) -> ::core::result::Result<Self, Self::Err> {
                match value.as_ref() {
                    #( #arms ),*
                    _ => {
                        let error_string = format!(
                            "Couldn't find a service with the name: {value}. Available services are: {}.",
                            #available_services
                        );
                        let error = ::overwatch::overwatch::Error::Any(::overwatch::DynError::from(error_string));
                        Err(error)
                    }
                }
            }
        }
    }
}

/// Generates implementations of the `AsServiceId` trait for service types.
///
/// This function creates trait implementations that map service types to their
/// corresponding service ID enum variants. It examines the fields of a service
/// container struct and automatically generates the necessary trait
/// implementations to connect each service with its identifier in the runtime
/// service ID enum.
///
/// This is an internal function used by the `derive_services` macro to generate
/// the necessary trait implementations for service identification.
///
/// # Arguments
///
/// * `fields` - A punctuated list of fields from the service container struct
///
/// # Returns
///
/// A `TokenStream` containing all the `AsServiceId` trait implementations for
/// the service types
///
/// # Example
///
/// Assuming we have the following service container struct:
///
/// ```rust,ignore
/// use overwatch::OpaqueServiceHandle;
///
/// struct MyServices {
///     database: OpaqueServiceHandle<DatabaseService>,
///     api: OpaqueServiceHandle<ApiService>,
/// }
/// ```
///
/// The function will generate code similar to:
///
/// ```rust,ignore
/// use overwatch::services::AsServiceId;
///
/// impl AsServiceId<DatabaseService> for RuntimeServiceId {
///     const SERVICE_ID: Self = RuntimeServiceId::Database;
/// }
///
/// impl AsServiceId<ApiService> for RuntimeServiceId {
///     const SERVICE_ID: Self = RuntimeServiceId::Api;
/// }
/// ```
///
/// For services with generic parameters:
///
/// ```rust,ignore
/// use overwatch::OpaqueServiceHandle;
///
/// struct MyServices {
///     cache: OpaqueServiceHandle<CacheService<String, u64>>,
/// }
/// ```
///
/// It will generate:
///
/// ```rust,ignore
/// use overwatch::services::AsServiceId;
///
/// impl AsServiceId<CacheService<String, u64>> for RuntimeServiceId {
///     const SERVICE_ID: Self = RuntimeServiceId::Cache;
/// }
/// ```
///
/// This enables the runtime system to map between service types and their
/// corresponding identifiers, which is essential for service lifecycle
/// management and message routing.
fn generate_as_service_id_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let impl_blocks = fields.iter().filter_map(|field| {
        let field_type = &field.ty;
        let field_attrs = &field.attrs;
        let capitalized_service_name = format_ident!(
            "{}",
            utils::field_name_to_type_name(
                &field
                    .ident
                    .clone()
                    .expect("Expected struct named fields.")
                    .to_string()
            )
        );

        let Type::Path(path) = &field_type else {
            return None;
        };
        let path_segment = path.path.segments.last()?;

        // Extract the inner type inside OpaqueServiceHandle<T>
        let PathArguments::AngleBracketed(args) = &path_segment.arguments else {
            return None;
        };

        let Some(GenericArgument::Type(inner_type)) = &args.args.first() else {
            return None;
        };

        let Type::Path(inner_path) = inner_type else {
            return None;
        };

        let inner_ident = &inner_path.path.segments.last().expect("Expected at least one segment in the inner type path").ident;
        let runtime_service_id_type_name = get_runtime_service_id_type_name();

        inner_path.path.segments.last().map_or_else(
            || None,
            |segment| match &segment.arguments {
                PathArguments::AngleBracketed(generic_args) => {
                    let struct_generics: Vec<_> = generic_args.args.iter()
                        .filter_map(|arg| match arg {
                            GenericArgument::Type(Type::Path(type_path)) => Some(type_path.clone()),
                            _ => None,
                        })
                        .collect();

                    Some(quote! {
                        #(#field_attrs),*
                        impl ::overwatch::services::AsServiceId<#inner_ident<#(#struct_generics),*>> for #runtime_service_id_type_name {
                            const SERVICE_ID: Self = #runtime_service_id_type_name::#capitalized_service_name;
                        }
                    })
                },
                // No generics case
                _ => Some(quote! {
                    #(#field_attrs),*
                    impl ::overwatch::services::AsServiceId<#inner_ident> for #runtime_service_id_type_name {
                        const SERVICE_ID: Self = #runtime_service_id_type_name::#capitalized_service_name;
                    }
                }),
            }
        )
    });

    quote! {
        #(#impl_blocks)*
    }
}

fn create_finished_signal_channels_from_amount(amount: usize) -> proc_macro2::TokenStream {
    quote! {
        let channels = (0..#amount).map(|_| { ::overwatch::utils::finished_signal::channel() });
        let (mut senders, receivers): (Vec<_>, Vec<_>) = channels.into_iter().unzip();
    }
}

fn create_finished_signal_channels_from_variable(variable: &Ident) -> proc_macro2::TokenStream {
    quote! {
        let channels = (0..#variable).map(|_| { ::overwatch::utils::finished_signal::channel() });
        let (mut senders, receivers): (Vec<_>, Vec<_>) = channels.into_iter().unzip();
    }
}

fn await_finished_signal_receivers() -> proc_macro2::TokenStream {
    quote! {
        for mut receiver in receivers {
            receiver.await.map_err(|error| {
                let dyn_error: ::overwatch::DynError = Box::new(error);
                ::overwatch::overwatch::Error::from(dyn_error)
            })?;
        }
    }
}

fn send_lifecycle_message_over_senders(
    field: &Ident,
    lifecycle_variant: &str,
) -> proc_macro2::TokenStream {
    let lifecycle_variant = format_ident!("{}", lifecycle_variant);
    quote! {
        self.#field.service_handle().lifecycle_notifier().send(
            ::overwatch::services::lifecycle::LifecycleMessage::#lifecycle_variant(senders.remove(0))
        ).await?;
    }
}

fn send_start_lifecycle_message_over_senders(field: &Ident) -> proc_macro2::TokenStream {
    send_lifecycle_message_over_senders(field, "Start")
}

fn send_stop_lifecycle_message_over_senders(field: &Ident) -> proc_macro2::TokenStream {
    send_lifecycle_message_over_senders(field, "Stop")
}
