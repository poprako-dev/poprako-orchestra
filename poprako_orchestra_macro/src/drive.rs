use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use quote::{format_ident, quote};
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

    if args.proxy.is_some() && args.runs.is_empty() && args.steps.is_empty() {
        return Err(syn::Error::new_spanned(
            args.proxy.as_ref().expect("checked proxy"),
            "`proxy = ...` requires at least one `run(...)` or `step(...)` operation",
        ));
    }

    let orchestra = orchestra_path()?;
    let error = &args.error;

    if !args.steps.is_empty() {
        let context = args.context.as_ref().expect("validated step context");
        item.generics
            .make_where_clause()
            .predicates
            .push(syn::parse2(quote!(#context: #orchestra::Context))?);
    }

    // Trait supertraits use `Self`; the blanket impl mirrors them with the
    // hidden `__DriveImpl` parameter instead.
    let mut trait_bounds = Punctuated::<TypeParamBound, Token![+]>::new();
    let mut impl_bounds = Punctuated::<TypeParamBound, Token![+]>::new();

    for spec in &args.runs {
        let lifetimes = &spec.lifetimes;
        let oper = &spec.oper;
        let bound = syn::parse2::<TypeParamBound>(quote!(
            #lifetimes #orchestra::Run<#oper, Error = #error>
        ))?;
        trait_bounds.push(bound.clone());
        impl_bounds.push(bound);
    }

    // Each step operation declares its own required level: `Step::Level` is
    // local to one `Step` implementation (per stepper + oper), so the aggregate
    // trait must not impose a shared level. It hoists every step requirement
    // into a `LevelGuard` supertrait instead. Supertraits are the one thing
    // rustc assumes for callers, so a usecase written against `R: Trait<C>`
    // can invoke any step without repeating per-operation level bounds, while
    // mismatches still fail at concrete instantiation through the blanket impl.
    for spec in &args.steps {
        let lifetimes = &spec.lifetimes;
        let oper = &spec.oper;
        let context = args.context.as_ref().expect("validated step context");

        let step_bound = syn::parse2::<TypeParamBound>(quote!(
            #lifetimes #orchestra::Step<#oper, #context, Error = #error>
        ))?;
        trait_bounds.push(step_bound.clone());
        impl_bounds.push(step_bound);

        trait_bounds.push(syn::parse2(quote!(
            #lifetimes #orchestra::LevelGuard<
                <#context as #orchestra::Context>::Level,
                <Self as #orchestra::Step<#oper, #context>>::Level
            >
        ))?);
        impl_bounds.push(syn::parse2(quote!(
            #lifetimes #orchestra::LevelGuard<
                <#context as #orchestra::Context>::Level,
                <__DriveImpl as #orchestra::Step<#oper, #context>>::Level
            >
        ))?);
    }

    item.supertraits.extend(trait_bounds);

    // The blanket impl must also carry the user-written supertraits (e.g.
    // `Send`) that the generated bounds do not cover.
    impl_bounds.extend(original.supertraits.iter().cloned());

    let trait_name = &item.ident;
    let trait_generics_owner = item.generics.clone();
    let (_, trait_generics, _) = trait_generics_owner.split_for_impl();

    let mut impl_generics = item.generics.clone();
    impl_generics
        .params
        .insert(0, syn::parse_quote!(__DriveImpl));
    impl_generics
        .make_where_clause()
        .predicates
        .push(syn::parse_quote!(__DriveImpl: #impl_bounds));
    let (impl_generics, _, where_clause) = impl_generics.split_for_impl();

    let main_output = quote! {
        #item

        impl #impl_generics #trait_name #trait_generics for __DriveImpl #where_clause {}
    };

    // A single proxy trait merges every `run(...)` and `step(...)` operation
    // into one capability, so complex logic depends on one name and never sees
    // whether an operation runs transactionally or not.
    let proxy_output = match &args.proxy {
        Some(name) => {
            let combined: Vec<&OperSpec> = args.runs.iter().chain(args.steps.iter()).collect();
            let proxy_trait = expand_proxy(&original, &args, name, &combined)?;
            let capability = expand_capability(&original, &args, name)?;
            quote! {
                #proxy_trait
                #capability
            }
        }
        None => quote!(),
    };

    Ok(quote! {
        #main_output
        #proxy_output
    })
}

fn proxy_generics(original: &ItemTrait, args: &DriveArgs) -> syn::Generics {
    let mut generics = original.generics.clone();
    if let Some(Type::Path(path)) = &args.context {
        if path.qself.is_none()
            && path.path.leading_colon.is_none()
            && path.path.segments.len() == 1
            && path.path.segments[0].arguments.is_empty()
        {
            let context_ident = &path.path.segments[0].ident;
            generics.params = generics
                .params
                .into_iter()
                .filter(|param| match param {
                    GenericParam::Type(type_param) => &type_param.ident != context_ident,
                    _ => true,
                })
                .collect();
        }
    }
    generics
}

fn expand_capability(
    original: &ItemTrait,
    args: &DriveArgs,
    proxy_name: &Ident,
) -> Result<proc_macro2::TokenStream> {
    let orchestra = orchestra_path()?;
    let descriptor = format_ident!("__poprako_proxy_capability_{proxy_name}");
    let runs = args.runs.iter().map(|spec| {
        let lifetimes = &spec.lifetimes;
        let oper = &spec.oper;
        quote!([#lifetimes #oper])
    });
    let steps = args.steps.iter().map(|spec| {
        let lifetimes = &spec.lifetimes;
        let oper = &spec.oper;
        quote!([#lifetimes #oper])
    });
    let mut generics = proxy_generics(original, args);
    for param in &mut generics.params {
        match param {
            GenericParam::Type(param) => param.default = None,
            GenericParam::Const(param) => param.default = None,
            GenericParam::Lifetime(_) => {}
        }
    }
    let generic_params = generics.params.iter().map(|param| quote!([#param]));

    Ok(quote! {
        #[doc(hidden)]
        #[macro_export]
        macro_rules! #descriptor {
            (
                [priority $($priority:ident)*]
                [context $($context:tt)*]
                [providers $($providers:tt)*]
                [pending $($pending:tt)*]
                [collected $($collected:tt)*]
                [current $mode:ident $provider:ident]
            ) => {
                #orchestra::__proxy_collect! {
                    [priority $($priority)*]
                    [context $($context)*]
                    [providers $($providers)*]
                    [pending $($pending)*]
                    [collected
                        $($collected)*
                        [
                            $mode
                            $provider
                            #proxy_name
                            [generics #(#generic_params)*]
                            [run #(#runs)*]
                            [step #(#steps)*]
                        ]
                    ]
                }
            };
        }

        #[doc(hidden)]
        pub use #descriptor as #proxy_name;

    })
}

/// Emits one optional proxy aggregate trait plus its blanket impl.
fn expand_proxy(
    original: &ItemTrait,
    args: &DriveArgs,
    proxy_name: &Ident,
    source_specs: &[&OperSpec],
) -> Result<proc_macro2::TokenStream> {
    let orchestra = orchestra_path()?;
    let error = &args.error;

    let mut specs: Vec<&&OperSpec> = Vec::new();
    for spec in source_specs {
        if !specs.iter().any(|seen| oper_key(seen) == oper_key(spec)) {
            specs.push(spec);
        }
    }

    // Each operation becomes a `for<...> Proxy<Oper, Error = error>` bound.
    // The proxy trait carries no context and no level: it erases the
    // transaction model entirely.
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
    let proxy_generics = proxy_generics(original, args);

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
