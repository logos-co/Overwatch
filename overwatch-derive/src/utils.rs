use proc_macro_error2::abort_call_site;
use quote::ToTokens;
use syn::{GenericArgument, PathArguments, Type};

pub fn extract_type_from(ty: &Type) -> Type {
    let stringify_type = ty.clone().into_token_stream().to_string();

    match ty {
        Type::Path(type_path) if type_path.qself.is_none() => {
            // Get the first segment of the path:
            let type_params = type_path
                .path
                .segments
                .iter()
                .next()
                .cloned()
                .unwrap()
                .arguments;
            // It should have only on angle-bracketed param ("<Foo>"):
            let generic_arg = match type_params {
                PathArguments::AngleBracketed(params) => {
                    params.args.iter().next().cloned().unwrap()
                }
                _ => abort_call_site!("Expected single type argument, found {}", stringify_type),
            };
            // This argument must be a type:
            match generic_arg {
                GenericArgument::Type(ty) => ty,
                _ => abort_call_site!("Expected single type argument, found {}", stringify_type),
            }
        }
        _ => abort_call_site!("Expected single type argument, found {}", stringify_type),
    }
}

pub fn field_name_to_type_name(name: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for symbol in name.chars() {
        if symbol == '_' {
            // Skip the underscore, and mark the next letter for capitalization
            capitalize_next = true;
        } else if capitalize_next {
            // Capitalize the current character
            result.push(
                symbol
                    .to_uppercase()
                    .next()
                    .expect("Name cannot end with an underscore."),
            );
            capitalize_next = false;
        } else {
            // Append the character as-is (lowercase)
            result.push(
                symbol
                    .to_lowercase()
                    .next()
                    .expect("Name cannot end with an underscore."),
            );
        }
    }

    result
}
