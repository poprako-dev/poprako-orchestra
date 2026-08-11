use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{BoundLifetimes, Ident, ItemTrait, Result, Token, Type, TypeParamBound};

struct OperSpec {
    lifetimes: Option<BoundLifetimes>,
    oper: Type,
}

impl Parse for OperSpec {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let lifetimes = match input.peek(Token![for]) {
            true => Some(input.parse()?),
            false => None,
        };
        let oper = input.parse()?;

        Ok(Self { lifetimes, oper })
    }
}

struct DriveArgs {
    context: Option<Type>,
    error: Type,
    runs: Vec<OperSpec>,
    steps: Vec<OperSpec>,
}

impl Parse for DriveArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut context = None;
        let mut error = None;
        let mut runs = Vec::new();
        let mut steps = Vec::new();

        while !input.is_empty() {
            let key = input.parse::<Ident>()?;

            match key.to_string().as_str() {
                "context" => {
                    if context.is_some() {
                        return Err(syn::Error::new_spanned(key, "duplicate `context` argument"));
                    }

                    input.parse::<Token![=]>()?;
                    context = Some(input.parse()?);
                }
                "error" => {
                    if error.is_some() {
                        return Err(syn::Error::new_spanned(key, "duplicate `error` argument"));
                    }

                    input.parse::<Token![=]>()?;
                    error = Some(input.parse()?);
                }
                "run" => {
                    let content;
                    syn::parenthesized!(content in input);
                    let specs = Punctuated::<OperSpec, Token![,]>::parse_terminated(&content)?;
                    runs.extend(specs);
                }
                "step" => {
                    let content;
                    syn::parenthesized!(content in input);
                    let specs = Punctuated::<OperSpec, Token![,]>::parse_terminated(&content)?;
                    steps.extend(specs);
                }
                _ => return Err(syn::Error::new_spanned(key, "unsupported `drive` argument")),
            }

            if input.is_empty() {
                continue;
            }

            input.parse::<Token![,]>()?;
        }

        let error = error.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "missing `error = Type` argument",
            )
        })?;

        Ok(Self {
            context,
            error,
            runs,
            steps,
        })
    }
}

fn orchestra_path() -> Result<proc_macro2::TokenStream> {
    let crate_name = proc_macro_crate::crate_name("poprako-orchestra")
        .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error))?;

    match crate_name {
        FoundCrate::Itself => Ok(quote!(crate)),
        FoundCrate::Name(name) => {
            let name = Ident::new(&name, proc_macro2::Span::call_site());
            Ok(quote!(::#name))
        }
    }
}

fn expand_drive(args: DriveArgs, mut item: ItemTrait) -> Result<proc_macro2::TokenStream> {
    if !item.items.is_empty() {
        return Err(syn::Error::new_spanned(
            &item,
            "`#[drive]` only supports empty aggregate traits",
        ));
    }

    if args.runs.is_empty() && args.steps.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.ident,
            "`#[drive]` requires at least one `run(...)` or `step(...)` argument",
        ));
    }

    if !args.steps.is_empty() && args.context.is_none() {
        return Err(syn::Error::new_spanned(
            &item.ident,
            "`context = ...` is required when `step(...)` is present",
        ));
    }

    let orchestra = orchestra_path()?;
    let error = &args.error;
    let mut bounds = Vec::<TypeParamBound>::new();

    if !args.steps.is_empty() {
        let context = args.context.as_ref().expect("validated step context");
        item.generics
            .make_where_clause()
            .predicates
            .push(syn::parse2(quote!(#context: #orchestra::Scope))?);
    }

    for spec in &args.runs {
        let lifetimes = &spec.lifetimes;
        let oper = &spec.oper;
        bounds.push(syn::parse2(quote!(
            #lifetimes #orchestra::Run<#oper, Error = #error>
        ))?);
    }

    for spec in &args.steps {
        let lifetimes = &spec.lifetimes;
        let oper = &spec.oper;
        let context = args.context.as_ref().expect("validated step context");
        bounds.push(syn::parse2(quote!(
            #lifetimes #orchestra::Step<#oper, #context, Error = #error>
        ))?);
        item.generics
            .make_where_clause()
            .predicates
            .push(syn::parse2(quote!(
                #lifetimes <#context as #orchestra::Scope>::Level:
                    #orchestra::AtLeast<<#oper as #orchestra::Oper>::Level>
            ))?);
    }

    item.supertraits.extend(bounds);

    let trait_name = &item.ident;
    let all_bounds = item.supertraits.clone();
    let (_, trait_generics, _) = item.generics.split_for_impl();
    let mut impl_generics = item.generics.clone();
    impl_generics
        .params
        .insert(0, syn::parse_quote!(__DriveImpl));
    impl_generics
        .make_where_clause()
        .predicates
        .push(syn::parse_quote!(__DriveImpl: #all_bounds));
    let (impl_generics, _, where_clause) = impl_generics.split_for_impl();

    Ok(quote! {
        #item

        impl #impl_generics #trait_name #trait_generics for __DriveImpl #where_clause {}
    })
}

pub fn drive(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as DriveArgs);
    let item = syn::parse_macro_input!(item as ItemTrait);

    match expand_drive(args, item) {
        Ok(output) => output.into(),
        Err(error) => error.into_compile_error().into(),
    }
}
