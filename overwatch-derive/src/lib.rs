mod utils;

use proc_macro_error2::{abort_call_site, proc_macro_error};
use quote::{format_ident, quote};
use syn::{
    punctuated::Punctuated, token::Comma, Data, DeriveInput, Field, GenericArgument, Generics,
    PathArguments, Type,
};

fn get_default_instrumentation() -> proc_macro2::TokenStream {
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
        #[tracing::instrument(skip(self, settings), err)]
    }

    #[cfg(not(feature = "instrumentation"))]
    quote! {}
}

#[proc_macro_derive(Services)]
#[proc_macro_error]
pub fn derive_services(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: DeriveInput = syn::parse(input).expect("A syn parseable token stream");
    let derived = impl_services(&input);
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
            fields: syn::Fields::Named(fields),
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
    let unique_ids_check = generate_assert_unique_identifiers(identifier, generics, fields);
    let aggregated_service_type = generate_aggregate_service_types(fields);
    let settings = generate_services_settings(identifier, generics, fields);
    let services_impl = generate_services_impl(identifier, generics, fields);

    quote! {
        #unique_ids_check

        #aggregated_service_type

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
        #[derive(::std::clone::Clone, ::std::fmt::Debug)]
        pub struct #services_settings_identifier #generics #where_clause {
            #( #services_settings ),*
        }
    }
}

fn generate_assert_unique_identifiers(
    services_identifier: &proc_macro2::Ident,
    generics: &Generics,
    fields: &Punctuated<Field, Comma>,
) -> proc_macro2::TokenStream {
    let services_ids = fields.iter().map(|field| {
        let _type = utils::extract_type_from(&field.ty);
        quote! {
            <#_type as ::overwatch::services::ServiceData>::SERVICE_ID
        }
    });
    let services_ids_check = format_ident!(
        "__{}__CONST_CHECK_UNIQUE_SERVICES_IDS",
        services_identifier.to_string().to_uppercase()
    );
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics #services_identifier #ty_generics #where_clause {
            const #services_ids_check: () = assert!(::overwatch::utils::const_checks::unique_ids(&[#( #services_ids ),*]));
        }
    }
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

    quote! {
        impl #impl_generics ::overwatch::overwatch::Services for #services_identifier #ty_generics #where_clause {
            type Settings = #services_settings_identifier #ty_generics;
            type AggregatedServiceId = AggregatedServiceId;

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
                    ::overwatch::OpaqueServiceHandle::<#service_type>::new::<<#service_type as ::overwatch::services::ServiceData>::StateOperator>(
                        #settings_field_identifier, overwatch_handle.clone(), <#service_type as ::overwatch::services::ServiceData>::SERVICE_RELAY_BUFFER_SIZE
                )?;
                manager
            }
        }
    });

    quote! {
        fn new(settings: Self::Settings, overwatch_handle: ::overwatch::overwatch::handle::OverwatchHandle) -> ::std::result::Result<Self, ::overwatch::DynError> {
            let Self::Settings {
                #( #fields_settings ),*
            } = settings;

            let app = Self {
                #( #managers ),*
            };

            ::std::result::Result::Ok(app)
        }
    }
}

fn generate_start_all_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let call_start = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        quote! {
            self.#field_identifier.service_runner::<<#type_id as ::overwatch::services::ServiceData>::StateOperator>().run::<#type_id>()?
        }
    });

    let instrumentation = get_default_instrumentation();
    quote! {
        #instrumentation
        fn start_all(&mut self) -> Result<::overwatch::overwatch::ServicesLifeCycleHandle, ::overwatch::overwatch::Error> {
            ::std::result::Result::Ok([#( #call_start ),*].try_into()?)
        }
    }
}

fn generate_start_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let cases = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        quote! {
            <#type_id as ::overwatch::services::ServiceData>::SERVICE_ID => {
                self.#field_identifier.service_runner::<<#type_id as ::overwatch::services::ServiceData>::StateOperator>().run::<#type_id>()?;
                ::std::result::Result::Ok(())
            }
        }
    });

    let instrumentation = get_default_instrumentation();
    quote! {
        #instrumentation
        fn start<S>(&mut self) -> Result<(), ::overwatch::overwatch::Error> where S: ::overwatch::utils::traits::AsRef<Self::AggregatedServiceId> + ::overwatch::services::ServiceData {
            match S::SERVICE_ID {
                #( #cases ),*
                service_id => ::std::result::Result::Err(::overwatch::overwatch::Error::Unavailable { service_id })
            }
        }
    }
}

