use convert_case::{Case, Casing as _};
use proc_macro_error2::abort_call_site;
use quote::ToTokens as _;
use syn::{GenericArgument, PathArguments, Type, TypePath};

/// Extracts the inner type from a generic type or returns the type as-is if it
/// has no generics.
///
/// # Behavior
/// - If the provided type is a generic type (e.g., `Wrapper<T>`), the function
///   extracts and returns the first generic argument (`T`).
/// - If the type has no generics (e.g., `String`), it returns the type
///   unchanged.
/// - If the input type is not a valid type path, a compile-time error is
///   triggered.
///
/// # Errors
/// This function will trigger a compile-time error if:
/// - The input type is not a valid type path.
/// - The first generic argument is not a type.
/// - The type has an unexpected argument format (e.g., parenthesized generics).
///
/// # Examples
/// ```rust,ignore
/// use crate::overwatch_derive::utils::extract_type_from;
/// use syn::{parse_quote, Type};
///
/// let ty: Type = parse_quote!(Option<u32>);
/// assert_eq!(extract_type_from(&ty), parse_quote!(u32));
///
/// let ty: Type = parse_quote!(Result<String, u32>);
/// assert_eq!(extract_type_from(&ty), parse_quote!(String));
///
/// let ty: Type = parse_quote!(String);
/// assert_eq!(extract_type_from(&ty), parse_quote!(String));
/// ```
///
/// # Notes
/// - If no generic arguments are found, the function simply returns the
///   original type.
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

/// Converts a field name (typically in `snake_case`) to a type name in
/// `PascalCase`.
///
/// # Behavior
/// - Converts a field name, usually in `snake_case`, to `PascalCase`.
///
/// # Examples
/// ```rust,ignore
/// assert_eq!(field_name_to_type_name("my_field"), "MyField");
/// assert_eq!(field_name_to_type_name("some_longer_field_name"), "SomeLongerFieldName");
/// assert_eq!(field_name_to_type_name("UPPER_CASE_FIELD"), "UpperCaseField");
/// ```
///
/// # Notes
/// - This function is useful when generating struct or enum type names from
///   field names.
/// - Assumes the input follows Rust naming conventions (e.g., `snake_case` for
///   fields).
pub fn field_name_to_type_name(name: &str) -> String {
    name.to_case(Case::Pascal)
}
