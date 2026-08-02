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

#[derive(Facet)]
#[repr(i32)]
pub enum Status {
    Idle = 0,
    Busy(u32) = 1,
    Error { code: u32 } = 2,
    Failed = -1,
}

#[derive(Facet)]
#[repr(C)]
pub enum CStatus {
    Ok = 0,
    Err = -1,
}

#[koffi::export]
impl Status {
    pub fn describe(&self) -> u32 {
        match self {
            Status::Idle => 0,
            Status::Busy(_) => 1,
            Status::Error { code } => *code,
            Status::Failed => 2,
        }
    }

    pub fn idle() -> Self {
        Self::Idle
    }

    pub fn new_busy(code: u32) -> Self {
        Self::Busy(code)
    }
}

#[derive(Facet)]
pub struct StatusHolder {
    pub status: Status,
    pub tag: u8,
}

#[koffi::export]
pub fn hello() -> Payload {
    Payload { data: 42 }
}

#[koffi::export]
pub fn make_status() -> Status {
    Status::Busy(7)
}

#[koffi::export]
pub fn status_code(s: Status) -> u32 {
    match s {
        Status::Idle => 0,
        Status::Busy(code) => code,
        Status::Error { code } => code,
        Status::Failed => 3,
    }
}

#[koffi::export]
pub fn c_status() -> CStatus {
    CStatus::Err
}

#[koffi::export]
pub fn c_status_is_err(s: CStatus) -> bool {
    matches!(s, CStatus::Err)
}

#[koffi::export]
pub fn make_holder() -> StatusHolder {
    StatusHolder {
        status: Status::Error { code: 42 },
        tag: 9,
    }
}

#[koffi::export]
pub fn holder_status(h: StatusHolder) -> Status {
    h.status
}
