use facet::Facet;

#[derive(Facet)]
pub struct UserProfile {
    pub id: u32,
    pub active: bool,
}

#[koffi::export]
pub fn process_user(user: UserProfile, factor: u32) -> bool {
    user.active && factor > 0
}

// #[koffi::export]
// pub fn resolve(map: rustc_hash::FxHashMap<u8, String>) -> String {
//     map.into_iter()
//         .map(|(k, v)| format!("{}: {}", k, v))
//         .collect::<Vec<_>>()
//         .join(", ")
// }
