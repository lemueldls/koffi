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
    pub receiver: Option<TypeShapeRef>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FnShapeParam {
    pub name: &'static str,
    pub param_type: TypeShapeRef,
}
