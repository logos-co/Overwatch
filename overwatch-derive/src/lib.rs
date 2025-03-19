use proc_macro::TokenStream;
use proc_macro_error2::{abort_call_site, proc_macro_error};
use quote::{format_ident, quote};
use syn::{
    parse, parse_macro_input, parse_str, punctuated::Punctuated, token::Comma, Data, DeriveInput,
    Field, Fields, GenericArgument, Generics, Ident, ItemStruct, PathArguments, Type,
};

mod utils;

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

        let runtime_service_id_type_name = get_runtime_service_id_type_name();
        let new_field_type = quote! {
            ::overwatch::OpaqueServiceHandle<#field_type, #runtime_service_id_type_name>
        };

        quote! {
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

fn get_default_instrumentation() -> proc_macro2::TokenStream {
    #[cfg(feature = "instrumentation")]
    quote! {
        #[tracing::instrument(skip(self))]
    }

    #[cfg(not(feature = "instrumentation"))]
    quote! {}
}

fn get_default_instrumentation_for_result() -> proc_macro2::TokenStream {
    #[cfg(feature = "instrumentation")]
    quote! {
        #[tracing::instrument(skip(self), err)]
    }

    #[cfg(not(feature = "instrumentation"))]
    quote! {}
}

fn get_default_instrumentation_without_settings() -> proc_macro2::TokenStream {
    #[cfg(feature = "instrumentation")]
    quote! {
        #[tracing::instrument(skip(self, settings))]
    }

    #[cfg(not(feature = "instrumentation"))]
    quote! {}
}

#[proc_macro_derive(Services)]
#[proc_macro_error]
pub fn services_derive(input: TokenStream) -> TokenStream {
    let parsed_input: DeriveInput = parse(input).expect("A syn parseable token stream");
    let derived = impl_services(&parsed_input);
    derived.into()
}

fn service_settings_identifier_from(
    services_identifier: &proc_macro2::Ident,
) -> proc_macro2::Ident {
    format_ident!("{}ServiceSettings", services_identifier)
}

fn service_settings_field_identifier_from(
    field_identifier: &proc_macro2::Ident,
) -> proc_macro2::Ident {
    format_ident!("{}_settings", field_identifier)
}

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

fn generate_services_impl(
    services_identifier: &proc_macro2::Ident,
    generics: &Generics,
    fields: &Punctuated<Field, Comma>,
) -> proc_macro2::TokenStream {
    let services_settings_identifier = service_settings_identifier_from(services_identifier);
    let impl_new = generate_new_impl(fields);
    let impl_start_all = generate_start_all_impl(fields);
    let impl_start = generate_start_impl(fields);
    let impl_stop = generate_stop_impl(fields);
    let impl_relay = generate_request_relay_impl(fields);
    let impl_status = generate_request_status_watcher_impl(fields);
    let impl_update_settings = generate_update_settings_impl(fields);

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let runtime_service_id_type_name = get_runtime_service_id_type_name();
    quote! {
        impl #impl_generics ::overwatch::overwatch::Services for #services_identifier #ty_generics #where_clause {
            type Settings = #services_settings_identifier #ty_generics;
            type RuntimeServiceId = #runtime_service_id_type_name;
            type ServicesLifeCycleHandle = RuntimeLifeCycleHandlers;

            #impl_new

            #impl_start_all

            #impl_start

            #impl_stop

            #impl_relay

            #impl_status

            #impl_update_settings
        }
    }
}

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
                let manager =
                    ::overwatch::OpaqueServiceHandle::<#service_type, Self::RuntimeServiceId>::new::<<#service_type as ::overwatch::services::ServiceData>::StateOperator>(
                        #settings_field_identifier, overwatch_handle.clone(), <#service_type as ::overwatch::services::ServiceData>::SERVICE_RELAY_BUFFER_SIZE
                )?;
                manager
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

fn generate_start_all_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let call_start = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        quote! {
            #field_identifier: self.#field_identifier.service_runner::<<#type_id as ::overwatch::services::ServiceData>::StateOperator>().run::<#type_id>()?
        }
    });

    let instrumentation = get_default_instrumentation_for_result();
    quote! {
        #instrumentation
        fn start_all(&mut self) -> ::core::result::Result<Self::RuntimeLifeCycleHandlers, ::overwatch::overwatch::Error> {
            ::core::result::Result::Ok(Self::RuntimeLifeCycleHandlers {
                #( #call_start ),*
            })
        }
    }
}

