use convert_case::{Case, Casing as _};
use proc_macro_error2::abort_call_site;
use quote::ToTokens as _;
use syn::{GenericArgument, PathArguments, Type, TypePath};

pub fn extract_type_from(ty: &Type) -> Type {
    let stringify_type = ty.clone().into_token_stream().to_string();

    let Type::Path(TypePath { path, .. }) = ty else {
        abort_call_site!("Expected a type path, found {}", stringify_type)
    };

    let last_segment = path.segments.last().unwrap();
    match &last_segment.arguments {
        PathArguments::AngleBracketed(params) => {
            if params.args.is_empty() {
                abort_call_site!(
                    "Expected at least one generic argument, found {}",
                    stringify_type
                );
            }

            // Backward-compatible: Extract **only the first generic argument** (previous
            // behavior)
            let first_generic = params.args.iter().next().unwrap();

            let GenericArgument::Type(inner_ty) = first_generic else {
                abort_call_site!(
                    "Expected a type as the first generic argument, found {}",
                    stringify_type
                );
            };
            inner_ty.clone()
        }
        PathArguments::None => {
            // New behavior: If there are no generics, return the type as-is.
            ty.clone()
        }
        PathArguments::Parenthesized(_) => {
            abort_call_site!("Unexpected type argument format in {}", stringify_type)
        }
    }
}

pub fn field_name_to_type_name(name: &str) -> String {
    name.to_case(Case::Pascal)
}
