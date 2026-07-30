pub use koffi_core::*;
pub use koffi_macros::*;
pub use koffi_runtime::*;

facet::define_attr_grammar! {
    ns "koffi";
    crate_path ::koffi;

    pub enum Attr {
        Namespace(&'static str)
    }
}
