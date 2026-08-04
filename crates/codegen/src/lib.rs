pub mod config {
    //! The `KotlinConfig` lives in `koffi-build` (it drives staging too);
    //! re-exported here so codegen callers keep one import path.
    pub use koffi_build::config::*;
}
pub mod extract;
pub mod generator;
pub mod layout;
pub mod schema;
