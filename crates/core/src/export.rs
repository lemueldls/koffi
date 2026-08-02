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
    /// inside one, method or receiver-less associated fn alike; `None` for
    /// a plain free function.
    ///
    /// A path- and symbol-naming concern only (see
    /// `SchemaFn::rust_absolute_path`/`c_abi_symbol`): it says nothing about
    /// whether the fn takes a `self` value. `Payload::new(data: u16) ->
    /// Self` has `parent: Some(Payload)` with no receiver at all; whether a
    /// param *is* the receiver lives on `FnShapeParam::is_receiver`.
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
    /// `Payload::new`.
    pub is_receiver: bool,
}
