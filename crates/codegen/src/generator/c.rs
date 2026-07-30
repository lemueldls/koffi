use crate::schema::{SchemaFn, SchemaParam};

impl SchemaFn {
    #[must_use]
    pub fn c_abi_symbol(&self) -> String {
        let rust_name = self.rust_name.trim_start_matches("r#");

        if let Some(recv) = &self.receiver {
            format!("__koffi_fn_{}_{rust_name}", recv.abi_ident_infix())
        } else {
            let mod_infix = self.module_path.as_deref().unwrap_or("").replace("::", "_");
            format!("__koffi_fn_{mod_infix}_{rust_name}")
        }
    }
}

impl SchemaParam {
    #[must_use]
    pub fn c_abi_symbol(&self) -> String {
        self.name.trim_start_matches("r#").to_string()
    }
}
