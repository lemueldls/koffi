extern crate proc_macro;
use proc_macro::TokenStream;

/// Export a free function or impl block to Kotlin.
///
/// Options:
/// - `name = "camelCaseName"` - override the generated Kotlin name
/// - `blocking` - run in a blocking coroutine dispatcher (`Dispatchers.IO`)
/// - `deprecated = "message"` - emit `@Deprecated` in Kotlin.
#[proc_macro_attribute]
pub fn export(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Mark a struct or enum as a transparent, postcard-serializable data type.
/// The type must implement `serde::Serialize + serde::DeserializeOwned + Clone`.
#[proc_macro_attribute]
pub fn data(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Mark a struct as an opaque handle.
///
/// Options:
/// - `mutable` - store in `Arc<RwLock<T>>`; enables &mut self methods.
#[proc_macro_attribute]
pub fn opaque(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Set the Kotlin package namespace for all declarations in this file.
///
/// Usage: `#![koffi::namespace("com.example.mylib")]`.
#[proc_macro_attribute]
pub fn namespace(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
