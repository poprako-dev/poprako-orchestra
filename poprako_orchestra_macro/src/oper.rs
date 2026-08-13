use proc_macro::TokenStream;
use quote::quote;
use syn::{Attribute, DeriveInput, Result, Type};

struct OperArgs {
    output: Type,
}

fn parse_oper_attribute(attrs: &[Attribute]) -> Result<OperArgs> {
    let mut output = None;

    for attr in attrs {
        if !attr.path().is_ident("oper") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("output") {
                if output.is_some() {
                    return Err(meta.error("duplicate `output` attribute"));
                }

                let value = meta.value()?;
                output = Some(value.parse()?);
                return Ok(());
            }

            Err(meta.error("unsupported oper attribute; expected `output`"))
        })?;
    }

    let output = output.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "missing `output` in `#[oper(output = Type)]`",
        )
    })?;

    Ok(OperArgs { output })
}

pub fn derive_oper(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let generics = &input.generics;
    let args = match parse_oper_attribute(&input.attrs) {
        Ok(args) => args,
        Err(error) => return error.into_compile_error().into(),
    };
    let output = args.output;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics Oper for #name #ty_generics #where_clause {
            type Output = #output;
        }
    }
    .into()
}
