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

#[derive(Facet)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Facet)]
pub struct Rect {
    pub top_left: Point,
    pub bottom_right: Point,
}

#[koffi::export]
pub fn make_rect() -> Rect {
    Rect {
        top_left: Point { x: 1, y: 2 },
        bottom_right: Point { x: 5, y: 8 },
    }
}

#[koffi::export]
pub fn rect_area(r: Rect) -> i32 {
    (r.bottom_right.x - r.top_left.x) * (r.bottom_right.y - r.top_left.y)
}

#[koffi::export]
pub fn rect_top_left(r: Rect) -> Point {
    r.top_left
}

#[koffi::export]
pub fn add_optional(a: u32, b: Option<u32>) -> u32 {
    a + b.unwrap_or(0)
}

#[koffi::export]
pub fn favorite() -> Option<u32> {
    Some(7)
}

#[koffi::export]
pub fn nothing() -> Option<u32> {
    None
}

#[koffi::export]
pub fn maybe_payload(show: bool) -> Option<Payload> {
    if show {
        Some(Payload { data: 11 })
    } else {
        None
    }
}

#[koffi::export]
pub fn paint(color: Option<bool>) -> bool {
    color.unwrap_or(false)
}

#[koffi::export]
pub fn how_long(seconds: Option<f64>) -> f64 {
    seconds.unwrap_or(1.5)
}

#[koffi::export]
pub fn divide(n: u32, d: u32) -> Result<u32, u8> {
    n.checked_div(d).ok_or(1)
}

#[koffi::export]
pub fn result_value(r: Result<u32, u8>) -> u32 {
    r.unwrap_or(0)
}

#[koffi::export]
pub fn nominate(x: u32) -> Result<Payload, Status> {
    if x < 100 {
        Ok(Payload { data: x as u16 })
    } else {
        Err(Status::Busy(x))
    }
}

#[koffi::export]
pub fn drink(x: u32) -> Result<Option<u32>, u8> {
    if x.is_multiple_of(2) {
        Ok(Some(x))
    } else {
        Ok(None)
    }
}

#[derive(Facet)]
pub struct Dancer {
    pub id: u32,
    pub active: Option<bool>,
}

#[koffi::export]
pub fn ride(dance: Option<Dancer>) -> Option<Dancer> {
    dance.map(|mut d| {
        d.id += 1;
        d
    })
}

#[derive(Facet)]
#[repr(i32)]
pub enum Mood {
    Fine = 0,
    Flying(Option<bool>) = 1,
}

#[koffi::export]
pub fn mood() -> Mood {
    Mood::Flying(Some(true))
}

#[koffi::export]
pub fn mood_is_flying(m: Mood) -> bool {
    matches!(m, Mood::Flying(Some(_)))
}

// Opaque handle: `#[facet(opaque)]` keeps the real layout (one `u64`,
// pointer-sized) but exposes no fields, so koffi marshals it as a Long
// address. Its methods have no data class to live on, so they stay on the
// generated Ffi object as `helloKotlinWindowDescribe`,
// `helloKotlinWindowRetag`, ...
#[derive(Facet)]
#[facet(opaque)]
pub struct Window {
    id: u64,
}

#[koffi::export]
impl Window {
    pub fn open(id: u64) -> Self {
        Self { id }
    }

    pub fn describe(&self) -> u64 {
        self.id
    }

    pub fn retag(&mut self, id: u64) -> u64 {
        self.id = id;
        self.id
    }
}

#[derive(Facet)]
pub struct WindowPair {
    pub a: Window,
    pub b: Window,
    pub tag: u8,
}

#[koffi::export]
impl WindowPair {
    pub fn new(a: u64, b: u64) -> Self {
        Self {
            a: Window::open(a),
            b: Window::open(b),
            tag: 7,
        }
    }

    pub fn first_describe(&self) -> u64 {
        self.a.describe()
    }

    pub fn retag_a(&mut self, id: u64) -> u64 {
        self.a.retag(id)
    }
}

#[koffi::export]
pub fn describe_window(w: Window) -> u64 {
    w.describe()
}

// Proxy: `SafePacket.secret` is really a `Secret`, but crosses the boundary
// as `SecretWire` thanks to `#[facet(proxy = ..)]` and the two TryFrom
// impls below. facet still needs the real field type to have a shape, so
// `Secret` derives Facet too - koffi never marshals it directly, the proxy
// wire type is what appears on the wire.
#[derive(Facet)]
pub struct Secret {
    bytes: [u8; 4],
}

#[derive(Facet)]
pub struct SecretWire {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
}

impl TryFrom<SecretWire> for Secret {
    type Error = String;

    fn try_from(w: SecretWire) -> Result<Self, Self::Error> {
        Ok(Self {
            bytes: [w.a, w.b, w.c, w.d],
        })
    }
}

impl TryFrom<&Secret> for SecretWire {
    type Error = String;

    fn try_from(s: &Secret) -> Result<Self, Self::Error> {
        Ok(Self {
            a: s.bytes[0],
            b: s.bytes[1],
            c: s.bytes[2],
            d: s.bytes[3],
        })
    }
}

#[derive(Facet)]
pub struct SafePacket {
    #[facet(proxy = SecretWire)]
    pub secret: Secret,
    pub hop: u8,
}

#[koffi::export]
pub fn encrypt(secret: SafePacket) -> SafePacket {
    let mut bytes = secret.secret.bytes;
    bytes.reverse();
    SafePacket {
        secret: Secret { bytes },
        hop: secret.hop + 1,
    }
}
