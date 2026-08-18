use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{BoundLifetimes, ItemTrait, Result, Token, Type, TypeParamBound};

struct OperSpec {
    lifetimes: Option<BoundLifetimes>,
    oper: Type,
}

impl Parse for OperSpec {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let lifetimes = input.parse::<Option<BoundLifetimes>>()?;
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
            let key = input.parse::<syn::Ident>()?;
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
                    runs.extend(Punctuated::<OperSpec, Token![,]>::parse_terminated(
                        &content,
                    )?);
                }
                "step" => {
                    let content;
                    syn::parenthesized!(content in input);
                    steps.extend(Punctuated::<OperSpec, Token![,]>::parse_terminated(
                        &content,
                    )?);
                }
                _ => return Err(syn::Error::new_spanned(key, "unsupported `drive` argument")),
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
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
            let name = syn::Ident::new(&name, proc_macro2::Span::call_site());
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
            "`#[drive]` requires at least one operation",
        ));
    }
    if !args.steps.is_empty() && args.context.is_none() {
        return Err(syn::Error::new_spanned(
            &item.ident,
            "`context = ...` is required for `step(...)`",
        ));
    }

    let orchestra = orchestra_path()?;
    let error = &args.error;
    if let Some(context) = &args.context {
        item.generics
            .make_where_clause()
            .predicates
            .push(syn::parse2(quote!(#context: #orchestra::Context))?);
    }

    let mut trait_bounds = Punctuated::<TypeParamBound, Token![+]>::new();
    let mut impl_bounds = Punctuated::<TypeParamBound, Token![+]>::new();
    for spec in &args.runs {
        let lifetimes = &spec.lifetimes;
        let oper = &spec.oper;
        let bound: TypeParamBound =
            syn::parse2(quote!(#lifetimes #orchestra::Run<#oper, Error = #error>))?;
        trait_bounds.push(bound.clone());
        impl_bounds.push(bound);
    }
    for spec in &args.steps {
        let context = args.context.as_ref().expect("validated context");
        let lifetimes = &spec.lifetimes;
        let oper = &spec.oper;
        let step_bound: TypeParamBound =
            syn::parse2(quote!(#lifetimes #orchestra::Step<#oper, #context, Error = #error>))?;
        let level_bound: TypeParamBound = syn::parse2(quote!(#lifetimes #orchestra::LevelGuard<
            <#context as #orchestra::Context>::Level,
            <Self as #orchestra::Step<#oper, #context>>::Level
        >))?;
        trait_bounds.push(step_bound.clone());
        trait_bounds.push(level_bound);
        impl_bounds.push(step_bound);
        impl_bounds.push(syn::parse2::<TypeParamBound>(
            quote!(#lifetimes #orchestra::LevelGuard<
            <#context as #orchestra::Context>::Level,
            <__DriveImpl as #orchestra::Step<#oper, #context>>::Level
        >),
        )?);
    }
    item.supertraits.extend(trait_bounds);
    let original_supertraits = item.supertraits.clone();
    let trait_name = &item.ident;
    let (_, trait_generics, _) = item.generics.split_for_impl();
    let mut impl_generics = item.generics.clone();
    impl_generics
        .params
        .insert(0, syn::parse_quote!(__DriveImpl));
    impl_bounds.extend(original_supertraits.iter().cloned());
    impl_generics
        .make_where_clause()
        .predicates
        .push(syn::parse_quote!(__DriveImpl: #impl_bounds));
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
