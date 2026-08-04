use crate::schema::{Schema, SchemaFn, SchemaParam};

impl SchemaFn {
    #[must_use]
    pub fn c_abi_symbol(&self) -> String {
        let rust_name = self.rust_name.trim_start_matches("r#");

        if let Some(parent) = &self.parent {
            format!("__koffi_fn_{}_{rust_name}", parent.abi_ident_infix())
        } else {
            let mod_infix = self.module_path.as_deref().unwrap_or("").replace("::", "_");
            format!("__koffi_fn_{mod_infix}_{rust_name}")
        }
    }

    /// JNI symbol for the `private external fun <member>Impl` helper on
    /// the Kotlin `actual object`. JNI name mangling: `.` in the package
    /// becomes `_`, every original `_` becomes `_1`, and the components are
    /// joined with `_`. The member is `ffi_member_name() + "Impl"` — the
    /// same lowerCamelCase names the expect object's functions use.
    #[must_use]
    pub fn jni_symbol(&self, schema: &Schema) -> String {
        let package = self.module_path.as_deref().unwrap_or("").replace("::", ".");
        format!(
            "Java_{}_{}_{}Impl",
            jni_escape(&package),
            jni_escape(&schema.ffi_object_name()),
            jni_escape(&self.ffi_member_name()),
        )
    }
}

fn jni_escape(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        match ch {
            '.' => out.push('_'),
            '_' => out.push_str("_1"),
            c => out.push(c),
        }
    }
    out
}

impl SchemaParam {
    #[must_use]
    pub fn c_abi_symbol(&self) -> String {
        self.name.trim_start_matches("r#").to_string()
    }
}
