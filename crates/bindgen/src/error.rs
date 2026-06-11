use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum BindgenError {
    #[error("Rustdoc resolution failed: {0}")]
    RustdocFailed(String),

    #[error("Unsupported type: {0}")]
    UnsupportedType(String),

    #[error("Could not resolve graph")]
    NoResolveGraph,

    #[error("No root package in resolve graph")]
    NoRootPackage,

    #[error("Package not found: {0}")]
    PackageNotFound(String),

    #[error("Cargo build failed for target: {0}")]
    CargoBuildFailed(String),

    #[error("Empty type path")]
    EmptyTypePath,

    #[error("Expected 1 generic argument for {0}")]
    ExpectedOneGeneric(String),

    #[error("Expected 2 generic arguments for {0}")]
    ExpectedTwoGenerics(String),

    #[error("Cargo.toml error: {0}")]
    CargoTomlError(String),

    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    #[error("Environment variable error: {0}")]
    EnvError(#[from] std::env::VarError),

    #[error("Syntax error: {0}")]
    SyntaxError(#[from] syn::Error),

    #[error("Cargo metadata error: {0}")]
    CargoMetadataError(#[from] cargo_metadata::Error),

    #[error("JSON error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("Facet JSON serialization error: {0}")]
    FacetJsonSerializeError(#[from] facet_format::SerializeError<facet_json::JsonSerializeError>),

    #[error("TOML parsing error: {0}")]
    TomlError(#[from] toml::de::Error),

    #[error("Template rendering error: {0}")]
    TemplateError(#[from] askama::Error),

    #[error("WalkDir error: {0}")]
    WalkDirError(#[from] walkdir::Error),

    #[error("Strip prefix error: {0}")]
    StripPrefixError(#[from] std::path::StripPrefixError),
}
