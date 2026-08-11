use proc_macro::TokenStream;
use quote::quote;
use syn::{Attribute, DeriveInput, Result, Type};

struct OperArgs {
    output: Type,
    level: Type,
}

fn parse_oper_attribute(attrs: &[Attribute]) -> Result<OperArgs> {
    let mut output = None;
    let mut level = None;

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

            if meta.path.is_ident("level") {
                if level.is_some() {
                    return Err(meta.error("duplicate `level` attribute"));
                }

                let value = meta.value()?;
                level = Some(value.parse()?);
                return Ok(());
            }

            Err(meta.error("unsupported oper attribute; expected `output` or `level`"))
        })?;
    }

    let output = output.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "missing `output` in `#[oper(output = Type, level = Level)]`",
        )
    })?;
    let level = level.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "missing `level` in `#[oper(output = Type, level = Level)]`",
        )
    })?;

    Ok(OperArgs { output, level })
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
    let level = args.level;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote! {
        impl #impl_generics Oper for #name #ty_generics #where_clause {
            type Output = #output;
            type Level = #level;
        }
    }
    .into()
}
