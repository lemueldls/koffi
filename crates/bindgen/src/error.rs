use std::{io, path::Path};

use thiserror::Error;

use crate::diagnostic::{Diagnostic, Label, SourceSpan};

#[derive(Error, Debug)]
pub enum BindgenError {
    #[error("{0}")]
    Diagnostic(Box<Diagnostic>),

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

impl BindgenError {
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Diagnostic(diagnostic) => *diagnostic.clone(),
            Self::RustdocFailed(message) => Diagnostic::error("Rustdoc resolution failed")
                .with_note(message),
            Self::UnsupportedType(message) => Diagnostic::error("Unsupported type")
                .with_note(message)
                .with_help(
                    "Use FFI-safe primitives, String/&str, Vec<T>, &[u8], Option<T>, Result<T, E>, maps, sets, or #[koffi::data]/#[koffi::opaque] types.",
                ),
            Self::NoResolveGraph => Diagnostic::error("Cargo metadata did not include a resolve graph")
                .with_help("Run Koffi from a normal Cargo package with dependency resolution enabled."),
            Self::NoRootPackage => Diagnostic::error("Cargo metadata did not identify a root package"),
            Self::PackageNotFound(package) => Diagnostic::error("Package not found in Cargo metadata")
                .with_note(package),
            Self::CargoBuildFailed(target) => Diagnostic::error("Cargo build failed")
                .with_note(format!("target: {target}")),
            Self::EmptyTypePath => Diagnostic::error("Empty type path"),
            Self::ExpectedOneGeneric(parent) => Diagnostic::error("Invalid generic argument count")
                .with_note(format!("expected 1 generic argument for {parent}")),
            Self::ExpectedTwoGenerics(parent) => Diagnostic::error("Invalid generic argument count")
                .with_note(format!("expected 2 generic arguments for {parent}")),
            Self::CargoTomlError(message) => Diagnostic::error("Cargo.toml error")
                .with_note(message),
            Self::IoError(err) => Diagnostic::error("I/O error")
                .with_note(err.to_string()),
            Self::EnvError(err) => Diagnostic::error("Environment variable error")
                .with_note(err.to_string()),
            Self::SyntaxError(err) => Diagnostic::error("Rust syntax error")
                .with_note(err.to_string()),
            Self::CargoMetadataError(err) => Diagnostic::error("Cargo metadata error")
                .with_note(err.to_string()),
            Self::SerdeJsonError(err) => Diagnostic::error("JSON error")
                .with_note(err.to_string()),
            Self::FacetJsonSerializeError(err) => Diagnostic::error("Facet JSON serialization error")
                .with_note(err.to_string()),
            Self::TomlError(err) => Diagnostic::error("TOML parsing error")
                .with_note(err.to_string()),
            Self::TemplateError(err) => Diagnostic::error("Template rendering error")
                .with_note(err.to_string()),
            Self::WalkDirError(err) => Diagnostic::error("Directory walk error")
                .with_note(err.to_string()),
            Self::StripPrefixError(err) => Diagnostic::error("Path error")
                .with_note(err.to_string()),
        }
    }

    #[must_use]
    pub fn with_source_span(
        self,
        file: impl AsRef<Path>,
        span: SourceSpan,
        label: impl Into<String>,
    ) -> Self {
        let diagnostic = self.diagnostic().with_label(
            Label::primary(file.as_ref().to_path_buf(), span).with_message(label.into()),
        );

        Self::Diagnostic(Box::new(diagnostic))
    }

    #[must_use]
    pub fn from_diagnostic(diagnostic: Diagnostic) -> Self {
        Self::Diagnostic(Box::new(diagnostic))
    }
}
