#![allow(unused, non_snake_case, clippy::all)]

#[repr(C)]
#[derive(Clone, Copy)]
pub struct __koffi_struct_hello_kotlin_UserProfile {
    pub id: u32,
    pub active: bool,
}

impl From<__koffi_struct_hello_kotlin_UserProfile> for ::hello_kotlin::UserProfile {
    fn from(c: __koffi_struct_hello_kotlin_UserProfile) -> Self {
        ::hello_kotlin::UserProfile {
            id: c.id.into(),
            active: c.active.into(),
        }
    }
}

impl From<::hello_kotlin::UserProfile> for __koffi_struct_hello_kotlin_UserProfile {
    fn from(c: ::hello_kotlin::UserProfile) -> Self {
        __koffi_struct_hello_kotlin_UserProfile {
            id: c.id.into(),
            active: c.active.into(),
        }
    }
}

/// `::hello_kotlin::process_user`
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __koffi_fn_hello_kotlin_process_user(
    _p_user: __koffi_struct_hello_kotlin_UserProfile,
    _p_factor: u32,
) -> bool {
    koffi::guarded(|| {
        let result = ::hello_kotlin::process_user(
            _p_user.into(),
            _p_factor.into(),
        );
        result
    })
}