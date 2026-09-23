use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, LitStr, parse_macro_input};

#[proc_macro_derive(Table, attributes(table))]
pub fn derive_table(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            name,
            "Table can only be derived for structs",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            name,
            "Table requires named fields",
        ));
    };

    let field_idents: Vec<_> = fields
        .named
        .iter()
        .map(|f| f.ident.as_ref().expect("named"))
        .collect();
    let columns: Vec<_> = field_idents.iter().map(|i| i.to_string()).collect();
    let table_override = table_override(input)?;
    let table_fn = table_override.map(|table| {
        quote! {
            fn table() -> String {
                #table.to_string()
            }
        }
    });

    Ok(quote! {
        impl ::homestead::Table for #name {
            const COLUMNS: &'static [&'static str] = &[#(#columns),*];

            #table_fn

            fn from_row(row: &::homestead::rusqlite::Row<'_>) -> ::homestead::rusqlite::Result<Self> {
                Ok(Self {
                    #(#field_idents: row.get(stringify!(#field_idents))?),*
                })
            }

            fn values(&self) -> Vec<::homestead::Bind> {
                vec![
                    #(::homestead::Bind::from(self.#field_idents.clone())),*
                ]
            }
        }
    })
}

fn table_override(input: &DeriveInput) -> syn::Result<Option<String>> {
    for attr in &input.attrs {
        if attr.path().is_ident("table") {
            let lit: LitStr = attr.parse_args()?;
            return Ok(Some(lit.value()));
        }
    }
    Ok(None)
}