fn generate_start_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let cases = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        let runtime_service_id_type_name = get_runtime_service_id_type_name();
        quote! {
            &<#type_id as ::overwatch::services::ServiceId<#runtime_service_id_type_name>>::SERVICE_ID => {
                self.#field_identifier.service_runner::<<#type_id as ::overwatch::services::ServiceData>::StateOperator>().run::<#type_id>()?;
                ::core::result::Result::Ok(())
            }
        }
    });

    let instrumentation = get_default_instrumentation_for_result();
    quote! {
        #instrumentation
        fn start(&mut self, service_id: &Self::RuntimeServiceId) -> ::core::result::Result<(), ::overwatch::overwatch::Error> {
            match service_id {
                #( #cases ),*
            }
        }
    }
}

fn generate_stop_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let cases = fields.iter().map(|field| {
        let _field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        let runtime_service_id_type_name = get_runtime_service_id_type_name();
        // TODO: actually stop them here once service lifecycle is implemented
        quote! {
            &<#type_id as ::overwatch::services::ServiceId<#runtime_service_id_type_name>>::SERVICE_ID => { unimplemented!() }
        }
    });

    let instrumentation = get_default_instrumentation();
    quote! {
        #instrumentation
        fn stop(&mut self, service_id: &Self::RuntimeServiceId) {
            match service_id {
                #( #cases ),*
            }
        }
    }
}

fn generate_request_relay_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let cases = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        let runtime_service_id_type_name = get_runtime_service_id_type_name();
        quote! {
            &<#type_id as ::overwatch::services::ServiceId<#runtime_service_id_type_name>>::SERVICE_ID => {
                ::core::result::Result::Ok(::std::boxed::Box::new(
                    self.#field_identifier
                        .relay_with()
                        .ok_or(::overwatch::services::relay::RelayError::AlreadyConnected)?
                ) as ::overwatch::services::relay::AnyMessage)
            }
        }
    });

    let instrumentation = get_default_instrumentation_for_result();
    quote! {
        #instrumentation
        fn request_relay(&mut self, service_id: &Self::RuntimeServiceId) -> ::overwatch::services::relay::RelayResult {
            match service_id {
                #( #cases )*
            }
        }
    }
}

fn generate_request_status_watcher_impl(
    fields: &Punctuated<Field, Comma>,
) -> proc_macro2::TokenStream {
    let cases = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        let runtime_service_id_type_name = get_runtime_service_id_type_name();
        quote! {
            &<#type_id as ::overwatch::services::ServiceId<#runtime_service_id_type_name>>::SERVICE_ID => {
                self.#field_identifier.status_watcher()
            }
        }
    });

    let instrumentation = get_default_instrumentation();
    quote! {
        #instrumentation
        fn request_status_watcher(&self, service_id: &Self::RuntimeServiceId) -> ::overwatch::services::status::StatusWatcher {
            match service_id {
                #( #cases )*
            }
        }
    }
}

fn generate_update_settings_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
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
            self.#field_identifier.update_settings(#settings_field_identifier);
        }
    });

    let instrumentation = get_default_instrumentation_without_settings();
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

fn generate_runtime_service_types(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let runtime_service_id = generate_runtime_service_id(fields);
    let service_id_trait_impls = generate_service_id_trait_impls();
    let service_id_impls = generate_service_id_impls(fields);

    quote! {
        #runtime_service_id

        #service_id_trait_impls

        #service_id_impls
    }
}

fn generate_runtime_service_id(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let services_names = fields.iter().clone().map(|field| &field.ident);
    let enum_variants = services_names.map(|service_name| {
        let capitalized_service_name = format_ident!(
            "{}",
            utils::field_name_to_type_name(
                &service_name
                    .clone()
                    .expect("Expected struct named fields.")
                    .to_string()
            )
        );

        quote! { #capitalized_service_name }
    });
    let runtime_service_id_type_name = get_runtime_service_id_type_name();
    let expanded = quote! {
        #[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::marker::Copy, ::core::cmp::PartialEq, ::core::cmp::Eq, ::core::hash::Hash, ::overwatch::LifecycleHandlers)]
        pub enum #runtime_service_id_type_name {
            #(#enum_variants),*
        }
    };

    quote! {
        #expanded
    }
}

fn generate_service_id_trait_impls() -> proc_macro2::TokenStream {
    let runtime_service_id_type_name = get_runtime_service_id_type_name();
    quote! {
        impl ::core::fmt::Display for #runtime_service_id_type_name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                <Self as ::core::fmt::Debug>::fmt(self, f)
            }
        }
    }
}

