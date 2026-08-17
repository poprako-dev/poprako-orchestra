use proc_macro::TokenStream;

mod drive;
mod oper;
mod proxy;

#[proc_macro_derive(Oper, attributes(oper))]
pub fn derive_oper(input: TokenStream) -> TokenStream {
    oper::derive_oper(input)
}

#[proc_macro_attribute]
pub fn drive(attr: TokenStream, item: TokenStream) -> TokenStream {
    drive::drive(attr, item)
}

#[doc(hidden)]
#[proc_macro]
pub fn __proxy_collect(input: TokenStream) -> TokenStream {
    proxy::collect(input)
}

#[proc_macro]
pub fn proxy(input: TokenStream) -> TokenStream {
    proxy::proxy(input)
}
