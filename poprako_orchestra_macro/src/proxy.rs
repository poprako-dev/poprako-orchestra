use std::collections::HashSet;

use proc_macro::TokenStream;
use proc_macro_crate::FoundCrate;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Expr, GenericParam, Ident, Result, Token, Type};

struct Binder {
    params: Punctuated<GenericParam, Token![,]>,
}

impl Parse for Binder {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        input.parse::<Token![for]>()?;
        input.parse::<Token![<]>()?;
        let params = Punctuated::parse_separated_nonempty(input)?;
        input.parse::<Token![>]>()?;
        Ok(Self { params })
    }
}

struct OperSpec {
    binder: Option<Binder>,
    oper: Type,
}

impl Parse for OperSpec {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let binder = if input.peek(Token![for]) {
            Some(input.parse()?)
        } else {
            None
        };
        let oper = input.parse()?;
        Ok(Self { binder, oper })
    }
}

struct Declaration {
    provider: Ident,
    operations: Vec<OperSpec>,
}

impl Parse for Declaration {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let provider = input.parse()?;
        input.parse::<Token![=>]>()?;
        let mut operations = vec![input.parse()?];
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            if input.peek(Token![;]) {
                break;
            }
            operations.push(input.parse()?);
        }
        input.parse::<Token![;]>()?;
        Ok(Self {
            provider,
            operations,
        })
    }
}

struct ProxyInput {
    mode: Mode,
    context: Option<Expr>,
    declarations: Vec<Declaration>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Mode {
    Run,
    Step,
}

impl Parse for ProxyInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mode = input.parse::<Ident>()?;
        let mode = match mode.to_string().as_str() {
            "run" => Mode::Run,
            "step" => Mode::Step,
            _ => return Err(syn::Error::new_spanned(mode, "expected `run` or `step`")),
        };
        let context = match mode {
            Mode::Run => None,
            Mode::Step => {
                let content;
                syn::parenthesized!(content in input);
                Some(content.parse()?)
            }
        };
        let content;
        syn::braced!(content in input);
        let mut declarations = Vec::new();
        while !content.is_empty() {
            declarations.push(content.parse()?);
        }
        validate(&declarations)?;
        Ok(ProxyInput {
            mode,
            context,
            declarations,
        })
    }
}

fn validate(declarations: &[Declaration]) -> Result<()> {
    if declarations.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "`proxy!` requires at least one provider",
        ));
    }
    let mut providers = HashSet::new();
    let mut operations = HashSet::new();
    for declaration in declarations {
        let provider = declaration.provider.to_string();
        if !providers.insert(provider) {
            return Err(syn::Error::new_spanned(
                &declaration.provider,
                "duplicate provider",
            ));
        }
        if declaration.operations.is_empty() {
            return Err(syn::Error::new_spanned(
                &declaration.provider,
                "provider must declare an operation",
            ));
        }
        for operation in &declaration.operations {
            let binder = operation.binder.as_ref().map(|binder| &binder.params);
            let oper = &operation.oper;
            let key = quote!(#binder #oper).to_string();
            if !operations.insert(key) {
                return Err(syn::Error::new_spanned(
                    &operation.oper,
                    "duplicate operation",
                ));
            }
        }
    }
    Ok(())
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

fn impl_tokens(
    orchestra: &TokenStream2,
    input: &ProxyInput,
    declaration: &Declaration,
    operation: &OperSpec,
) -> TokenStream2 {
    let provider = &declaration.provider;
    let oper = &operation.oper;
    let binder = operation.binder.as_ref().map(|binder| &binder.params);
    let context = input.context.as_ref();
    let params = match binder {
        Some(params) => quote!('proxy, #params),
        None => quote!('proxy),
    };
    let provider_params = input
        .declarations
        .iter()
        .map(|declaration| &declaration.provider);
    let provider_types = input
        .declarations
        .iter()
        .map(|declaration| &declaration.provider);
    let proxy_type = match input.mode {
        Mode::Run => quote!(ProxyImpl<'proxy, #(#provider_types),*>),
        Mode::Step => quote!(ProxyImpl<'proxy, #(#provider_types),*, ProxyContext>),
    };
    match input.mode {
        Mode::Run => quote! {
            #[allow(non_camel_case_types)]
            impl<#params, #(#provider_params),*> #orchestra::Proxy<#oper> for #proxy_type
            where
                #provider: #orchestra::Run<#oper>,
            {
                type Error = <#provider as #orchestra::Run<#oper>>::Error;

                fn exec(&mut self, oper: &#oper) -> impl ::core::future::Future<
                    Output = ::core::result::Result<<#oper as #orchestra::Oper>::Output, Self::Error>
                > + Send {
                    <#provider as #orchestra::Run<#oper>>::run(self.#provider, oper)
                }
            }
        },
        Mode::Step => {
            let _context = context.expect("step proxy has context");
            quote! {
                #[allow(non_camel_case_types)]
                impl<#params, #(#provider_params),*, ProxyContext> #orchestra::Proxy<#oper> for #proxy_type
                where
                    ProxyContext: #orchestra::Context,
                    #provider: #orchestra::Step<#oper, ProxyContext>,
                    #provider: #orchestra::LevelGuard<
                        <ProxyContext as #orchestra::Context>::Level,
                        <#provider as #orchestra::Step<#oper, ProxyContext>>::Level,
                    >,
                {
                    type Error = <#provider as #orchestra::Step<#oper, ProxyContext>>::Error;

                    fn exec(&mut self, oper: &#oper) -> impl ::core::future::Future<
                        Output = ::core::result::Result<<#oper as #orchestra::Oper>::Output, Self::Error>
                    > + Send {
                        <#provider as #orchestra::Step<#oper, ProxyContext>>::step(
                            self.#provider,
                            &mut *self.context,
                            oper,
                        )
                    }
                }
            }
        }
    }
}

fn expand(input: ProxyInput) -> Result<TokenStream2> {
    let orchestra = orchestra_path()?;
    let providers = input
        .declarations
        .iter()
        .map(|declaration| &declaration.provider);
    let fields = input.declarations.iter().map(|declaration| {
        let provider = &declaration.provider;
        quote!(#provider: &'proxy #provider,)
    });
    let context_field = input
        .context
        .as_ref()
        .map(|_| quote!(context: &'proxy mut ProxyContext,));
    let context_generic = input.context.as_ref().map(|_| quote!(, ProxyContext));
    let provider_generics = input
        .declarations
        .iter()
        .map(|declaration| &declaration.provider);
    let implementations = input.declarations.iter().flat_map(|declaration| {
        declaration
            .operations
            .iter()
            .map(|operation| impl_tokens(&orchestra, &input, declaration, operation))
    });
    let context_value = input
        .context
        .as_ref()
        .map(|context| quote!(context: &mut *#context,));
    let provider_values = providers.map(|provider| quote!(#provider,));
    Ok(quote! {{
        #[allow(non_camel_case_types)]
        struct ProxyImpl<'proxy, #(#provider_generics),* #context_generic> {
            #(#fields)*
            #context_field
        }

        #(#implementations)*

        ProxyImpl {
            #context_value
            #(#provider_values)*
        }
    }})
}

pub fn proxy(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as ProxyInput);
    match expand(input) {
        Ok(output) => output.into(),
        Err(error) => error.into_compile_error().into(),
    }
}
