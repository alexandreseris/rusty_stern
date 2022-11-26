use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Update)]
pub fn update_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;

    let fields = if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(ref fields),
        ..
    }) = ast.data
    {
        fields
    } else {
        panic!("Only support Struct")
    };

    let mut idents = Vec::new();

    for field in fields.named.iter() {
        idents.push(&field.ident);
    }

    let expanded = quote! {
        impl Update for #struct_name {
            fn update_from(&mut self, other: Self) {
                #(
                    self.#idents = other.#idents;
                )*
            }
        }
    };
    expanded.into()
}
