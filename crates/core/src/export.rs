use facet::Shape;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TypeShapeRef {
    shape: &'static Shape,
}

impl TypeShapeRef {
    #[must_use]
    pub const fn from_shape(shape: &'static Shape) -> Self {
        Self { shape }
    }

    #[must_use]
    pub const fn shape(&self) -> &'static Shape {
        self.shape
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.shape.effective_name()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FnShapeRef {
    pub name: &'static str,
    pub params: &'static [FnShapeParam],
    pub return_type: TypeShapeRef,
    pub module_path: Option<&'static str>,
    /// The `Self` type of the enclosing `impl` block, for anything declared
    /// inside one, a method or a receiver-less associated fn alike. `None`
    /// for a plain free function.
    ///
    /// This is purely a path- and symbol-naming concern (see
    /// `SchemaFn::rust_absolute_path`/`c_abi_symbol` in koffi-codegen): it
    /// says nothing about whether the function actually takes a `self`
    /// value. `Payload::new(data: u16) -> Self` has `parent: Some(Payload)`
    /// with no receiver at all. Whether a given param *is* the receiver is
    /// tracked per-param on `FnShapeParam::is_receiver`, not here, the two
    /// used to be conflated (a `receiver` field that meant both "the parent
    /// type" and "params[0] is self"), which is exactly what made
    /// `Payload::new` impossible to represent correctly.
    pub parent: Option<TypeShapeRef>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FnShapeParam {
    pub name: &'static str,
    pub param_type: TypeShapeRef,
    /// True for exactly the synthetic `self` entry a method's macro-side
    /// expansion prepends to `params`, false for every real parameter,
    /// including every parameter of a `parent`-having associated fn like
    /// `Payload::new`, which has no receiver despite having a `parent`.
    pub is_receiver: bool,
}
