use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{BoundLifetimes, GenericParam, Ident, ItemTrait, Result, Token, Type, TypeParamBound};

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
    proxy: Option<Ident>,
    runs: Vec<OperSpec>,
    steps: Vec<OperSpec>,
}

impl Parse for DriveArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut context = None;
        let mut error = None;
        let mut proxy = None;
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
                "proxy" => {
                    if proxy.is_some() {
                        return Err(syn::Error::new_spanned(key, "duplicate `proxy` argument"));
                    }

                    input.parse::<Token![=]>()?;
                    proxy = Some(input.parse()?);
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
            proxy,
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
    // Snapshot the trait before `#[drive]` synthesizes `context` bounds, so the
    // optional proxy trait reuses only the user-written generics and predicates.
    let original = item.clone();

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

    let main_output = quote! {
        #item

        impl #impl_generics #trait_name #trait_generics for __DriveImpl #where_clause {}
    };

    let proxy_output = match &args.proxy {
        Some(name) => expand_proxy(&original, &args, name)?,
        None => quote!(),
    };

    Ok(quote! {
        #main_output
        #proxy_output
    })
}

/// Emits the optional `XxxRepoProxy` aggregate trait plus its blanket impl.
///
/// The proxy trait is the `run(...)` and `step(...)` oper lists merged and
/// deduplicated, each as a `Proxy<O, Error = error>` supertrait. `Proxy` erases
/// the run-vs-step distinction, so no `Scope` or level predicates are needed.
fn expand_proxy(
    original: &ItemTrait,
    args: &DriveArgs,
    proxy_name: &Ident,
) -> Result<proc_macro2::TokenStream> {
    let orchestra = orchestra_path()?;
    let error = &args.error;

    // Merge run(...) then step(...), keeping the first occurrence of each oper.
    let mut specs: Vec<&OperSpec> = Vec::new();
    for spec in args.runs.iter().chain(args.steps.iter()) {
        if !specs.iter().any(|seen| oper_key(seen) == oper_key(spec)) {
            specs.push(spec);
        }
    }

    // Each merged oper becomes a `for<...> Proxy<Oper, Error = error>` bound.
    let mut bounds: Punctuated<TypeParamBound, Token![+]> = Punctuated::new();
    for spec in &specs {
        let lifetimes = &spec.lifetimes;
        let oper = &spec.oper;
        bounds.push(syn::parse2(quote!(
            #lifetimes #orchestra::Proxy<#oper, Error = #error>
        ))?);
    }

    // The proxy trait is context-free: drop the context type param when
    // `context = C` names one of the trait's generic type parameters.
    let mut proxy_generics = original.generics.clone();
    if let Some(context) = &args.context {
        if let Type::Path(path) = context {
            if path.qself.is_none()
                && path.path.leading_colon.is_none()
                && path.path.segments.len() == 1
                && path.path.segments[0].arguments.is_empty()
            {
                let context_ident = &path.path.segments[0].ident;
                proxy_generics.params = proxy_generics
                    .params
                    .into_iter()
                    .filter(|param| match param {
                        GenericParam::Type(type_param) => &type_param.ident != context_ident,
                        _ => true,
                    })
                    .collect();
            }
        }
    }

    let mut proxy_item = original.clone();
    proxy_item.ident = proxy_name.clone();
    proxy_item.generics = proxy_generics.clone();
    proxy_item.supertraits = bounds.clone();
    proxy_item.items.clear();

    let (_, trait_generics, _) = proxy_generics.split_for_impl();
    let mut impl_generics = proxy_generics.clone();
    impl_generics
        .params
        .insert(0, syn::parse_quote!(__ProxyImpl));
    impl_generics
        .make_where_clause()
        .predicates
        .push(syn::parse_quote!(__ProxyImpl: #bounds));
    let (impl_generics, _, where_clause) = impl_generics.split_for_impl();

    Ok(quote! {
        #proxy_item

        impl #impl_generics #proxy_name #trait_generics for __ProxyImpl #where_clause {}
    })
}

/// Token-level identity of an oper declaration, used for deduplication.
fn oper_key(spec: &OperSpec) -> String {
    let OperSpec { lifetimes, oper } = spec;
    quote!(#lifetimes #oper).to_string()
}

pub fn drive(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as DriveArgs);
    let item = syn::parse_macro_input!(item as ItemTrait);

    match expand_drive(args, item) {
        Ok(output) => output.into(),
        Err(error) => error.into_compile_error().into(),
    }
}
