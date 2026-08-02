use facet::Facet;

#[derive(Facet)]
pub struct Payload {
    pub data: u16,
}

#[koffi::export]
impl Payload {
    pub fn new(data: u16) -> Self {
        Self { data }
    }

    pub fn with_data(&self, data: u16) -> Self {
        Self { data }
    }

    pub fn describe(&self) -> u32 {
        self.data as u32
    }

    pub fn describe_format() -> u32 {
        1
    }
}

#[koffi::export]
pub fn hello() -> Payload {
    Payload { data: 42 }
}
