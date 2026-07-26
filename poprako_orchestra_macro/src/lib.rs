use proc_macro::TokenStream;

mod drive;
mod oper;

#[proc_macro_derive(Oper, attributes(oper))]
pub fn derive_oper(input: TokenStream) -> TokenStream {
    oper::derive_oper(input)
}

#[proc_macro_attribute]
pub fn drive(attr: TokenStream, item: TokenStream) -> TokenStream {
    drive::drive(attr, item)
}
