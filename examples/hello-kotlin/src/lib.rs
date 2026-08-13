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
    Loading(u32, u32) = 3,
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
            Status::Loading(..) => 4,
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
        Status::Loading(a, b) => a + b,
    }
}

#[koffi::export]
pub fn make_loading() -> Status {
    Status::Loading(11, 22)
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
// pointer-sized) but exposes no fields, so koffi marshals it as an address.
// The impl block's fns surface on a Kotlin handle class - `Window.open(42)`,
// `w.describe()`, `w.retag(...)` - instead of the raw Ffi object members.
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

// Spans: String/Vec<u8> cross as owned byte buffers, &str/&[u8] as
// borrows (params only). Structs hold owned spans; the owned/borrowed
// split only shapes the marshallers, never the Kotlin types.
#[derive(Facet)]
pub struct Greeting {
    pub name: String,
    pub blob: Vec<u8>,
}

#[derive(Facet)]
pub struct Mail {
    pub greeting: Greeting,
    pub hops: u32,
}

#[koffi::export]
pub fn greet(g: Greeting) -> String {
    g.name
}

#[koffi::export]
pub fn echo_bytes(b: Vec<u8>) -> Vec<u8> {
    b
}

#[koffi::export]
pub fn string_pair(a: String, b: &str) -> bool {
    a == b
}

#[koffi::export]
pub fn bytes_pair(a: Vec<u8>, b: &[u8]) -> bool {
    a == b
}

#[koffi::export]
pub fn span_length(s: String) -> u32 {
    s.len() as u32
}

#[koffi::export]
pub fn empty_string() -> String {
    String::new()
}

#[koffi::export]
pub fn take_string(s: String) -> bool {
    s == "koffi"
}

#[koffi::export]
pub fn upgrade(m: Mail) -> Mail {
    Mail {
        greeting: Greeting {
            name: format!("{}!", m.greeting.name),
            blob: m.greeting.blob,
        },
        hops: m.hops + 1,
    }
}

// Kotlin keyword collisions: `object`, `when` and `class` are valid Rust
// identifiers but hard Kotlin keywords, and raw `r#` idents surface the
// same way after the prefix is stripped. The generator backticks them on
// the Kotlin side; this block exercises every identifier position (fn
// names, params, struct fields, enum variants, type names) on all three
// backends.
#[derive(Facet)]
pub struct KeywordBag {
    pub object: u32,
    pub when: u32,
    pub class: u32,
    pub fun: u32,
}

#[koffi::export]
impl KeywordBag {
    pub fn new(object: u32, when: u32) -> Self {
        Self {
            object,
            when,
            class: 7,
            fun: 8,
        }
    }

    pub fn r#match(&self) -> u32 {
        self.object + self.when
    }
}

#[koffi::export]
pub fn r#in(obj: KeywordBag, r#when: u32) -> u32 {
    obj.object + r#when
}

#[koffi::export]
pub fn object() -> u32 {
    42
}

#[koffi::export]
pub fn class(a: u32, fun: u32) -> u32 {
    a + fun
}

#[koffi::export]
pub fn r#type(r#in: u32) -> u32 {
    r#in
}

#[derive(Facet)]
#[repr(i32)]
#[allow(non_camel_case_types)]
pub enum KeywordFlag {
    when = 0,
    val = 1,
    fun = 2,
}

#[koffi::export]
impl KeywordFlag {
    // Not a variant name too: the variant would shadow this fn for any
    // other crate (and the Kotlin enum entry would shadow the companion
    // fn), making the call unresolvable.
    pub fn object() -> Self {
        Self::when
    }
}

#[koffi::export]
pub fn flag_value(f: KeywordFlag) -> i32 {
    f as i32
}

#[derive(Facet)]
#[repr(i32)]
pub enum KeywordData {
    Plain = 0,
    Fancy { object: u32, when: u32, fun: u32 } = 1,
}

#[koffi::export]
pub fn fancy() -> KeywordData {
    KeywordData::Fancy {
        object: 1,
        when: 2,
        fun: 3,
    }
}

#[koffi::export]
pub fn keyword_data_sum(d: KeywordData) -> u32 {
    match d {
        KeywordData::Plain => 0,
        KeywordData::Fancy { object, when, fun } => object + when + fun,
    }
}

#[derive(Facet)]
#[allow(non_camel_case_types)]
pub struct when {
    pub object: u32,
    pub val: u32,
}

#[koffi::export]
pub fn make_when(object: u32, val: u32) -> when {
    when { object, val }
}

#[koffi::export]
pub fn when_sum(w: when) -> u32 {
    w.object + w.val
}

#[derive(Facet)]
#[facet(opaque)]
#[allow(non_camel_case_types)]
pub struct object {
    id: u64,
}

#[koffi::export]
impl object {
    pub fn open(id: u64) -> Self {
        Self { id }
    }

    pub fn describe(&self) -> u64 {
        self.id
    }
}

// Tuple structs: facet names the fields `"0"`, `"1"`, which the generator
// renames to `field0`, `field1` everywhere (C header, Kotlin properties,
// glue crate). The glue constructs the user type positionally.
#[derive(Facet)]
pub struct Pair(pub u32, pub u64);

#[koffi::export]
impl Pair {
    pub fn new(x: u32, y: u64) -> Self {
        Self(x, y)
    }

    pub fn swapped(&self) -> Self {
        Self(self.1 as u32, self.0 as u64)
    }
}

#[koffi::export]
pub fn make_pair() -> Pair {
    Pair(3, 7)
}

#[koffi::export]
pub fn pair_sum(p: Pair) -> u64 {
    p.0 as u64 + p.1
}

#[derive(Facet)]
pub struct Positioned {
    pub top: Pair,
    pub origin: Pair,
}

#[koffi::export]
pub fn positioned_sum(p: Positioned) -> u64 {
    pair_sum(p.top) + pair_sum(p.origin)
}
