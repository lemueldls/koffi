extern crate proc_macro;
use proc_macro::TokenStream;

/// Marks a function, struct impl block, or trait impl for export to Kotlin.
///
/// Under the hood, this is a pass-through attribute that does not modify the Rust code itself.
/// The `koffi-bindgen` CLI tool will parse the AST of the source files to locate these annotations.
#[proc_macro_attribute]
pub fn export(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a struct or enum as a transparent, serializable FFI type.
/// The type must implement `Clone` and be composed of FFI-safe / postcard-serializable types.
#[proc_macro_attribute]
pub fn data(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Marks a struct as an opaque handle (never inspected by Kotlin directly).
/// It is managed by a thread-safe global registry and passed around as a `u64` handle ID.
#[proc_macro_attribute]
pub fn opaque(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Sets the Kotlin package/namespace namespace for generated declarations.
#[proc_macro_attribute]
pub fn namespace(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
