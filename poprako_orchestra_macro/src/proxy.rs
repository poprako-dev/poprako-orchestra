use std::collections::HashSet;

use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use syn::parse::{Parse, ParseStream};
use syn::{
    BoundLifetimes, Expr, GenericParam, Ident, Result, Token, Type, bracketed, parenthesized,
};

mod keyword {
    syn::custom_keyword!(priority);
    syn::custom_keyword!(run);
    syn::custom_keyword!(step);
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Mode {
    Run,
    Step,
}

impl Mode {
    fn parse_ident(ident: &Ident) -> Result<Self> {
        match ident.to_string().as_str() {
            "run" => Ok(Self::Run),
            "step" => Ok(Self::Step),
            _ => Err(syn::Error::new_spanned(ident, "expected `run` or `step`")),
        }
    }

    fn tokens(self) -> TokenStream2 {
        match self {
            Self::Run => quote!(run),
            Self::Step => quote!(step),
        }
    }
}

#[derive(Clone)]
struct Declaration {
    mode: Mode,
    provider: Ident,
    capability: Ident,
}

struct ProxyInput {
    priority: Vec<Mode>,
    context: Option<Expr>,
    declarations: Vec<Declaration>,
}

fn parse_capabilities(
    input: ParseStream<'_>,
    mode: Mode,
    declarations: &mut Vec<Declaration>,
) -> Result<()> {
    loop {
        let provider = input.parse::<Ident>()?;
        input.parse::<Token![as]>()?;

        loop {
            declarations.push(Declaration {
                mode,
                provider: provider.clone(),
                capability: input.parse()?,
            });

            if !input.peek(Token![+]) {
                break;
            }
            input.parse::<Token![+]>()?;
        }

        if input.peek(Token![;]) {
            input.parse::<Token![;]>()?;
            return Ok(());
        }

        input.parse::<Token![,]>()?;
    }
}

impl Parse for ProxyInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut priority = None;
        let mut context = None;
        let mut declarations = Vec::new();
        let mut saw_run = false;
        let mut saw_step = false;

        while !input.is_empty() {
            if input.peek(keyword::priority) {
                if priority.is_some() {
                    return Err(input.error("duplicate `priority` section"));
                }
                input.parse::<keyword::priority>()?;
                input.parse::<Token![=>]>()?;
                let first = input.parse::<Ident>()?;
                input.parse::<Token![,]>()?;
                let second = input.parse::<Ident>()?;
                input.parse::<Token![;]>()?;
                let modes = vec![Mode::parse_ident(&first)?, Mode::parse_ident(&second)?];
                if modes[0] == modes[1] {
                    return Err(syn::Error::new_spanned(
                        second,
                        "priority must contain `run` and `step` exactly once",
                    ));
                }
                priority = Some(modes);
                continue;
            }

            if input.peek(keyword::run) {
                if saw_run {
                    return Err(input.error("duplicate `run` section"));
                }
                saw_run = true;
                input.parse::<keyword::run>()?;
                input.parse::<Token![=>]>()?;
                parse_capabilities(input, Mode::Run, &mut declarations)?;
                continue;
            }

            if input.peek(keyword::step) {
                if saw_step {
                    return Err(input.error("duplicate `step` section"));
                }
                saw_step = true;
                input.parse::<keyword::step>()?;
                let content;
                parenthesized!(content in input);
                context = Some(content.parse()?);
                input.parse::<Token![=>]>()?;
                parse_capabilities(input, Mode::Step, &mut declarations)?;
                continue;
            }

            return Err(input.error("expected `run`, `step(context)`, or `priority`"));
        }

        if declarations.is_empty() {
            return Err(input.error("`proxy!` requires at least one capability"));
        }

        Ok(Self {
            priority: priority.unwrap_or_else(|| vec![Mode::Step, Mode::Run]),
            context,
            declarations,
        })
    }
}

#[derive(Clone)]
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

struct SpecList(Vec<OperSpec>);

