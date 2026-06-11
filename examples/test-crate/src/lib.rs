#![allow(dead_code)]

pub mod util;

// Primitive FFI
#[koffi::export]
pub fn r#add_i32(a: i32, b: i32) -> i32 {
    a + b
}

#[koffi::export]
pub fn toggle_bool(val: bool) -> bool {
    !val
}

// String & Bytes FFI
#[koffi::export]
pub fn greet_user(name: &str) -> String {
    format!("Hello, {}!", name)
}

/// Documentation comment test
///
/// - This function reverses a byte array.
#[koffi::export]
pub fn reverse_byte_array(bytes: &[u8]) -> Vec<u8> {
    let mut reversed = bytes.to_vec();
    reversed.reverse();

    reversed
}

// Opaque Structs (Handles) FFI
#[koffi::opaque]
pub struct DatabaseConnection {
    url: String,
    open: bool,
}

#[koffi::export]
impl DatabaseConnection {
    pub fn open(url: &str) -> Self {
        DatabaseConnection {
            url: url.to_string(),
            open: true,
        }
    }

    pub fn get_url(&self) -> String {
        self.url.clone()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    // pub fn close(&mut self) {
    //     self.open = false;
    // }
}

// // Transparent Structs (Postcard Serialized) FFI
// #[koffi::data]
// #[derive(Clone, serde::Serialize, serde::Deserialize)]
// pub struct UserProfile {
//     pub id: u64,
//     pub username: String,
//     pub is_admin: bool,
// }

// #[koffi::export]
// pub fn create_profile(id: u64, username: &str, is_admin: bool) -> UserProfile
// {     UserProfile {
//         id,
//         username: username.to_string(),
//         is_admin,
//     }
// }

// #[koffi::export]
// pub fn format_profile(profile: UserProfile) -> String {
//     format!(
//         "User #{} ({}): admin={}",
//         profile.id, profile.username, profile.is_admin
//     )
// }
