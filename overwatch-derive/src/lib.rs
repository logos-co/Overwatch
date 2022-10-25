mod utils;

use proc_macro_error::{abort_call_site, proc_macro_error};
use quote::{format_ident, quote};
use syn::{punctuated::Punctuated, token::Comma, Data, DeriveInput, Field};

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
    match data {
        Data::Struct(DataStruct {
            fields: syn::Fields::Named(fields),
            ..
        }) => impl_services_for_struct(struct_identifier, &fields.named),
        _ => {
            abort_call_site!("Deriving Services is only supported for named Structs");
        }
    }
}

fn impl_services_for_struct(
    identifier: &proc_macro2::Ident,
    fields: &Punctuated<Field, Comma>,
) -> proc_macro2::TokenStream {
    let settings = generate_services_settings(identifier, fields);
    let unique_ids_check = generate_assert_unique_identifiers(identifier, fields);
    let services_impl = generate_services_impl(identifier, fields);

    quote! {
        #unique_ids_check

        #settings

        #services_impl
    }
}

fn generate_services_settings(
    services_identifier: &proc_macro2::Ident,
    fields: &Punctuated<Field, Comma>,
) -> proc_macro2::TokenStream {
    let services_settings = fields.iter().map(|field| {
        let service_name = field.ident.as_ref().expect("A named struct attribute");
        let _type = utils::extract_type_from(&field.ty);

        quote!(pub #service_name: <#_type as ::overwatch::services::ServiceData>::Settings)
    });
    let services_settings_identifier = service_settings_identifier_from(services_identifier);
    quote! {
        #[derive(::std::clone::Clone, ::std::fmt::Debug)]
        pub struct #services_settings_identifier {
            #( #services_settings ),*
        }
    }
}

fn generate_assert_unique_identifiers(
    services_identifier: &proc_macro2::Ident,
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

    quote! {
        const #services_ids_check: () = assert!(::overwatch::utils::const_checks::unique_ids(&[#( #services_ids ),*]));
    }
}

fn generate_services_impl(
    services_identifier: &proc_macro2::Ident,
    fields: &Punctuated<Field, Comma>,
) -> proc_macro2::TokenStream {
    let services_settings_identifier = service_settings_identifier_from(services_identifier);
    let impl_new = generate_new_impl(fields);
    let impl_start_all = generate_start_all_impl(fields);
    let impl_start = generate_start_impl(fields);
    let impl_stop = generate_stop_impl(fields);
    let impl_relay = generate_request_relay_impl(fields);
    let impl_update_settings = generate_update_settings_impl(fields);

    quote! {
        impl ::overwatch::overwatch::Services for #services_identifier {
            type Settings = #services_settings_identifier;

            #impl_new

            #impl_start_all

            #impl_start

            #impl_stop

            #impl_relay

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
                    ::overwatch::services::handle::ServiceHandle::<#service_type>::new(
                        #settings_field_identifier, overwatch_handle.clone(),
                );
                manager
            }
        }
    });

    quote! {
        fn new(settings: Self::Settings, overwatch_handle: ::overwatch::overwatch::handle::OverwatchHandle) -> Self {
            let Self::Settings {
                #( #fields_settings ),*
            } = settings;

            let app = Self {
                #( #managers ),*
            };

            app
        }
    }
}

fn generate_start_all_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let call_start = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        quote! {
            self.#field_identifier.service_runner().run();
        }
    });

    quote! {
        #[::tracing::instrument(skip(self), err)]
        fn start_all(&mut self) -> Result<(), ::overwatch::overwatch::Error> {
            #( #call_start )*
            Ok(())
        }
    }
}

fn generate_start_impl(fields: &Punctuated<Field, Comma>) -> proc_macro2::TokenStream {
    let cases = fields.iter().map(|field| {
        let field_identifier = field.ident.as_ref().expect("A struct attribute identifier");
        let type_id = utils::extract_type_from(&field.ty);
        quote! {
            <#type_id as ::overwatch::services::ServiceData>::SERVICE_ID => {
                self.#field_identifier.service_runner().run();
                Ok(())
            }
        }
    });

    quote! {
        #[::tracing::instrument(skip(self), err)]
        fn start(&mut self, service_id: ::overwatch::services::ServiceId) -> Result<(), ::overwatch::overwatch::Error> {
            match service_id {
                #( #cases ),*
                service_id => Err(::overwatch::overwatch::Error::Unavailable { service_id })
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

    quote! {
        #[::tracing::instrument(skip(self), err)]
        fn stop(&mut self, service_id: ::overwatch::services::ServiceId) -> Result<(), ::overwatch::overwatch::Error> {
            match service_id {
                #( #cases ),*
                service_id => Err(::overwatch::overwatch::Error::Unavailable { service_id })
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
                Ok(::std::boxed::Box::new(
                    self.#field_identifier
                        .relay_with()
                        .expect("An open relay to service is established")
                ) as ::overwatch::services::relay::AnyMessage)
            }
        }
    });

    quote! {
        #[::tracing::instrument(skip(self), err)]
        fn request_relay(&mut self, service_id: ::overwatch::services::ServiceId) -> ::overwatch::services::relay::RelayResult {
            {
                match service_id {
                    #( #cases )*
                    service_id => Err(::overwatch::services::relay::RelayError::Unavailable { service_id })
                }
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

    quote! {
        #[::tracing::instrument(skip(self, settings), err)]
        fn update_settings(&mut self, settings: Self::Settings) -> Result<(), ::overwatch::overwatch::Error> {
            let Self::Settings {
                #( #fields_settings ),*
            } = settings;

            #( #update_settings_call )*

            Ok(())
        }
    }
}