impl Parse for SpecList {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut specs = Vec::new();
        while !input.is_empty() {
            let content;
            bracketed!(content in input);
            specs.push(content.parse()?);
        }
        Ok(Self(specs))
    }
}

struct Entry {
    mode: Mode,
    provider: Ident,
    capability: Ident,
    generics: Vec<GenericParam>,
    runs: Vec<OperSpec>,
    steps: Vec<OperSpec>,
}

impl Parse for Entry {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mode = Mode::parse_ident(&input.parse()?)?;
        let provider = input.parse()?;
        let capability = input.parse()?;

        let generic_outer;
        bracketed!(generic_outer in input);
        generic_outer.parse::<Ident>()?;
        let mut generics = Vec::new();
        while !generic_outer.is_empty() {
            let generic;
            bracketed!(generic in generic_outer);
            generics.push(generic.parse()?);
        }

        let run_outer;
        bracketed!(run_outer in input);
        run_outer.parse::<keyword::run>()?;
        let runs = run_outer.parse::<SpecList>()?.0;

        let step_outer;
        bracketed!(step_outer in input);
        step_outer.parse::<keyword::step>()?;
        let steps = step_outer.parse::<SpecList>()?.0;

        Ok(Self {
            mode,
            provider,
            capability,
            generics,
            runs,
            steps,
        })
    }
}

struct CollectInput {
    priority: Vec<Mode>,
    context: Option<Expr>,
    providers: Vec<Ident>,
    pending: Vec<Declaration>,
    collected: Vec<Entry>,
}

fn parse_mode_list(input: ParseStream<'_>) -> Result<Vec<Mode>> {
    let content;
    bracketed!(content in input);
    content.parse::<Ident>()?;
    let mut modes = Vec::new();
    while !content.is_empty() {
        modes.push(Mode::parse_ident(&content.parse()?)?);
    }
    Ok(modes)
}

fn parse_context(input: ParseStream<'_>) -> Result<Option<Expr>> {
    let content;
    bracketed!(content in input);
    content.parse::<Ident>()?;
    if content.is_empty() {
        return Ok(None);
    }
    Ok(Some(content.parse()?))
}

fn parse_providers(input: ParseStream<'_>) -> Result<Vec<Ident>> {
    let content;
    bracketed!(content in input);
    content.parse::<Ident>()?;
    let mut providers = Vec::new();
    while !content.is_empty() {
        let provider;
        bracketed!(provider in content);
        providers.push(provider.parse()?);
    }
    Ok(providers)
}

fn parse_pending(input: ParseStream<'_>) -> Result<Vec<Declaration>> {
    let content;
    bracketed!(content in input);
    content.parse::<Ident>()?;
    let mut declarations = Vec::new();
    while !content.is_empty() {
        let declaration;
        bracketed!(declaration in content);
        declarations.push(Declaration {
            mode: Mode::parse_ident(&declaration.parse()?)?,
            provider: declaration.parse()?,
            capability: declaration.parse()?,
        });
    }
    Ok(declarations)
}

fn parse_collected(input: ParseStream<'_>) -> Result<Vec<Entry>> {
    let content;
    bracketed!(content in input);
    content.parse::<Ident>()?;
    let mut entries = Vec::new();
    while !content.is_empty() {
        let entry;
        bracketed!(entry in content);
        entries.push(entry.parse()?);
    }
    Ok(entries)
}

impl Parse for CollectInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        Ok(Self {
            priority: parse_mode_list(input)?,
            context: parse_context(input)?,
            providers: parse_providers(input)?,
            pending: parse_pending(input)?,
            collected: parse_collected(input)?,
        })
    }
}