fn generate_stop_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let cases = fields.iter().map(|field| {
        let _field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        // TODO: actually stop them here once service lifecycle is implemented
        quote! {
            <#type_id as ::overwatch::services::ServiceData>::SERVICE_ID => { unimplemented!() }
        }
    });

    let instrumentation = get_default_instrumentation();
    quote! {
        #instrumentation
        fn stop<S>(&mut self) -> Result<(), ::overwatch::overwatch::Error> where S: ::overwatch::utils::traits::AsRef<Self::AggregatedServiceId> + ::overwatch::services::ServiceData {
            match S::SERVICE_ID {
                #( #cases ),*
                service_id => ::std::result::Result::Err(::overwatch::overwatch::Error::Unavailable { service_id })
            }
        }
    }
}

fn generate_request_relay_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let cases = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        quote! {
            <#type_id as ::overwatch::services::ServiceData>::SERVICE_ID => {
                ::std::result::Result::Ok(::std::boxed::Box::new(
                    self.#field_identifier
                        .relay_with()
                        .ok_or(::overwatch::services::relay::RelayError::AlreadyConnected)?
                ) as ::overwatch::services::relay::AnyMessage)
            }
        }
    });

    let instrumentation = get_default_instrumentation();
    quote! {
        #instrumentation
        fn request_relay<S>(&mut self) -> ::overwatch::services::relay::RelayResult where S: ::overwatch::utils::traits::AsRef<Self::AggregatedServiceId> + ::overwatch::services::ServiceData {
            match S::SERVICE_ID {
                #( #cases )*
                service_id => ::std::result::Result::Err(::overwatch::services::relay::RelayError::Unavailable { service_id })
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
        quote! {
            <#type_id as ::overwatch::services::ServiceData>::SERVICE_ID => {
                    ::std::result::Result::Ok(self.#field_identifier.status_watcher())
            }
        }
    });

    quote! {
        #[::tracing::instrument(skip(self), err)]
        fn request_status_watcher<S>(&self) -> ::overwatch::services::status::ServiceStatusResult where S: ::overwatch::utils::traits::AsRef<Self::AggregatedServiceId> + ::overwatch::services::ServiceData {
            match S::SERVICE_ID {
                #( #cases )*
                service_id => ::std::result::Result::Err(::overwatch::services::status::ServiceStatusError::Unavailable { service_id })
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
        fn update_settings(&mut self, settings: Self::Settings) -> Result<(), ::overwatch::overwatch::Error> {
            let Self::Settings {
                #( #fields_settings ),*
            } = settings;

            #( #update_settings_call )*

            ::std::result::Result::Ok(())
        }
    }
}

fn generate_aggregate_service_types(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let aggregated_service_id = generate_aggregate_service_id(fields);
    let as_ref_impls = generate_as_ref_impls(fields);

    quote! {
        #aggregated_service_id

        #as_ref_impls
    }
}

fn generate_aggregate_service_id(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
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
    let expanded = quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        // TODO: Rename to `ServiceId` once the old `ServiceId` is removed.
        pub enum AggregatedServiceId {
            #(#enum_variants),*
        }
    };

    quote! {
        #expanded
    }
}

fn generate_as_ref_impls(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
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

        if let Type::Path(path) = &field_type {
            if let Some(path_segment) = path.path.segments.last() {
                if path_segment.ident == "OpaqueServiceHandle" {
                    // Extract the inner type inside OpaqueServiceHandle<T>
                    if let PathArguments::AngleBracketed(args) = &path_segment.arguments {
                        if let Some(GenericArgument::Type(inner_type)) = &args.args.first() {
                            match inner_type {
                                Type::Path(inner_path) => {
                                    let inner_ident = &inner_path
                                        .path
                                        .segments
                                        .last()
                                        .expect(
                                            "Expected at least one segment in the inner type path",
                                        )
                                        .ident;

                                    // Extract generics if present, but rename them to T1, T2, etc.
                                    if let Some(PathArguments::AngleBracketed(generic_args)) =
                                        &inner_path
                                            .path
                                            .segments
                                            .last()
                                            .map(|segment| segment.arguments.clone())
                                    {
                                        let generic_count = generic_args.args.len();
                                        let generic_params: Vec<_> = (1..=generic_count)
                                            .map(|i| format_ident!("T{}", i))
                                            .collect();

                                        return Some(quote! {
                                            impl<#(#generic_params),*> ::overwatch::utils::traits::AsRef<AggregatedServiceId> for #inner_ident<#(#generic_params),*> {
                                                fn as_ref() -> &'static AggregatedServiceId {
                                                    AggregatedServiceId::#capitalized_service_name
                                                }
                                            }
                                        })
                                    }
                                    // No generics case
                                    return Some(quote! {
                                        impl ::overwatch::utils::traits::AsRef<AggregatedServiceId> for #inner_ident {
                                            fn as_ref() -> &'static AggregatedServiceId {
                                                AggregatedServiceId::#capitalized_service_name
                                            }
                                        }
                                    })
                                }
                                _ => Option::<proc_macro2::TokenStream>::None,
                            };
                        }
                    }
                }
            }
        }
        None
    }).collect();

    quote! {
        #(#impl_blocks)*
    }
}
