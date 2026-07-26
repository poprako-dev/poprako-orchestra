use proc_macro::TokenStream;
use quote::quote;
use syn::{Attribute, DeriveInput, Result, Type};

fn parse_output_attribute(attrs: &[Attribute]) -> Result<Type> {
    let mut output = None;

    for attr in attrs {
        if !attr.path().is_ident("oper") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("output") {
                return Err(meta.error("unsupported oper attribute; expected `output`"));
            }

            if output.is_some() {
                return Err(meta.error("duplicate `output` attribute"));
            }

            let value = meta.value()?;
            output = Some(value.parse()?);
            Ok(())
        })?;
    }

    output.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "missing `#[oper(output = Type)]` attribute",
        )
    })
}

pub fn derive_oper(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = &input.generics;
    let output = match parse_output_attribute(&input.attrs) {
        Ok(output) => output,
        Err(error) => return error.into_compile_error().into(),
    };
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics Oper for #name #ty_generics #where_clause {
            type Output = #output;
        }
    }
    .into()
}