fn orchestra_path() -> Result<TokenStream2> {
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

fn begin(input: ProxyInput) -> Result<TokenStream2> {
    let orchestra = orchestra_path()?;
    let priority = input.priority.iter().map(|mode| mode.tokens());
    let context = input.context;
    let declarations = input.declarations;
    let mut providers = Vec::new();
    for declaration in &declarations {
        if !providers
            .iter()
            .any(|seen: &Ident| seen == &declaration.provider)
        {
            providers.push(declaration.provider.clone());
        }
    }
    let pending = declarations.iter().map(|declaration| {
        let mode = declaration.mode.tokens();
        let provider = &declaration.provider;
        let capability = &declaration.capability;
        quote!([#mode #provider #capability])
    });
    let context = context.map(|context| quote!(#context));

    Ok(quote! {
        #orchestra::__proxy_collect! {
            [priority #(#priority)*]
            [context #context]
            [providers #([#providers])*]
            [pending #(#pending)*]
            [collected]
        }
    })
}

fn continue_collect(input: CollectInput) -> Result<TokenStream2> {
    let Some(current) = input.pending.first() else {
        return finish(input);
    };
    let descriptor = &current.capability;
    let priority = input.priority.iter().map(|mode| mode.tokens());
    let context = input.context.map(|context| quote!(#context));
    let providers = &input.providers;
    let pending = input.pending.iter().skip(1).map(|declaration| {
        let mode = declaration.mode.tokens();
        let provider = &declaration.provider;
        let capability = &declaration.capability;
        quote!([#mode #provider #capability])
    });
    let collected = input.collected.iter().map(entry_tokens);
    let mode = current.mode.tokens();
    let provider = &current.provider;

    Ok(quote! {
        #descriptor! {
            [priority #(#priority)*]
            [context #context]
            [providers #([#providers])*]
            [pending #(#pending)*]
            [collected #(#collected)*]
            [current #mode #provider]
        }
    })
}

fn spec_tokens(spec: &OperSpec) -> TokenStream2 {
    let lifetimes = &spec.lifetimes;
    let oper = &spec.oper;
    quote!([#lifetimes #oper])
}

fn entry_tokens(entry: &Entry) -> TokenStream2 {
    let mode = entry.mode.tokens();
    let provider = &entry.provider;
    let capability = &entry.capability;
    let runs = entry.runs.iter().map(spec_tokens);
    let steps = entry.steps.iter().map(spec_tokens);
    let generics = &entry.generics;
    quote!([
        #mode #provider #capability
        [generics #([#generics])*]
        [run #(#runs)*]
        [step #(#steps)*]
    ])
}

fn oper_key(spec: &OperSpec) -> String {
    let mut oper = spec.oper.to_token_stream().to_string();
    let mut lifetime_names = spec
        .lifetimes
        .iter()
        .flat_map(|lifetimes| lifetimes.lifetimes.iter())
        .filter_map(|param| match param {
            GenericParam::Lifetime(param) => Some(param.lifetime.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();
    lifetime_names.sort_by_key(|name| std::cmp::Reverse(name.len()));
    for (index, name) in lifetime_names.iter().enumerate() {
        oper = oper.replace(name, &format!("'__poprako_{index}"));
    }
    format!("{}:{oper}", lifetime_names.len())
}

fn proxy_impl(
    orchestra: &TokenStream2,
    providers: &[Ident],
    context: bool,
    entry: &Entry,
    spec: &OperSpec,
) -> TokenStream2 {
    let provider = &entry.provider;
    let oper = &spec.oper;
    let lifetime_params: Vec<_> = spec
        .lifetimes
        .as_ref()
        .into_iter()
        .flat_map(|lifetimes| lifetimes.lifetimes.iter())
        .collect();
    let capability_lifetimes: Vec<_> = entry
        .generics
        .iter()
        .filter(|param| matches!(param, GenericParam::Lifetime(_)))
        .collect();
    let capability_generics: Vec<_> = entry
        .generics
        .iter()
        .filter(|param| !matches!(param, GenericParam::Lifetime(_)))
        .collect();
    let context_lifetime = context.then(|| quote!('proxy_context,));
    let context_type = context.then(|| quote!(ProxyContext,));
    let context_args = context.then(|| quote!('proxy_context, ProxyContext,));
    let provider_generics = providers;

    match entry.mode {
        Mode::Run => quote! {
            #[allow(non_camel_case_types)]
            impl<
                #(#lifetime_params,)*
                #(#capability_lifetimes,)*
                'proxy_provider,
                #context_lifetime
                #(#capability_generics,)*
                #context_type
                #(#provider_generics),*
            > #orchestra::Proxy<#oper> for CapabilityProxy<
                'proxy_provider,
                #context_args
                #(#provider_generics),*
            >
            where
                #provider: #orchestra::Run<#oper>,
            {
                type Error = <#provider as #orchestra::Run<#oper>>::Error;

                fn exec(
                    &mut self,
                    oper: &#oper,
                ) -> impl ::core::future::Future<
                    Output = ::core::result::Result<
                        <#oper as #orchestra::Oper>::Output,
                        Self::Error,
                    >,
                > + Send {
                    <#provider as #orchestra::Run<#oper>>::run(self.#provider, oper)
                }
            }
        },
        Mode::Step => quote! {
            #[allow(non_camel_case_types)]
            impl<
                #(#lifetime_params,)*
                #(#capability_lifetimes,)*
                'proxy_provider,
                'proxy_context,
                #(#capability_generics,)*
                ProxyContext,
                #(#provider_generics),*
            > #orchestra::Proxy<#oper> for CapabilityProxy<
                'proxy_provider,
                'proxy_context,
                ProxyContext,
                #(#provider_generics),*
            >
            where
                ProxyContext: #orchestra::Context,
                #provider: #orchestra::Step<#oper, ProxyContext>,
                #provider: #orchestra::LevelGuard<
                    <ProxyContext as #orchestra::Context>::Level,
                    <#provider as #orchestra::Step<#oper, ProxyContext>>::Level,
                >,
            {
                type Error = <#provider as #orchestra::Step<#oper, ProxyContext>>::Error;

                fn exec(
                    &mut self,
                    oper: &#oper,
                ) -> impl ::core::future::Future<
                    Output = ::core::result::Result<
                        <#oper as #orchestra::Oper>::Output,
                        Self::Error,
                    >,
                > + Send {
                    <#provider as #orchestra::Step<#oper, ProxyContext>>::step(
                        self.#provider,
                        &mut *self.context,
                        oper,
                    )
                }
            }
        },
    }
}

fn finish(input: CollectInput) -> Result<TokenStream2> {
    let orchestra = orchestra_path()?;
    let has_context = input.context.is_some();
    let mut selected = Vec::new();
    let mut operations = HashSet::new();
    let mut capabilities = HashSet::new();

    for mode in &input.priority {
        for entry in input.collected.iter().filter(|entry| &entry.mode == mode) {
            let capability_key = format!("{}:{}", mode.tokens(), entry.capability);
            if !capabilities.insert(capability_key) {
                continue;
            }
            let specs = match mode {
                Mode::Run => &entry.runs,
                Mode::Step => &entry.steps,
            };
            for spec in specs {
                if operations.insert(oper_key(spec)) {
                    selected.push((entry, spec));
                }
            }
        }
    }

    let providers = &input.providers;
    let provider_fields = providers
        .iter()
        .map(|provider| quote!(#provider: &'proxy_provider #provider,));
    let context_field = has_context.then(|| quote!(context: &'proxy_context mut ProxyContext,));
    let context_generic = has_context.then(|| quote!('proxy_context, ProxyContext,));
    let context_value = input
        .context
        .map(|context| quote!(context: &mut *#context,));
    let implementations = selected
        .into_iter()
        .map(|(entry, spec)| proxy_impl(&orchestra, providers, has_context, entry, spec));

    Ok(quote! {{
        #[allow(non_camel_case_types)]
        struct CapabilityProxy<
            'proxy_provider,
            #context_generic
            #(#providers),*
        > {
            #context_field
            #(#provider_fields)*
        }

        #(#implementations)*

        CapabilityProxy {
            #context_value
            #(#providers),*
        }
    }})
}

pub fn proxy(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as ProxyInput);
    match begin(input) {
        Ok(output) => output.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

pub fn collect(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as CollectInput);
    match continue_collect(input) {
        Ok(output) => output.into(),
        Err(error) => error.into_compile_error().into(),
    }
}
