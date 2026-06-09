use std::{collections::HashMap, path::PathBuf};

/// Mutable parse state threaded through the recursive item visitor.
pub struct ParseContext {
    /// Stack of namespaces. The active namespace is always the last entry.
    /// Initialized with the crate-level namespace from Cargo.toml metadata.
    namespace_stack: Vec<String>,

    /// Stack of file paths being parsed (for error reporting).
    pub file_stack: Vec<PathBuf>,

    /// Accumulated output.
    pub structs: Vec<koffi_ir::StructInfo>,
    pub enums: Vec<koffi_ir::EnumInfo>,
    pub functions: Vec<koffi_ir::FnInfo>,
    pub type_decls: TypeDeclarationMap,
}

impl ParseContext {
    pub fn new(crate_namespace: String, type_decls: TypeDeclarationMap) -> Self {
        Self {
            namespace_stack: vec![crate_namespace],
            file_stack: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            functions: Vec::new(),
            type_decls,
        }
    }

    /// The namespace that applies to the current parse location.
    pub fn current_namespace(&self) -> &str {
        self.namespace_stack
            .last()
            .map_or("generated", String::as_str)
    }

    /// Push a namespace override, returning a guard that pops it on drop.
    pub fn push_namespace(&mut self, ns: String) {
        self.namespace_stack.push(ns);
    }

    pub fn pop_namespace(&mut self) {
        if self.namespace_stack.len() > 1 {
            self.namespace_stack.pop();
        }
    }
}

/// `true` -> opaque handle, `false` -> postcard-serializable data.
type TypeKind = bool;

/// `type_name -> is_opaque`. Built by sub-pass A.
pub type TypeDeclarationMap = HashMap<String, TypeKind>;
