#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Null,
    Endpoint,
    TCB,
    VSpace,
    CNode,
    Untyped,
}

/// An opaque reference to a kernel object. 
/// Strictly prevents capabilities from holding raw memory pointers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ObjectRef(usize);

impl ObjectRef {
    pub const fn new(id: usize) -> Self {
        Self(id)
    }

    pub const fn null() -> Self {
        Self(0)
    }
}
