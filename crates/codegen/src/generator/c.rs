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
    /// joined with `_`. The member is `ffi_member_name() + "Impl"` - the
    /// same lowerCamelCase names the expect object's functions use.
    #[must_use]
    pub fn jni_symbol(&self, schema: &Schema) -> String {
        schema.jni_symbol_for(&format!("{}Impl", self.ffi_member_name()))
    }
}

impl Schema {
    /// JNI symbol for a helper member on the `actual object` that isn't an
    /// exported fn (`koffiReadBytes`, `koffiFreeBytes`): same mangling as
    /// `SchemaFn::jni_symbol`, but the member name is given outright.
    #[must_use]
    pub fn jni_symbol_for(&self, member: &str) -> String {
        let package = self
            .functions
            .first()
            .and_then(|f| f.module_path.as_deref())
            .unwrap_or("")
            .replace("::", ".");
        format!(
            "Java_{}_{}_{}",
            jni_escape(&package),
            jni_escape(&self.ffi_object_name()),
            jni_escape(member),
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
