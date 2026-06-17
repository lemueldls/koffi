//! Parse context threaded through the recursive item visitor.

use std::{collections::HashMap, path::PathBuf};

use koffi_ir::{EnumInfo, FnInfo, StructInfo};

use crate::diagnostic::DiagnosticSink;

/// `true` -> opaque handle, `false` -> postcard-serializable data.
type TypeKind = bool;

/// `type_name -> is_opaque`. Built by sub-pass A.
pub type TypeDeclarationMap = HashMap<String, TypeKind>;

/// Mutable state carried through the recursive [`super::visitor::visit_items`] traversal.
///
/// # Stacks
///
/// Two parallel stacks are maintained:
///
/// ## Namespace stack (Kotlin package)
///
/// ```text
/// [crate default]            <- always present (index 0), from Cargo.toml
///   └─ [mod override]        <- pushed by #[koffi::namespace] on a mod item
///        └─ [item override]  <- applied per-item from ExportArgs.package
/// ```
///
/// ## Module stack (Rust path)
///
/// ```text
/// []                         <- empty = crate root
///   └─ ["camera"]            <- pushed when entering `pub mod camera`
///        └─ ["camera","ops"] <- pushed when entering `pub mod ops` inside camera
/// ```
///
/// `current_namespace()` returns the top of the namespace stack.
/// `current_module_path()` returns a snapshot of the module stack.
pub struct ParseContext {
    /// Active Kotlin namespace stack. Never empty; index 0 is the crate-level
    /// default read from `[package.metadata.koffi] namespace`.
    namespace_stack: Vec<String>,

    /// Active Rust module path stack. Empty at the crate root.
    /// Each entry is the name of a `mod` item being visited.
    module_stack: Vec<String>,

    /// Stack of file paths currently being parsed, used for error reporting.
    pub file_stack: Vec<PathBuf>,

    /// Accumulated struct declarations (both opaque and data).
    pub structs: Vec<StructInfo>,

    /// Accumulated enum declarations.
    pub enums: Vec<EnumInfo>,

    /// Accumulated function and method declarations.
    pub functions: Vec<FnInfo>,

    /// Diagnostic sink for collecting errors and warnings encountered during the
    /// Phase 1 traversal. Errors here cause the parse to fail after the full traversal
    /// completes; warnings are surfaced to the user but do not abort parsing.
    pub sink: DiagnosticSink,

    /// Type declaration map built by sub-pass A.
    /// Maps `type_name -> is_opaque`.
    /// Used by `parse_type` to correctly classify local user types.
    pub type_decls: TypeDeclarationMap,
}

impl ParseContext {
    /// Create a new context with `crate_namespace` as the base namespace.
    ///
    /// `type_decls` is the output of [`super::visitor::collect_type_declarations`],
    /// which must be run before the full parse begins.
    #[must_use]
    pub fn new(crate_namespace: String, type_decls: TypeDeclarationMap) -> Self {
        Self {
            namespace_stack: vec![crate_namespace],
            module_stack: Vec::new(),
            file_stack: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: Vec::new(),
            sink: DiagnosticSink::new(),
            type_decls,
        }
    }

    /// The namespace that applies to the current parse position.
    pub fn current_namespace(&self) -> &str {
        self.namespace_stack
            .last()
            .map(String::as_str)
            .unwrap_or("generated")
    }

    /// Push a namespace override onto the stack. Must be paired with
    /// [`pop_namespace`] after the subtree is processed.
    pub fn push_namespace(&mut self, ns: String) {
        self.namespace_stack.push(ns);
    }

    /// Pop the most recently pushed namespace override.
    ///
    /// Silently does nothing if only the crate-level default remains,
    /// preventing stack underflow from mismatched push/pop.
    pub fn pop_namespace(&mut self) {
        if self.namespace_stack.len() > 1 {
            self.namespace_stack.pop();
        }
    }

    /// Push a module name onto the Rust module path stack.
    /// Call when descending into a `mod foo { ... }` item.
    /// Must be paired with [`pop_module`].
    pub fn push_module(&mut self, name: String) {
        self.module_stack.push(name);
    }

    /// Pop the most recently entered module name.
    pub fn pop_module(&mut self) {
        self.module_stack.pop();
    }

    /// Return a snapshot of the current Rust module path.
    ///
    /// Returns an empty `Vec` at the crate root, or e.g. `["camera", "ops"]`
    /// when inside `pub mod camera { pub mod ops { ... } }`.
    #[must_use]
    pub fn current_module_path(&self) -> Vec<String> {
        self.module_stack.clone()
    }

    #[must_use]
    pub fn current_file(&self) -> Option<&PathBuf> {
        self.file_stack.last()
    }
}
