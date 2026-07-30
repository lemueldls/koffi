use proc_macro::TokenStream;
use quote::quote;
use syn::{FnArg, Item, ItemFn, Pat, PatType, ReturnType, Visibility, parse_macro_input};

#[proc_macro_attribute]
pub fn export(_args: TokenStream, input: TokenStream) -> TokenStream {
    match parse_macro_input!(input as Item) {
        Item::Fn(f) => parse_function(f),
        Item::Struct(s) => TokenStream::from(quote! { #s }),
        Item::Impl(i) => TokenStream::from(quote! { #i }),
        _ => {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[koffi::export] can only be applied to a fn, a struct, or an impl block",
            )
            .to_compile_error()
            .into()
        }
    }
}

fn parse_function(input_fn: ItemFn) -> TokenStream {
    if !matches!(input_fn.vis, Visibility::Public(..)) {
        let span = input_fn.sig.ident.span();

        return syn::Error::new(span, "koffi: function must be public")
            .to_compile_error()
            .into();
    }

    let fn_name_raw = &input_fn.sig.ident.to_string();
    let fn_name = fn_name_raw.trim_start_matches("r#");

    let symbol_ident = quote::format_ident!("__KOFFI_FN_{}_ENTRY", fn_name.to_uppercase());

    let (param_names, param_types): (Vec<_>, Vec<_>) = input_fn
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            match arg {
                FnArg::Receiver(..) => None,
                FnArg::Typed(PatType { pat, ty, .. }) => {
                    if let Pat::Ident(pat_ident) = &**pat {
                        Some((pat_ident.ident.to_string(), ty))
                    } else {
                        None
                    }
                }
            }
        })
        .unzip();

    let return_type = match &input_fn.sig.output {
        ReturnType::Default => quote!(()),
        ReturnType::Type(_, ty) => quote!(#ty),
    };

    let expanded = quote! {
        #[deny(private_interfaces)]
        #input_fn

        #[used]
        #[unsafe(no_mangle)]
        #[unsafe(link_section = ".koffi_fns")]
        pub static #symbol_ident: ::koffi::FnShapeRef = ::koffi::FnShapeRef {
            name: #fn_name_raw,
            params: &[
                #( ::koffi::FnShapeParam {
                    name: #param_names,
                    param_type: ::koffi::TypeShapeRef::from_shape(<#param_types as ::facet::Facet>::SHAPE),
                } ),*
            ],
            return_type: ::koffi::TypeShapeRef::from_shape(<#return_type as ::facet::Facet>::SHAPE),
            module_path: ::core::option::Option::Some(::core::module_path!()),
            receiver: ::core::option::Option::None,
        };
    };

    TokenStream::from(expanded)
}
