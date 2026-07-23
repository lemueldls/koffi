#![allow(dead_code)]

// pub mod util;

// Primitive FFI
#[koffi::export]
pub fn r#add_i32(a: i32, b: i32) -> i32 {
    a + b
}

// #[koffi::export]
// pub fn toggle_bool(val: bool) -> bool {
//     !val
// }

#[koffi::export]
pub fn maybe_float() -> Option<f64> {
    Some(42.0)
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

// Transparent Structs (Postcard Serialized) FFI
#[koffi::data]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct UserProfile {
    pub id: u64,
    pub username: String,
    pub is_admin: bool,
}

#[koffi::export]
pub fn create_profile(id: u64, username: &str, is_admin: bool) -> UserProfile {
    UserProfile {
        id,
        username: username.to_string(),
        is_admin,
    }
}

#[koffi::export]
pub fn format_profile(profile: UserProfile) -> String {
    format!(
        "User #{} ({}): admin={}",
        profile.id, profile.username, profile.is_admin
    )
}

#[koffi::data]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum UserRole {
    Admin,
    User,
    Guest(u32),
}

#[koffi::export]
pub fn role_to_string(role: UserRole) -> String {
    match role {
        UserRole::Admin => "admin".to_string(),
        UserRole::User => "user".to_string(),
        UserRole::Guest(id) => format!("guest_{}", id),
    }
}

#[koffi::export]
pub fn role_from_string(role_str: &str) -> UserRole {
    match role_str {
        "admin" => UserRole::Admin,
        "user" => UserRole::User,
        _ => {
            if let Ok(id) = role_str.strip_prefix("guest_").unwrap_or("").parse::<u32>() {
                UserRole::Guest(id)
            } else {
                panic!("Invalid role string: {}", role_str);
            }
        }
    }
}

// #[koffi::export]
// impl UserRole {
//     pub fn is_admin(&self) -> bool {
//         matches!(self, UserRole::Admin)
//     }
// }