fn generate_service_id_impls(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let impl_blocks: Vec<_> = fields.iter().filter_map(|field| {
        let field_type = &field.ty;
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

        // Extract generics if present, but rename them to T1, T2, etc.
        match inner_path
            .path
            .segments
            .last()
            .map(|segment| segment.arguments.clone()) {
                Some(PathArguments::AngleBracketed(generic_args)) => {
                    let generic_count = generic_args.args.len();
                    let generic_params: Vec<_> = (1..=generic_count)
                        .map(|i| format_ident!("T{}", i))
                        .collect();

                    Some(quote! {
                        impl<#(#generic_params),*> ::overwatch::utils::traits::ServiceId<#runtime_service_id_type_name> for #inner_ident<#(#generic_params),*> {
                            const SERVICE_ID: #runtime_service_id_type_name = #runtime_service_id_type_name::#capitalized_service_name;
                        }
                    })
                },
                // No generics case
                _ => Some(quote! {
                    impl ::overwatch::services::ServiceId<#runtime_service_id_type_name> for #inner_ident {
                        const SERVICE_ID: #runtime_service_id_type_name = #runtime_service_id_type_name::#capitalized_service_name;
                    }
                }),
        }
    }).collect();

    quote! {
        #(#impl_blocks)*
    }
}

/// Docs WIP.
///
/// # Panics
///
/// If the derive macro is not used on the created service ID enum.
#[proc_macro_derive(LifecycleHandlers)]
pub fn generate_lifecyle_handlers(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let enum_name = input.ident;

    let Data::Enum(data_enum) = input.data else {
        panic!("`LifecycleHandlers` can only be used on enums");
    };

    let fields: Vec<Ident> = data_enum.variants.iter().map(|v| v.ident.clone()).collect();
    let struct_fields = fields.iter().map(|name| {
        let field_name = Ident::new(&name.to_string().to_lowercase(), name.span());
        quote! { #field_name: ::overwatch::services::life_cycle::LifecycleHandle }
    });

    let match_arms_shutdown = fields.iter().map(|name| {
        let field_name = Ident::new(&name.to_string().to_lowercase(), name.span());
        quote! { &#enum_name::#name => self.#field_name.send(::overwatch::services::life_cycle::LifecycleMessage::Shutdown(sender)) }
    });

    let match_arms_kill = fields.iter().map(|name| {
        let field_name = Ident::new(&name.to_string().to_lowercase(), name.span());
        quote! { &#enum_name::#name => self.#field_name.send(::overwatch::services::life_cycle::LifecycleMessage::Kill) }
    });

    let kill_all_body = fields.iter().map(|name| {
        let field_name = Ident::new(&name.to_string().to_lowercase(), name.span());
        quote! { self.#field_name.send(::overwatch::services::life_cycle::LifecycleMessage::Kill)?; }
    });

    let expanded = quote! {
        pub struct RuntimeLifeCycleHandlers {
            #(#struct_fields,)*
        }

        impl RuntimeLifeCycleHandlers {
            /// Send a `Shutdown` message to the specified service.
            ///
            /// # Arguments
            ///
            /// `service` - The [`ServiceId`] of the target service
            /// `sender` - The sender side of a broadcast channel. It's expected that
            /// once the receiver finishes processing the message, a signal will be
            /// sent back.
            ///
            /// # Errors
            ///
            /// The error returned when trying to send the shutdown command to the
            /// specified service.
            pub fn shutdown(
                &self,
                service: &#enum_name,
                sender: ::tokio::sync::broadcast::Sender<::overwatch::services::life_cycle::FinishedSignal>,
            ) -> Result<(), ::overwatch::DynError> {
                match service {
                    #(#match_arms_shutdown,)*
                }
            }

            /// Send a [`LifecycleMessage::Kill`] message to the specified service
            /// ([`ServiceId`]) [`crate::overwatch::OverwatchRunner`].
            /// # Arguments
            ///
            /// `service` - The [`ServiceId`] of the target service
            ///
            /// # Errors
            ///
            /// The error returned when trying to send the kill command to the specified
            /// service.
            pub fn kill(&self, service: &#enum_name) -> Result<(), ::overwatch::DynError> {
                match service {
                    #(#match_arms_kill,)*
                }
            }

            /// Send a [`LifecycleMessage::Kill`] message to all services registered in
            /// this handle.
            ///
            /// # Errors
            ///
            /// The error returned when trying to send the kill command to any of the
            /// running services.
            pub fn kill_all(&self) -> Result<(), ::overwatch::DynError> {
                #(#kill_all_body)*
                Ok(())
            }
        }
    };

    TokenStream::from(expanded)
}
