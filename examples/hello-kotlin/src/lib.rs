use facet::Facet;

#[derive(Facet)]
pub struct Payload {
    pub data: u16,
}

#[koffi::export]
pub fn hello() -> Payload {
    Payload { data: 42 }
}
