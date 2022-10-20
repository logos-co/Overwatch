use proc_macro_error::abort_call_site;
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
