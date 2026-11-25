pub const COLOR_NAMED_BLACK: u8 = 0;
pub const COLOR_NAMED_RED: u8 = 1;
pub const COLOR_NAMED_GREEN: u8 = 2;
pub const COLOR_NAMED_YELLOW: u8 = 3;
pub const COLOR_NAMED_BLUE: u8 = 4;
pub const COLOR_NAMED_MAGENTA: u8 = 5;
pub const COLOR_NAMED_CYAN: u8 = 6;
pub const COLOR_NAMED_WHITE: u8 = 7;
pub const COLOR_NAMED_BRIGHT_BLACK: u8 = 8;
pub const COLOR_NAMED_BRIGHT_RED: u8 = 9;
pub const COLOR_NAMED_BRIGHT_GREEN: u8 = 10;
pub const COLOR_NAMED_BRIGHT_YELLOW: u8 = 11;
pub const COLOR_NAMED_BRIGHT_BLUE: u8 = 12;
pub const COLOR_NAMED_BRIGHT_MAGENTA: u8 = 13;
pub const COLOR_NAMED_BRIGHT_CYAN: u8 = 14;
pub const COLOR_NAMED_BRIGHT_WHITE: u8 = 15;
pub const DA_CONFORMANCE_VT100: u16 = 1;
pub const DA_CONFORMANCE_VT101: u16 = 1;
pub const DA_CONFORMANCE_VT102: u16 = 6;
pub const DA_CONFORMANCE_VT125: u16 = 12;
pub const DA_CONFORMANCE_VT131: u16 = 7;
pub const DA_CONFORMANCE_VT132: u16 = 4;
pub const DA_CONFORMANCE_VT220: u16 = 62;
pub const DA_CONFORMANCE_VT240: u16 = 62;
pub const DA_CONFORMANCE_VT320: u16 = 63;
pub const DA_CONFORMANCE_VT340: u16 = 63;
pub const DA_CONFORMANCE_VT420: u16 = 64;
pub const DA_CONFORMANCE_VT510: u16 = 65;
pub const DA_CONFORMANCE_VT520: u16 = 65;
pub const DA_CONFORMANCE_VT525: u16 = 65;
pub const DA_CONFORMANCE_LEVEL_2: u16 = 62;
pub const DA_CONFORMANCE_LEVEL_3: u16 = 63;
pub const DA_CONFORMANCE_LEVEL_4: u16 = 64;
pub const DA_CONFORMANCE_LEVEL_5: u16 = 65;
pub const DA_FEATURE_COLUMNS_132: u16 = 1;
pub const DA_FEATURE_PRINTER: u16 = 2;
pub const DA_FEATURE_REGIS: u16 = 3;
pub const DA_FEATURE_SIXEL: u16 = 4;
pub const DA_FEATURE_SELECTIVE_ERASE: u16 = 6;
pub const DA_FEATURE_USER_DEFINED_KEYS: u16 = 8;
pub const DA_FEATURE_NATIONAL_REPLACEMENT: u16 = 9;
pub const DA_FEATURE_TECHNICAL_CHARACTERS: u16 = 15;
pub const DA_FEATURE_LOCATOR: u16 = 16;
pub const DA_FEATURE_TERMINAL_STATE: u16 = 17;
pub const DA_FEATURE_WINDOWING: u16 = 18;
pub const DA_FEATURE_HORIZONTAL_SCROLLING: u16 = 21;
pub const DA_FEATURE_ANSI_COLOR: u16 = 22;
pub const DA_FEATURE_RECTANGULAR_EDITING: u16 = 28;
pub const DA_FEATURE_ANSI_TEXT_LOCATOR: u16 = 29;
pub const DA_FEATURE_CLIPBOARD: u16 = 52;
pub const DA_DEVICE_TYPE_VT100: u16 = 0;
pub const DA_DEVICE_TYPE_VT220: u16 = 1;
pub const DA_DEVICE_TYPE_VT240: u16 = 2;
pub const DA_DEVICE_TYPE_VT330: u16 = 18;
pub const DA_DEVICE_TYPE_VT340: u16 = 19;
pub const DA_DEVICE_TYPE_VT320: u16 = 24;
pub const DA_DEVICE_TYPE_VT382: u16 = 32;
pub const DA_DEVICE_TYPE_VT420: u16 = 41;
pub const DA_DEVICE_TYPE_VT510: u16 = 61;
pub const DA_DEVICE_TYPE_VT520: u16 = 64;
pub const DA_DEVICE_TYPE_VT525: u16 = 65;
pub const MODS_SHIFT: u16 = 1;
pub const MODS_CTRL: u16 = 2;
pub const MODS_ALT: u16 = 4;
pub const MODS_SUPER: u16 = 8;
pub const MODS_CAPS_LOCK: u16 = 16;
pub const MODS_NUM_LOCK: u16 = 32;
pub const MODS_SHIFT_SIDE: u16 = 64;
pub const MODS_CTRL_SIDE: u16 = 128;
pub const MODS_ALT_SIDE: u16 = 256;
pub const MODS_SUPER_SIDE: u16 = 512;
pub const KITTY_KEY_DISABLED: u8 = 0;
pub const KITTY_KEY_DISAMBIGUATE: u8 = 1;
pub const KITTY_KEY_REPORT_EVENTS: u8 = 2;
pub const KITTY_KEY_REPORT_ALTERNATES: u8 = 4;
pub const KITTY_KEY_REPORT_ALL: u8 = 8;
pub const KITTY_KEY_REPORT_ASSOCIATED: u8 = 16;
pub const KITTY_KEY_ALL: u8 = 31;
pub mod Result {
    pub type Type = ::std::os::raw::c_int;
    pub const SUCCESS: Type = 0;
    pub const OUT_OF_MEMORY: Type = -1;
    pub const INVALID_VALUE: Type = -2;
    pub const OUT_OF_SPACE: Type = -3;
    pub const NO_VALUE: Type = -4;
    pub const IO_ERROR: Type = -5;
    pub const LIMIT_EXCEEDED: Type = -6;
    pub const REJECTED: Type = -7;
    pub const RESULT_MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TerminalImpl {
    _unused: [u8; 0],
}
pub type Terminal = *mut TerminalImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SnapshotDecoderImpl {
    _unused: [u8; 0],
}
pub type SnapshotDecoder = *mut SnapshotDecoderImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TrackedGridRefImpl {
    _unused: [u8; 0],
}
pub type TrackedGridRef = *mut TrackedGridRefImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct KittyGraphicsImpl {
    _unused: [u8; 0],
}
pub type KittyGraphics = *mut KittyGraphicsImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct KittyGraphicsImageImpl {
    _unused: [u8; 0],
}
pub type KittyGraphicsImage = *const KittyGraphicsImageImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct KittyGraphicsPlacementIteratorImpl {
    _unused: [u8; 0],
}
pub type KittyGraphicsPlacementIterator = *mut KittyGraphicsPlacementIteratorImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct RenderStateImpl {
    _unused: [u8; 0],
}
pub type RenderState = *mut RenderStateImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct RenderStateRowIteratorImpl {
    _unused: [u8; 0],
}
pub type RenderStateRowIterator = *mut RenderStateRowIteratorImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct RenderStateRowCellsImpl {
    _unused: [u8; 0],
}
pub type RenderStateRowCells = *mut RenderStateRowCellsImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct RenderStateImageIteratorImpl {
    _unused: [u8; 0],
}
pub type RenderStateImageIterator = *mut RenderStateImageIteratorImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SearchImpl {
    _unused: [u8; 0],
}
pub type Search = *mut SearchImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SgrParserImpl {
    _unused: [u8; 0],
}
pub type SgrParser = *mut SgrParserImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct FormatterImpl {
    _unused: [u8; 0],
}
pub type Formatter = *mut FormatterImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct OscParserImpl {
    _unused: [u8; 0],
}
pub type OscParser = *mut OscParserImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct OscCommandImpl {
    _unused: [u8; 0],
}
pub type OscCommand = *mut OscCommandImpl;
pub mod FormatterFormat {
    pub type Type = ::std::os::raw::c_int;
    pub const PLAIN: Type = 0;
    pub const VT: Type = 1;
    pub const HTML: Type = 2;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct String {
    pub ptr: *const u8,
    pub len: usize,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of String"][::std::mem::size_of::<String>() - 16usize];
    ["Alignment of String"][::std::mem::align_of::<String>() - 8usize];
    ["Offset of field: String::ptr"][::std::mem::offset_of!(String, ptr) - 0usize];
    ["Offset of field: String::len"][::std::mem::offset_of!(String, len) - 8usize];
};
impl Default for String {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Buffer {
    pub ptr: *mut u8,
    pub cap: usize,
    pub len: usize,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of Buffer"][::std::mem::size_of::<Buffer>() - 24usize];
    ["Alignment of Buffer"][::std::mem::align_of::<Buffer>() - 8usize];
    ["Offset of field: Buffer::ptr"][::std::mem::offset_of!(Buffer, ptr) - 0usize];
    ["Offset of field: Buffer::cap"][::std::mem::offset_of!(Buffer, cap) - 8usize];
    ["Offset of field: Buffer::len"][::std::mem::offset_of!(Buffer, len) - 16usize];
};
impl Default for Buffer {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct SurfacePosition {
    pub x: f64,
    pub y: f64,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of SurfacePosition"][::std::mem::size_of::<SurfacePosition>() - 16usize];
    ["Alignment of SurfacePosition"][::std::mem::align_of::<SurfacePosition>() - 8usize];
    ["Offset of field: SurfacePosition::x"][::std::mem::offset_of!(SurfacePosition, x) - 0usize];
    ["Offset of field: SurfacePosition::y"][::std::mem::offset_of!(SurfacePosition, y) - 8usize];
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Codepoints {
    pub ptr: *const u32,
    pub len: usize,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of Codepoints"][::std::mem::size_of::<Codepoints>() - 16usize];
    ["Alignment of Codepoints"][::std::mem::align_of::<Codepoints>() - 8usize];
    ["Offset of field: Codepoints::ptr"][::std::mem::offset_of!(Codepoints, ptr) - 0usize];
    ["Offset of field: Codepoints::len"][::std::mem::offset_of!(Codepoints, len) - 8usize];
};
impl Default for Codepoints {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
unsafe extern "C" {
    pub fn ghostty_type_json() -> *const ::std::os::raw::c_char;
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct AllocatorVtable {
    pub alloc: ::std::option::Option<
        unsafe extern "C" fn(
            ctx: *mut ::std::os::raw::c_void,
            len: usize,
            alignment: u8,
            ret_addr: usize,
        ) -> *mut ::std::os::raw::c_void,
    >,
    pub resize: ::std::option::Option<
        unsafe extern "C" fn(
            ctx: *mut ::std::os::raw::c_void,
            memory: *mut ::std::os::raw::c_void,
            memory_len: usize,
            alignment: u8,
            new_len: usize,
            ret_addr: usize,
        ) -> bool,
    >,
    pub remap: ::std::option::Option<
        unsafe extern "C" fn(
            ctx: *mut ::std::os::raw::c_void,
            memory: *mut ::std::os::raw::c_void,
            memory_len: usize,
            alignment: u8,
            new_len: usize,
            ret_addr: usize,
        ) -> *mut ::std::os::raw::c_void,
    >,
    pub free: ::std::option::Option<
        unsafe extern "C" fn(
            ctx: *mut ::std::os::raw::c_void,
            memory: *mut ::std::os::raw::c_void,
            memory_len: usize,
            alignment: u8,
            ret_addr: usize,
        ),
    >,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of AllocatorVtable"][::std::mem::size_of::<AllocatorVtable>() - 32usize];
    ["Alignment of AllocatorVtable"][::std::mem::align_of::<AllocatorVtable>() - 8usize];
    ["Offset of field: AllocatorVtable::alloc"]
        [::std::mem::offset_of!(AllocatorVtable, alloc) - 0usize];
    ["Offset of field: AllocatorVtable::resize"]
        [::std::mem::offset_of!(AllocatorVtable, resize) - 8usize];
    ["Offset of field: AllocatorVtable::remap"]
        [::std::mem::offset_of!(AllocatorVtable, remap) - 16usize];
    ["Offset of field: AllocatorVtable::free"]
        [::std::mem::offset_of!(AllocatorVtable, free) - 24usize];
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Allocator {
    pub ctx: *mut ::std::os::raw::c_void,
    pub vtable: *const AllocatorVtable,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of Allocator"][::std::mem::size_of::<Allocator>() - 16usize];
    ["Alignment of Allocator"][::std::mem::align_of::<Allocator>() - 8usize];
    ["Offset of field: Allocator::ctx"][::std::mem::offset_of!(Allocator, ctx) - 0usize];
    ["Offset of field: Allocator::vtable"][::std::mem::offset_of!(Allocator, vtable) - 8usize];
};
impl Default for Allocator {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
unsafe extern "C" {
    pub fn ghostty_alloc(allocator: *const Allocator, len: usize) -> *mut u8;
}
unsafe extern "C" {
    pub fn ghostty_free(allocator: *const Allocator, ptr: *mut u8, len: usize);
}
pub mod OptimizeMode {
    pub type Type = ::std::os::raw::c_int;
    pub const DEBUG: Type = 0;
    pub const RELEASE_SAFE: Type = 1;
    pub const RELEASE_SMALL: Type = 2;
    pub const RELEASE_FAST: Type = 3;
    pub const MODE_MAX_VALUE: Type = 2147483647;
}
pub mod BuildInfo {
    pub type Type = ::std::os::raw::c_int;
    pub const INVALID: Type = 0;
    pub const SIMD: Type = 1;
    pub const KITTY_GRAPHICS: Type = 2;
    pub const TMUX_CONTROL_MODE: Type = 3;
    pub const OPTIMIZE: Type = 4;
    pub const VERSION_STRING: Type = 5;
    pub const VERSION_MAJOR: Type = 6;
    pub const VERSION_MINOR: Type = 7;
    pub const VERSION_PATCH: Type = 8;
    pub const VERSION_PRE: Type = 9;
    pub const VERSION_BUILD: Type = 10;
    pub const MAX_VALUE: Type = 2147483647;
}
unsafe extern "C" {
    pub fn ghostty_build_info(
        data: BuildInfo::Type,
        out: *mut ::std::os::raw::c_void,
    ) -> Result::Type;
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct ColorRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of ColorRgb"][::std::mem::size_of::<ColorRgb>() - 3usize];
    ["Alignment of ColorRgb"][::std::mem::align_of::<ColorRgb>() - 1usize];
    ["Offset of field: ColorRgb::r"][::std::mem::offset_of!(ColorRgb, r) - 0usize];
    ["Offset of field: ColorRgb::g"][::std::mem::offset_of!(ColorRgb, g) - 1usize];
    ["Offset of field: ColorRgb::b"][::std::mem::offset_of!(ColorRgb, b) - 2usize];
};
pub type ColorPaletteIndex = u8;
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct ColorPaletteMask {
    pub bits: [u64; 4usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of ColorPaletteMask"][::std::mem::size_of::<ColorPaletteMask>() - 32usize];
    ["Alignment of ColorPaletteMask"][::std::mem::align_of::<ColorPaletteMask>() - 8usize];
    ["Offset of field: ColorPaletteMask::bits"]
        [::std::mem::offset_of!(ColorPaletteMask, bits) - 0usize];
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ColorX11Entry {
    pub name: *const ::std::os::raw::c_char,
    pub color: ColorRgb,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of ColorX11Entry"][::std::mem::size_of::<ColorX11Entry>() - 16usize];
    ["Alignment of ColorX11Entry"][::std::mem::align_of::<ColorX11Entry>() - 8usize];
    ["Offset of field: ColorX11Entry::name"][::std::mem::offset_of!(ColorX11Entry, name) - 0usize];
    ["Offset of field: ColorX11Entry::color"]
        [::std::mem::offset_of!(ColorX11Entry, color) - 8usize];
};
impl Default for ColorX11Entry {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
unsafe extern "C" {
    pub fn ghostty_color_rgb_get(color: *const ColorRgb, r: *mut u8, g: *mut u8, b: *mut u8);
}
unsafe extern "C" {
    pub fn ghostty_color_parse_x11(
        name: *const ::std::os::raw::c_char,
        len: usize,
        out: *mut ColorRgb,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_color_parse(
        value: *const ::std::os::raw::c_char,
        len: usize,
        out: *mut ColorRgb,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_color_parse_palette_entry(
        value: *const ::std::os::raw::c_char,
        len: usize,
        out_index: *mut u8,
        out_rgb: *mut ColorRgb,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_color_palette_default(out: *mut ColorRgb);
}
unsafe extern "C" {
    pub fn ghostty_color_palette_generate(
        base: *const ColorRgb,
        skip: *const ColorPaletteMask,
        bg: *const ColorRgb,
        fg: *const ColorRgb,
        harmonious: bool,
        out: *mut ColorRgb,
    );
}
unsafe extern "C" {
    pub fn ghostty_color_luminance(color: *const ColorRgb) -> f64;
}
unsafe extern "C" {
    pub fn ghostty_color_perceived_luminance(color: *const ColorRgb) -> f64;
}
unsafe extern "C" {
    pub fn ghostty_color_contrast(a: *const ColorRgb, b: *const ColorRgb) -> f64;
}
unsafe extern "C" {
    pub fn ghostty_color_x11_names() -> *const ColorX11Entry;
}
unsafe extern "C" {
    pub fn ghostty_color_x11_name_count() -> usize;
}
pub mod ColorScheme {
    pub type Type = ::std::os::raw::c_int;
    pub const LIGHT: Type = 0;
    pub const DARK: Type = 1;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct DeviceAttributesPrimary {
    pub conformance_level: u16,
    pub features: [u16; 64usize],
    pub num_features: usize,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of DeviceAttributesPrimary"]
        [::std::mem::size_of::<DeviceAttributesPrimary>() - 144usize];
    ["Alignment of DeviceAttributesPrimary"]
        [::std::mem::align_of::<DeviceAttributesPrimary>() - 8usize];
    ["Offset of field: DeviceAttributesPrimary::conformance_level"]
        [::std::mem::offset_of!(DeviceAttributesPrimary, conformance_level) - 0usize];
    ["Offset of field: DeviceAttributesPrimary::features"]
        [::std::mem::offset_of!(DeviceAttributesPrimary, features) - 2usize];
    ["Offset of field: DeviceAttributesPrimary::num_features"]
        [::std::mem::offset_of!(DeviceAttributesPrimary, num_features) - 136usize];
};
impl Default for DeviceAttributesPrimary {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct DeviceAttributesSecondary {
    pub device_type: u16,
    pub firmware_version: u16,
    pub rom_cartridge: u16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of DeviceAttributesSecondary"]
        [::std::mem::size_of::<DeviceAttributesSecondary>() - 6usize];
    ["Alignment of DeviceAttributesSecondary"]
        [::std::mem::align_of::<DeviceAttributesSecondary>() - 2usize];
    ["Offset of field: DeviceAttributesSecondary::device_type"]
        [::std::mem::offset_of!(DeviceAttributesSecondary, device_type) - 0usize];
    ["Offset of field: DeviceAttributesSecondary::firmware_version"]
        [::std::mem::offset_of!(DeviceAttributesSecondary, firmware_version) - 2usize];
    ["Offset of field: DeviceAttributesSecondary::rom_cartridge"]
        [::std::mem::offset_of!(DeviceAttributesSecondary, rom_cartridge) - 4usize];
};
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct DeviceAttributesTertiary {
    pub unit_id: u32,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of DeviceAttributesTertiary"]
        [::std::mem::size_of::<DeviceAttributesTertiary>() - 4usize];
    ["Alignment of DeviceAttributesTertiary"]
        [::std::mem::align_of::<DeviceAttributesTertiary>() - 4usize];
    ["Offset of field: DeviceAttributesTertiary::unit_id"]
        [::std::mem::offset_of!(DeviceAttributesTertiary, unit_id) - 0usize];
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct DeviceAttributes {
    pub primary: DeviceAttributesPrimary,
    pub secondary: DeviceAttributesSecondary,
    pub tertiary: DeviceAttributesTertiary,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of DeviceAttributes"][::std::mem::size_of::<DeviceAttributes>() - 160usize];
    ["Alignment of DeviceAttributes"][::std::mem::align_of::<DeviceAttributes>() - 8usize];
    ["Offset of field: DeviceAttributes::primary"]
        [::std::mem::offset_of!(DeviceAttributes, primary) - 0usize];
    ["Offset of field: DeviceAttributes::secondary"]
        [::std::mem::offset_of!(DeviceAttributes, secondary) - 144usize];
    ["Offset of field: DeviceAttributes::tertiary"]
        [::std::mem::offset_of!(DeviceAttributes, tertiary) - 152usize];
};
impl Default for DeviceAttributes {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
unsafe extern "C" {
    pub fn ghostty_color_scheme_report_encode(
        scheme: ColorScheme::Type,
        buf: *mut ::std::os::raw::c_char,
        buf_len: usize,
        out_written: *mut usize,
    ) -> Result::Type;
}
pub mod FocusEvent {
    pub type Type = ::std::os::raw::c_int;
    pub const GAINED: Type = 0;
    pub const LOST: Type = 1;
    pub const MAX_VALUE: Type = 2147483647;
}
unsafe extern "C" {
    pub fn ghostty_focus_encode(
        event: FocusEvent::Type,
        buf: *mut ::std::os::raw::c_char,
        buf_len: usize,
        out_written: *mut usize,
    ) -> Result::Type;
}
pub type ReaderFn = ::std::option::Option<
    unsafe extern "C" fn(
        userdata: *mut ::std::os::raw::c_void,
        buffer: *mut u8,
        capacity: usize,
        out_read: *mut usize,
    ) -> bool,
>;
pub type WriterFn = ::std::option::Option<
    unsafe extern "C" fn(
        userdata: *mut ::std::os::raw::c_void,
        data: *const u8,
        len: usize,
    ) -> bool,
>;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Reader {
    pub read: ReaderFn,
    pub userdata: *mut ::std::os::raw::c_void,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of Reader"][::std::mem::size_of::<Reader>() - 16usize];
    ["Alignment of Reader"][::std::mem::align_of::<Reader>() - 8usize];
    ["Offset of field: Reader::read"][::std::mem::offset_of!(Reader, read) - 0usize];
    ["Offset of field: Reader::userdata"][::std::mem::offset_of!(Reader, userdata) - 8usize];
};
impl Default for Reader {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Writer {
    pub write: WriterFn,
    pub userdata: *mut ::std::os::raw::c_void,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of Writer"][::std::mem::size_of::<Writer>() - 16usize];
    ["Alignment of Writer"][::std::mem::align_of::<Writer>() - 8usize];
    ["Offset of field: Writer::write"][::std::mem::offset_of!(Writer, write) - 0usize];
    ["Offset of field: Writer::userdata"][::std::mem::offset_of!(Writer, userdata) - 8usize];
};
impl Default for Writer {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub type MimeReaderFn = ::std::option::Option<
    unsafe extern "C" fn(
        userdata: *mut ::std::os::raw::c_void,
        mime: String,
        writer: Writer,
    ) -> bool,
>;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MimeReader {
    pub read: MimeReaderFn,
    pub userdata: *mut ::std::os::raw::c_void,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of MimeReader"][::std::mem::size_of::<MimeReader>() - 16usize];
    ["Alignment of MimeReader"][::std::mem::align_of::<MimeReader>() - 8usize];
    ["Offset of field: MimeReader::read"][::std::mem::offset_of!(MimeReader, read) - 0usize];
    ["Offset of field: MimeReader::userdata"]
        [::std::mem::offset_of!(MimeReader, userdata) - 8usize];
};
impl Default for MimeReader {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub type Cell = u64;
pub type Row = u64;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct CellsView {
    pub ptr: *const Cell,
    pub len: usize,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of CellsView"][::std::mem::size_of::<CellsView>() - 16usize];
    ["Alignment of CellsView"][::std::mem::align_of::<CellsView>() - 8usize];
    ["Offset of field: CellsView::ptr"][::std::mem::offset_of!(CellsView, ptr) - 0usize];
    ["Offset of field: CellsView::len"][::std::mem::offset_of!(CellsView, len) - 8usize];
};
impl Default for CellsView {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub mod CellContentTag {
    pub type Type = ::std::os::raw::c_int;
    pub const CODEPOINT: Type = 0;
    pub const CODEPOINT_GRAPHEME: Type = 1;
    pub const BG_COLOR_PALETTE: Type = 2;
    pub const BG_COLOR_RGB: Type = 3;
    pub const TAG_MAX_VALUE: Type = 2147483647;
}
pub mod CellWide {
    pub type Type = ::std::os::raw::c_int;
    pub const NARROW: Type = 0;
    pub const WIDE: Type = 1;
    pub const SPACER_TAIL: Type = 2;
    pub const SPACER_HEAD: Type = 3;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod CellSemanticContent {
    pub type Type = ::std::os::raw::c_int;
    pub const OUTPUT: Type = 0;
    pub const INPUT: Type = 1;
    pub const PROMPT: Type = 2;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod CellData {
    pub type Type = ::std::os::raw::c_int;
    pub const INVALID: Type = 0;
    pub const CODEPOINT: Type = 1;
    pub const CONTENT_TAG: Type = 2;
    pub const WIDE: Type = 3;
    pub const HAS_TEXT: Type = 4;
    pub const HAS_STYLING: Type = 5;
    pub const STYLE_ID: Type = 6;
    pub const HAS_HYPERLINK: Type = 7;
    pub const PROTECTED: Type = 8;
    pub const SEMANTIC_CONTENT: Type = 9;
    pub const COLOR_PALETTE: Type = 10;
    pub const COLOR_RGB: Type = 11;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod RowSemanticPrompt {
    pub type Type = ::std::os::raw::c_int;
    pub const NONE: Type = 0;
    pub const PROMPT: Type = 1;
    pub const PROMPT_CONTINUATION: Type = 2;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod RowData {
    pub type Type = ::std::os::raw::c_int;
    pub const INVALID: Type = 0;
    pub const WRAP: Type = 1;
    pub const WRAP_CONTINUATION: Type = 2;
    pub const GRAPHEME: Type = 3;
    pub const STYLED: Type = 4;
    pub const HYPERLINK: Type = 5;
    pub const SEMANTIC_PROMPT: Type = 6;
    pub const KITTY_VIRTUAL_PLACEHOLDER: Type = 7;
    pub const DIRTY: Type = 8;
    pub const MAX_VALUE: Type = 2147483647;
}
unsafe extern "C" {
    pub fn ghostty_cell_get(
        cell: Cell,
        data: CellData::Type,
        out: *mut ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_cell_get_multi(
        cell: Cell,
        count: usize,
        keys: *const CellData::Type,
        values: *mut *mut ::std::os::raw::c_void,
        out_written: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_row_get(
        row: Row,
        data: RowData::Type,
        out: *mut ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_row_get_multi(
        row: Row,
        count: usize,
        keys: *const RowData::Type,
        values: *mut *mut ::std::os::raw::c_void,
        out_written: *mut usize,
    ) -> Result::Type;
}
pub type StyleId = u16;
pub mod StyleColorTag {
    pub type Type = ::std::os::raw::c_int;
    pub const NONE: Type = 0;
    pub const PALETTE: Type = 1;
    pub const RGB: Type = 2;
    pub const TAG_MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union StyleColorValue {
    pub palette: ColorPaletteIndex,
    pub rgb: ColorRgb,
    pub _padding: u64,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of StyleColorValue"][::std::mem::size_of::<StyleColorValue>() - 8usize];
    ["Alignment of StyleColorValue"][::std::mem::align_of::<StyleColorValue>() - 8usize];
    ["Offset of field: StyleColorValue::palette"]
        [::std::mem::offset_of!(StyleColorValue, palette) - 0usize];
    ["Offset of field: StyleColorValue::rgb"]
        [::std::mem::offset_of!(StyleColorValue, rgb) - 0usize];
    ["Offset of field: StyleColorValue::_padding"]
        [::std::mem::offset_of!(StyleColorValue, _padding) - 0usize];
};
impl Default for StyleColorValue {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Copy, Clone)]
pub struct StyleColor {
    pub tag: StyleColorTag::Type,
    pub value: StyleColorValue,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of StyleColor"][::std::mem::size_of::<StyleColor>() - 16usize];
    ["Alignment of StyleColor"][::std::mem::align_of::<StyleColor>() - 8usize];
    ["Offset of field: StyleColor::tag"][::std::mem::offset_of!(StyleColor, tag) - 0usize];
    ["Offset of field: StyleColor::value"][::std::mem::offset_of!(StyleColor, value) - 8usize];
};
impl Default for StyleColor {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Style {
    pub size: usize,
    pub fg_color: StyleColor,
    pub bg_color: StyleColor,
    pub underline_color: StyleColor,
    pub bold: bool,
    pub italic: bool,
    pub faint: bool,
    pub blink: bool,
    pub inverse: bool,
    pub invisible: bool,
    pub strikethrough: bool,
    pub overline: bool,
    pub underline: ::std::os::raw::c_int,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of Style"][::std::mem::size_of::<Style>() - 72usize];
    ["Alignment of Style"][::std::mem::align_of::<Style>() - 8usize];
    ["Offset of field: Style::size"][::std::mem::offset_of!(Style, size) - 0usize];
    ["Offset of field: Style::fg_color"][::std::mem::offset_of!(Style, fg_color) - 8usize];
    ["Offset of field: Style::bg_color"][::std::mem::offset_of!(Style, bg_color) - 24usize];
    ["Offset of field: Style::underline_color"]
        [::std::mem::offset_of!(Style, underline_color) - 40usize];
    ["Offset of field: Style::bold"][::std::mem::offset_of!(Style, bold) - 56usize];
    ["Offset of field: Style::italic"][::std::mem::offset_of!(Style, italic) - 57usize];
    ["Offset of field: Style::faint"][::std::mem::offset_of!(Style, faint) - 58usize];
    ["Offset of field: Style::blink"][::std::mem::offset_of!(Style, blink) - 59usize];
    ["Offset of field: Style::inverse"][::std::mem::offset_of!(Style, inverse) - 60usize];
    ["Offset of field: Style::invisible"][::std::mem::offset_of!(Style, invisible) - 61usize];
    ["Offset of field: Style::strikethrough"]
        [::std::mem::offset_of!(Style, strikethrough) - 62usize];
    ["Offset of field: Style::overline"][::std::mem::offset_of!(Style, overline) - 63usize];
    ["Offset of field: Style::underline"][::std::mem::offset_of!(Style, underline) - 64usize];
};
impl Default for Style {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
unsafe extern "C" {
    pub fn ghostty_style_default(style: *mut Style);
}
unsafe extern "C" {
    pub fn ghostty_style_is_default(style: *const Style) -> bool;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct GridRef {
    pub size: usize,
    pub node: *mut ::std::os::raw::c_void,
    pub x: u16,
    pub y: u16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of GridRef"][::std::mem::size_of::<GridRef>() - 24usize];
    ["Alignment of GridRef"][::std::mem::align_of::<GridRef>() - 8usize];
    ["Offset of field: GridRef::size"][::std::mem::offset_of!(GridRef, size) - 0usize];
    ["Offset of field: GridRef::node"][::std::mem::offset_of!(GridRef, node) - 8usize];
    ["Offset of field: GridRef::x"][::std::mem::offset_of!(GridRef, x) - 16usize];
    ["Offset of field: GridRef::y"][::std::mem::offset_of!(GridRef, y) - 18usize];
};
impl Default for GridRef {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
unsafe extern "C" {
    pub fn ghostty_grid_ref_cell(ref_: *const GridRef, out_cell: *mut Cell) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_grid_ref_row(ref_: *const GridRef, out_row: *mut Row) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_grid_ref_graphemes(
        ref_: *const GridRef,
        buf: *mut u32,
        buf_len: usize,
        out_len: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_grid_ref_hyperlink_uri(
        ref_: *const GridRef,
        buf: *mut u8,
        buf_len: usize,
        out_len: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_grid_ref_style(ref_: *const GridRef, out_style: *mut Style) -> Result::Type;
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct PointCoordinate {
    pub x: u16,
    pub y: u32,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of PointCoordinate"][::std::mem::size_of::<PointCoordinate>() - 8usize];
    ["Alignment of PointCoordinate"][::std::mem::align_of::<PointCoordinate>() - 4usize];
    ["Offset of field: PointCoordinate::x"][::std::mem::offset_of!(PointCoordinate, x) - 0usize];
    ["Offset of field: PointCoordinate::y"][::std::mem::offset_of!(PointCoordinate, y) - 4usize];
};
pub mod PointTag {
    pub type Type = ::std::os::raw::c_int;
    pub const ACTIVE: Type = 0;
    pub const VIEWPORT: Type = 1;
    pub const SCREEN: Type = 2;
    pub const HISTORY: Type = 3;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union PointValue {
    pub coordinate: PointCoordinate,
    pub _padding: [u64; 2usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of PointValue"][::std::mem::size_of::<PointValue>() - 16usize];
    ["Alignment of PointValue"][::std::mem::align_of::<PointValue>() - 8usize];
    ["Offset of field: PointValue::coordinate"]
        [::std::mem::offset_of!(PointValue, coordinate) - 0usize];
    ["Offset of field: PointValue::_padding"]
        [::std::mem::offset_of!(PointValue, _padding) - 0usize];
};
impl Default for PointValue {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Point {
    pub tag: PointTag::Type,
    pub value: PointValue,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of Point"][::std::mem::size_of::<Point>() - 24usize];
    ["Alignment of Point"][::std::mem::align_of::<Point>() - 8usize];
    ["Offset of field: Point::tag"][::std::mem::offset_of!(Point, tag) - 0usize];
    ["Offset of field: Point::value"][::std::mem::offset_of!(Point, value) - 8usize];
};
impl Default for Point {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SelectionGestureImpl {
    _unused: [u8; 0],
}
pub type SelectionGesture = *mut SelectionGestureImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SelectionGestureEventImpl {
    _unused: [u8; 0],
}
pub type SelectionGestureEvent = *mut SelectionGestureEventImpl;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Selection {
    pub size: usize,
    pub start: GridRef,
    pub end: GridRef,
    pub rectangle: bool,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of Selection"][::std::mem::size_of::<Selection>() - 64usize];
    ["Alignment of Selection"][::std::mem::align_of::<Selection>() - 8usize];
    ["Offset of field: Selection::size"][::std::mem::offset_of!(Selection, size) - 0usize];
    ["Offset of field: Selection::start"][::std::mem::offset_of!(Selection, start) - 8usize];
    ["Offset of field: Selection::end"][::std::mem::offset_of!(Selection, end) - 32usize];
    ["Offset of field: Selection::rectangle"]
        [::std::mem::offset_of!(Selection, rectangle) - 56usize];
};
impl Default for Selection {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SelectionBuffer {
    pub ptr: *mut Selection,
    pub cap: usize,
    pub len: usize,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of SelectionBuffer"][::std::mem::size_of::<SelectionBuffer>() - 24usize];
    ["Alignment of SelectionBuffer"][::std::mem::align_of::<SelectionBuffer>() - 8usize];
    ["Offset of field: SelectionBuffer::ptr"]
        [::std::mem::offset_of!(SelectionBuffer, ptr) - 0usize];
    ["Offset of field: SelectionBuffer::cap"]
        [::std::mem::offset_of!(SelectionBuffer, cap) - 8usize];
    ["Offset of field: SelectionBuffer::len"]
        [::std::mem::offset_of!(SelectionBuffer, len) - 16usize];
};
impl Default for SelectionBuffer {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TerminalSelectWordOptions {
    pub size: usize,
    pub ref_: GridRef,
    pub boundary_codepoints: *const u32,
    pub boundary_codepoints_len: usize,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalSelectWordOptions"]
        [::std::mem::size_of::<TerminalSelectWordOptions>() - 48usize];
    ["Alignment of TerminalSelectWordOptions"]
        [::std::mem::align_of::<TerminalSelectWordOptions>() - 8usize];
    ["Offset of field: TerminalSelectWordOptions::size"]
        [::std::mem::offset_of!(TerminalSelectWordOptions, size) - 0usize];
    ["Offset of field: TerminalSelectWordOptions::ref_"]
        [::std::mem::offset_of!(TerminalSelectWordOptions, ref_) - 8usize];
    ["Offset of field: TerminalSelectWordOptions::boundary_codepoints"]
        [::std::mem::offset_of!(TerminalSelectWordOptions, boundary_codepoints) - 32usize];
    ["Offset of field: TerminalSelectWordOptions::boundary_codepoints_len"]
        [::std::mem::offset_of!(TerminalSelectWordOptions, boundary_codepoints_len) - 40usize];
};
impl Default for TerminalSelectWordOptions {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TerminalSelectWordBetweenOptions {
    pub size: usize,
    pub start: GridRef,
    pub end: GridRef,
    pub boundary_codepoints: *const u32,
    pub boundary_codepoints_len: usize,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalSelectWordBetweenOptions"]
        [::std::mem::size_of::<TerminalSelectWordBetweenOptions>() - 72usize];
    ["Alignment of TerminalSelectWordBetweenOptions"]
        [::std::mem::align_of::<TerminalSelectWordBetweenOptions>() - 8usize];
    ["Offset of field: TerminalSelectWordBetweenOptions::size"]
        [::std::mem::offset_of!(TerminalSelectWordBetweenOptions, size) - 0usize];
    ["Offset of field: TerminalSelectWordBetweenOptions::start"]
        [::std::mem::offset_of!(TerminalSelectWordBetweenOptions, start) - 8usize];
    ["Offset of field: TerminalSelectWordBetweenOptions::end"]
        [::std::mem::offset_of!(TerminalSelectWordBetweenOptions, end) - 32usize];
    ["Offset of field: TerminalSelectWordBetweenOptions::boundary_codepoints"]
        [::std::mem::offset_of!(TerminalSelectWordBetweenOptions, boundary_codepoints) - 56usize];
    ["Offset of field: TerminalSelectWordBetweenOptions::boundary_codepoints_len"][::std::mem::offset_of!(
        TerminalSelectWordBetweenOptions,
        boundary_codepoints_len
    ) - 64usize];
};
impl Default for TerminalSelectWordBetweenOptions {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TerminalSelectLineOptions {
    pub size: usize,
    pub ref_: GridRef,
    pub whitespace: *const u32,
    pub whitespace_len: usize,
    pub semantic_prompt_boundary: bool,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalSelectLineOptions"]
        [::std::mem::size_of::<TerminalSelectLineOptions>() - 56usize];
    ["Alignment of TerminalSelectLineOptions"]
        [::std::mem::align_of::<TerminalSelectLineOptions>() - 8usize];
    ["Offset of field: TerminalSelectLineOptions::size"]
        [::std::mem::offset_of!(TerminalSelectLineOptions, size) - 0usize];
    ["Offset of field: TerminalSelectLineOptions::ref_"]
        [::std::mem::offset_of!(TerminalSelectLineOptions, ref_) - 8usize];
    ["Offset of field: TerminalSelectLineOptions::whitespace"]
        [::std::mem::offset_of!(TerminalSelectLineOptions, whitespace) - 32usize];
    ["Offset of field: TerminalSelectLineOptions::whitespace_len"]
        [::std::mem::offset_of!(TerminalSelectLineOptions, whitespace_len) - 40usize];
    ["Offset of field: TerminalSelectLineOptions::semantic_prompt_boundary"]
        [::std::mem::offset_of!(TerminalSelectLineOptions, semantic_prompt_boundary) - 48usize];
};
impl Default for TerminalSelectLineOptions {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TerminalSelectionFormatOptions {
    pub size: usize,
    pub emit: FormatterFormat::Type,
    pub unwrap: bool,
    pub trim: bool,
    pub selection: *const Selection,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalSelectionFormatOptions"]
        [::std::mem::size_of::<TerminalSelectionFormatOptions>() - 24usize];
    ["Alignment of TerminalSelectionFormatOptions"]
        [::std::mem::align_of::<TerminalSelectionFormatOptions>() - 8usize];
    ["Offset of field: TerminalSelectionFormatOptions::size"]
        [::std::mem::offset_of!(TerminalSelectionFormatOptions, size) - 0usize];
    ["Offset of field: TerminalSelectionFormatOptions::emit"]
        [::std::mem::offset_of!(TerminalSelectionFormatOptions, emit) - 8usize];
    ["Offset of field: TerminalSelectionFormatOptions::unwrap"]
        [::std::mem::offset_of!(TerminalSelectionFormatOptions, unwrap) - 12usize];
    ["Offset of field: TerminalSelectionFormatOptions::trim"]
        [::std::mem::offset_of!(TerminalSelectionFormatOptions, trim) - 13usize];
    ["Offset of field: TerminalSelectionFormatOptions::selection"]
        [::std::mem::offset_of!(TerminalSelectionFormatOptions, selection) - 16usize];
};
impl Default for TerminalSelectionFormatOptions {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub mod SelectionOrder {
    pub type Type = ::std::os::raw::c_int;
    pub const FORWARD: Type = 0;
    pub const REVERSE: Type = 1;
    pub const MIRRORED_FORWARD: Type = 2;
    pub const MIRRORED_REVERSE: Type = 3;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod SelectionAdjust {
    pub type Type = ::std::os::raw::c_int;
    pub const LEFT: Type = 0;
    pub const RIGHT: Type = 1;
    pub const UP: Type = 2;
    pub const DOWN: Type = 3;
    pub const HOME: Type = 4;
    pub const END: Type = 5;
    pub const PAGE_UP: Type = 6;
    pub const PAGE_DOWN: Type = 7;
    pub const BEGINNING_OF_LINE: Type = 8;
    pub const END_OF_LINE: Type = 9;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod SelectionGestureBehavior {
    pub type Type = ::std::os::raw::c_int;
    pub const CELL: Type = 0;
    pub const WORD: Type = 1;
    pub const LINE: Type = 2;
    pub const OUTPUT: Type = 3;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SelectionGestureBehaviors {
    pub single_click: SelectionGestureBehavior::Type,
    pub double_click: SelectionGestureBehavior::Type,
    pub triple_click: SelectionGestureBehavior::Type,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of SelectionGestureBehaviors"]
        [::std::mem::size_of::<SelectionGestureBehaviors>() - 12usize];
    ["Alignment of SelectionGestureBehaviors"]
        [::std::mem::align_of::<SelectionGestureBehaviors>() - 4usize];
    ["Offset of field: SelectionGestureBehaviors::single_click"]
        [::std::mem::offset_of!(SelectionGestureBehaviors, single_click) - 0usize];
    ["Offset of field: SelectionGestureBehaviors::double_click"]
        [::std::mem::offset_of!(SelectionGestureBehaviors, double_click) - 4usize];
    ["Offset of field: SelectionGestureBehaviors::triple_click"]
        [::std::mem::offset_of!(SelectionGestureBehaviors, triple_click) - 8usize];
};
impl Default for SelectionGestureBehaviors {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct SelectionGestureGeometry {
    pub columns: u32,
    pub cell_width: u32,
    pub padding_left: u32,
    pub screen_height: u32,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of SelectionGestureGeometry"]
        [::std::mem::size_of::<SelectionGestureGeometry>() - 16usize];
    ["Alignment of SelectionGestureGeometry"]
        [::std::mem::align_of::<SelectionGestureGeometry>() - 4usize];
    ["Offset of field: SelectionGestureGeometry::columns"]
        [::std::mem::offset_of!(SelectionGestureGeometry, columns) - 0usize];
    ["Offset of field: SelectionGestureGeometry::cell_width"]
        [::std::mem::offset_of!(SelectionGestureGeometry, cell_width) - 4usize];
    ["Offset of field: SelectionGestureGeometry::padding_left"]
        [::std::mem::offset_of!(SelectionGestureGeometry, padding_left) - 8usize];
    ["Offset of field: SelectionGestureGeometry::screen_height"]
        [::std::mem::offset_of!(SelectionGestureGeometry, screen_height) - 12usize];
};
pub mod SelectionGestureAutoscroll {
    pub type Type = ::std::os::raw::c_int;
    pub const NONE: Type = 0;
    pub const UP: Type = 1;
    pub const DOWN: Type = 2;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod SelectionGestureData {
    pub type Type = ::std::os::raw::c_int;
    pub const CLICK_COUNT: Type = 0;
    pub const DRAGGED: Type = 1;
    pub const AUTOSCROLL: Type = 2;
    pub const BEHAVIOR: Type = 3;
    pub const ANCHOR: Type = 4;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod SelectionGestureEventType {
    pub type Type = ::std::os::raw::c_int;
    pub const PRESS: Type = 0;
    pub const RELEASE: Type = 1;
    pub const DRAG: Type = 2;
    pub const AUTOSCROLL_TICK: Type = 3;
    pub const DEEP_PRESS: Type = 4;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod SelectionGestureEventOption {
    pub type Type = ::std::os::raw::c_int;
    pub const REF: Type = 0;
    pub const POSITION: Type = 1;
    pub const REPEAT_DISTANCE: Type = 2;
    pub const TIME_NS: Type = 3;
    pub const REPEAT_INTERVAL_NS: Type = 4;
    pub const WORD_BOUNDARY_CODEPOINTS: Type = 5;
    pub const BEHAVIORS: Type = 6;
    pub const RECTANGLE: Type = 7;
    pub const GEOMETRY: Type = 8;
    pub const VIEWPORT: Type = 9;
    pub const MAX_VALUE: Type = 2147483647;
}
unsafe extern "C" {
    pub fn ghostty_selection_gesture_event_new(
        allocator: *const Allocator,
        out_event: *mut SelectionGestureEvent,
        type_: SelectionGestureEventType::Type,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_selection_gesture_event_free(event: SelectionGestureEvent);
}
unsafe extern "C" {
    pub fn ghostty_selection_gesture_event_set(
        event: SelectionGestureEvent,
        option: SelectionGestureEventOption::Type,
        value: *const ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_selection_gesture_event(
        gesture: SelectionGesture,
        terminal: Terminal,
        event: SelectionGestureEvent,
        out_selection: *mut Selection,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_selection_gesture_new(
        allocator: *const Allocator,
        out_gesture: *mut SelectionGesture,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_selection_gesture_free(gesture: SelectionGesture, terminal: Terminal);
}
unsafe extern "C" {
    pub fn ghostty_selection_gesture_reset(gesture: SelectionGesture, terminal: Terminal);
}
unsafe extern "C" {
    pub fn ghostty_selection_gesture_get(
        gesture: SelectionGesture,
        terminal: Terminal,
        data: SelectionGestureData::Type,
        value: *mut ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_selection_gesture_get_multi(
        gesture: SelectionGesture,
        terminal: Terminal,
        count: usize,
        keys: *const SelectionGestureData::Type,
        values: *mut *mut ::std::os::raw::c_void,
        out_written: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_select_word(
        terminal: Terminal,
        options: *const TerminalSelectWordOptions,
        out_selection: *mut Selection,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_select_word_between(
        terminal: Terminal,
        options: *const TerminalSelectWordBetweenOptions,
        out_selection: *mut Selection,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_select_line(
        terminal: Terminal,
        options: *const TerminalSelectLineOptions,
        out_selection: *mut Selection,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_select_all(
        terminal: Terminal,
        out_selection: *mut Selection,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_select_output(
        terminal: Terminal,
        ref_: GridRef,
        out_selection: *mut Selection,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_selection_format_buf(
        terminal: Terminal,
        options: TerminalSelectionFormatOptions,
        buf: *mut u8,
        buf_len: usize,
        out_written: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_selection_format_alloc(
        terminal: Terminal,
        allocator: *const Allocator,
        options: TerminalSelectionFormatOptions,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_selection_adjust(
        terminal: Terminal,
        selection: *mut Selection,
        adjustment: SelectionAdjust::Type,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_selection_order(
        terminal: Terminal,
        selection: *const Selection,
        out_order: *mut SelectionOrder::Type,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_selection_ordered(
        terminal: Terminal,
        selection: *const Selection,
        desired: SelectionOrder::Type,
        out_selection: *mut Selection,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_selection_contains(
        terminal: Terminal,
        selection: *const Selection,
        point: Point,
        out_contains: *mut bool,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_selection_equal(
        terminal: Terminal,
        a: *const Selection,
        b: *const Selection,
        out_equal: *mut bool,
    ) -> Result::Type;
}
pub type Mode = u16;
pub mod ModeReportState {
    pub type Type = ::std::os::raw::c_int;
    pub const NOT_RECOGNIZED: Type = 0;
    pub const SET: Type = 1;
    pub const RESET: Type = 2;
    pub const PERMANENTLY_SET: Type = 3;
    pub const PERMANENTLY_RESET: Type = 4;
    pub const MAX_VALUE: Type = 2147483647;
}
unsafe extern "C" {
    pub fn ghostty_mode_report_encode(
        mode: Mode,
        state: ModeReportState::Type,
        buf: *mut ::std::os::raw::c_char,
        buf_len: usize,
        out_written: *mut usize,
    ) -> Result::Type;
}
pub mod OscCommandType {
    pub type Type = ::std::os::raw::c_int;
    pub const INVALID: Type = 0;
    pub const CHANGE_WINDOW_TITLE: Type = 1;
    pub const CHANGE_WINDOW_ICON: Type = 2;
    pub const SEMANTIC_PROMPT: Type = 3;
    pub const CLIPBOARD_CONTENTS: Type = 4;
    pub const REPORT_PWD: Type = 5;
    pub const MOUSE_SHAPE: Type = 6;
    pub const COLOR_OPERATION: Type = 7;
    pub const KITTY_COLOR_PROTOCOL: Type = 8;
    pub const SHOW_DESKTOP_NOTIFICATION: Type = 9;
    pub const HYPERLINK_START: Type = 10;
    pub const HYPERLINK_END: Type = 11;
    pub const CONEMU_SLEEP: Type = 12;
    pub const CONEMU_SHOW_MESSAGE_BOX: Type = 13;
    pub const CONEMU_CHANGE_TAB_TITLE: Type = 14;
    pub const CONEMU_PROGRESS_REPORT: Type = 15;
    pub const CONEMU_WAIT_INPUT: Type = 16;
    pub const CONEMU_GUIMACRO: Type = 17;
    pub const CONEMU_RUN_PROCESS: Type = 18;
    pub const CONEMU_OUTPUT_ENVIRONMENT_VARIABLE: Type = 19;
    pub const CONEMU_XTERM_EMULATION: Type = 20;
    pub const CONEMU_COMMENT: Type = 21;
    pub const KITTY_TEXT_SIZING: Type = 22;
    pub const KITTY_CLIPBOARD_PROTOCOL: Type = 23;
    pub const KITTY_DND_PROTOCOL: Type = 24;
    pub const HIERARCHICAL_CONTEXT: Type = 25;
    pub const KITTY_DESKTOP_NOTIFICATION: Type = 26;
    pub const UNKNOWN: Type = 27;
    pub const PROGRAM_STATUS: Type = 28;
    pub const TYPE_MAX_VALUE: Type = 2147483647;
}
pub mod OscTerminator {
    pub type Type = ::std::os::raw::c_int;
    pub const ST: Type = 0;
    pub const BEL: Type = 1;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod OscOption {
    pub type Type = ::std::os::raw::c_int;
    pub const UNKNOWN_MAX_BYTES: Type = 0;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod OscCommandData {
    pub type Type = ::std::os::raw::c_int;
    pub const INVALID: Type = 0;
    pub const CHANGE_WINDOW_TITLE_STR: Type = 1;
    pub const UNKNOWN_CONTENT: Type = 2;
    pub const UNKNOWN_TRUNCATED: Type = 3;
    pub const UNKNOWN_TERMINATOR: Type = 4;
    pub const MAX_VALUE: Type = 2147483647;
}
unsafe extern "C" {
    pub fn ghostty_osc_new(allocator: *const Allocator, parser: *mut OscParser) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_osc_free(parser: OscParser);
}
unsafe extern "C" {
    pub fn ghostty_osc_reset(parser: OscParser);
}
unsafe extern "C" {
    pub fn ghostty_osc_set(
        parser: OscParser,
        option: OscOption::Type,
        value: *const ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_osc_next(parser: OscParser, byte: u8);
}
unsafe extern "C" {
    pub fn ghostty_osc_end(parser: OscParser, terminator: u8) -> OscCommand;
}
unsafe extern "C" {
    pub fn ghostty_osc_command_type(command: OscCommand) -> OscCommandType::Type;
}
unsafe extern "C" {
    pub fn ghostty_osc_command_data(
        command: OscCommand,
        data: OscCommandData::Type,
        out: *mut ::std::os::raw::c_void,
    ) -> bool;
}
pub mod SizeReportStyle {
    pub type Type = ::std::os::raw::c_int;
    pub const MODE_2048: Type = 0;
    pub const CSI_14_T: Type = 1;
    pub const CSI_16_T: Type = 2;
    pub const CSI_18_T: Type = 3;
    pub const CSI_19_T: Type = 4;
    pub const STYLE_MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct SizeReportSize {
    pub rows: u16,
    pub columns: u16,
    pub cell_width: u32,
    pub cell_height: u32,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of SizeReportSize"][::std::mem::size_of::<SizeReportSize>() - 12usize];
    ["Alignment of SizeReportSize"][::std::mem::align_of::<SizeReportSize>() - 4usize];
    ["Offset of field: SizeReportSize::rows"]
        [::std::mem::offset_of!(SizeReportSize, rows) - 0usize];
    ["Offset of field: SizeReportSize::columns"]
        [::std::mem::offset_of!(SizeReportSize, columns) - 2usize];
    ["Offset of field: SizeReportSize::cell_width"]
        [::std::mem::offset_of!(SizeReportSize, cell_width) - 4usize];
    ["Offset of field: SizeReportSize::cell_height"]
        [::std::mem::offset_of!(SizeReportSize, cell_height) - 8usize];
};
unsafe extern "C" {
    pub fn ghostty_size_report_encode(
        style: SizeReportStyle::Type,
        size: SizeReportSize,
        buf: *mut ::std::os::raw::c_char,
        buf_len: usize,
        out_written: *mut usize,
    ) -> Result::Type;
}
pub mod KittyGraphicsData {
    pub type Type = ::std::os::raw::c_int;
    pub const INVALID: Type = 0;
    pub const PLACEMENT_ITERATOR: Type = 1;
    pub const GENERATION: Type = 2;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod KittyGraphicsPlacementData {
    pub type Type = ::std::os::raw::c_int;
    pub const INVALID: Type = 0;
    pub const IMAGE_ID: Type = 1;
    pub const PLACEMENT_ID: Type = 2;
    pub const IS_VIRTUAL: Type = 3;
    pub const X_OFFSET: Type = 4;
    pub const Y_OFFSET: Type = 5;
    pub const SOURCE_X: Type = 6;
    pub const SOURCE_Y: Type = 7;
    pub const SOURCE_WIDTH: Type = 8;
    pub const SOURCE_HEIGHT: Type = 9;
    pub const COLUMNS: Type = 10;
    pub const ROWS: Type = 11;
    pub const Z: Type = 12;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod KittyPlacementLayer {
    pub type Type = ::std::os::raw::c_int;
    pub const ALL: Type = 0;
    pub const BELOW_BG: Type = 1;
    pub const BELOW_TEXT: Type = 2;
    pub const ABOVE_TEXT: Type = 3;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod KittyGraphicsPlacementIteratorOption {
    pub type Type = ::std::os::raw::c_int;
    pub const LAYER: Type = 0;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod KittyImageFormat {
    pub type Type = ::std::os::raw::c_int;
    pub const RGB: Type = 0;
    pub const RGBA: Type = 1;
    pub const PNG: Type = 2;
    pub const GRAY_ALPHA: Type = 3;
    pub const GRAY: Type = 4;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod KittyImageCompression {
    pub type Type = ::std::os::raw::c_int;
    pub const NONE: Type = 0;
    pub const ZLIB_DEFLATE: Type = 1;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod KittyAnimationState {
    pub type Type = ::std::os::raw::c_int;
    pub const STOPPED: Type = 0;
    pub const LOADING: Type = 1;
    pub const RUNNING: Type = 2;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod KittyGraphicsImageData {
    pub type Type = ::std::os::raw::c_int;
    pub const INVALID: Type = 0;
    pub const ID: Type = 1;
    pub const NUMBER: Type = 2;
    pub const WIDTH: Type = 3;
    pub const HEIGHT: Type = 4;
    pub const FORMAT: Type = 5;
    pub const COMPRESSION: Type = 6;
    pub const DATA_PTR: Type = 7;
    pub const DATA_LEN: Type = 8;
    pub const GENERATION: Type = 9;
    pub const FRAME_COUNT: Type = 10;
    pub const ANIMATION_STATE: Type = 11;
    pub const CURRENT_FRAME: Type = 12;
    pub const LOOPS: Type = 13;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct KittyImageFrame {
    pub data: *const u8,
    pub data_len: usize,
    pub gap_ms: u32,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of KittyImageFrame"][::std::mem::size_of::<KittyImageFrame>() - 24usize];
    ["Alignment of KittyImageFrame"][::std::mem::align_of::<KittyImageFrame>() - 8usize];
    ["Offset of field: KittyImageFrame::data"]
        [::std::mem::offset_of!(KittyImageFrame, data) - 0usize];
    ["Offset of field: KittyImageFrame::data_len"]
        [::std::mem::offset_of!(KittyImageFrame, data_len) - 8usize];
    ["Offset of field: KittyImageFrame::gap_ms"]
        [::std::mem::offset_of!(KittyImageFrame, gap_ms) - 16usize];
};
impl Default for KittyImageFrame {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct KittyGraphicsPlacementRenderInfo {
    pub size: usize,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub grid_cols: u32,
    pub grid_rows: u32,
    pub viewport_col: i32,
    pub viewport_row: i32,
    pub viewport_visible: bool,
    pub source_x: u32,
    pub source_y: u32,
    pub source_width: u32,
    pub source_height: u32,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of KittyGraphicsPlacementRenderInfo"]
        [::std::mem::size_of::<KittyGraphicsPlacementRenderInfo>() - 56usize];
    ["Alignment of KittyGraphicsPlacementRenderInfo"]
        [::std::mem::align_of::<KittyGraphicsPlacementRenderInfo>() - 8usize];
    ["Offset of field: KittyGraphicsPlacementRenderInfo::size"]
        [::std::mem::offset_of!(KittyGraphicsPlacementRenderInfo, size) - 0usize];
    ["Offset of field: KittyGraphicsPlacementRenderInfo::pixel_width"]
        [::std::mem::offset_of!(KittyGraphicsPlacementRenderInfo, pixel_width) - 8usize];
    ["Offset of field: KittyGraphicsPlacementRenderInfo::pixel_height"]
        [::std::mem::offset_of!(KittyGraphicsPlacementRenderInfo, pixel_height) - 12usize];
    ["Offset of field: KittyGraphicsPlacementRenderInfo::grid_cols"]
        [::std::mem::offset_of!(KittyGraphicsPlacementRenderInfo, grid_cols) - 16usize];
    ["Offset of field: KittyGraphicsPlacementRenderInfo::grid_rows"]
        [::std::mem::offset_of!(KittyGraphicsPlacementRenderInfo, grid_rows) - 20usize];
    ["Offset of field: KittyGraphicsPlacementRenderInfo::viewport_col"]
        [::std::mem::offset_of!(KittyGraphicsPlacementRenderInfo, viewport_col) - 24usize];
    ["Offset of field: KittyGraphicsPlacementRenderInfo::viewport_row"]
        [::std::mem::offset_of!(KittyGraphicsPlacementRenderInfo, viewport_row) - 28usize];
    ["Offset of field: KittyGraphicsPlacementRenderInfo::viewport_visible"]
        [::std::mem::offset_of!(KittyGraphicsPlacementRenderInfo, viewport_visible) - 32usize];
    ["Offset of field: KittyGraphicsPlacementRenderInfo::source_x"]
        [::std::mem::offset_of!(KittyGraphicsPlacementRenderInfo, source_x) - 36usize];
    ["Offset of field: KittyGraphicsPlacementRenderInfo::source_y"]
        [::std::mem::offset_of!(KittyGraphicsPlacementRenderInfo, source_y) - 40usize];
    ["Offset of field: KittyGraphicsPlacementRenderInfo::source_width"]
        [::std::mem::offset_of!(KittyGraphicsPlacementRenderInfo, source_width) - 44usize];
    ["Offset of field: KittyGraphicsPlacementRenderInfo::source_height"]
        [::std::mem::offset_of!(KittyGraphicsPlacementRenderInfo, source_height) - 48usize];
};
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_get(
        graphics: KittyGraphics,
        data: KittyGraphicsData::Type,
        out: *mut ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_animation_tick(
        graphics: KittyGraphics,
        now_ms: u64,
        next_ms: *mut u64,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_image(
        graphics: KittyGraphics,
        image_id: u32,
    ) -> KittyGraphicsImage;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_image_get(
        image: KittyGraphicsImage,
        data: KittyGraphicsImageData::Type,
        out: *mut ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_image_get_multi(
        image: KittyGraphicsImage,
        count: usize,
        keys: *const KittyGraphicsImageData::Type,
        values: *mut *mut ::std::os::raw::c_void,
        out_written: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_image_frame(
        image: KittyGraphicsImage,
        number: u32,
        out: *mut KittyImageFrame,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_placement_iterator_new(
        allocator: *const Allocator,
        out_iterator: *mut KittyGraphicsPlacementIterator,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_placement_iterator_free(iterator: KittyGraphicsPlacementIterator);
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_placement_iterator_set(
        iterator: KittyGraphicsPlacementIterator,
        option: KittyGraphicsPlacementIteratorOption::Type,
        value: *const ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_placement_next(iterator: KittyGraphicsPlacementIterator) -> bool;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_placement_get(
        iterator: KittyGraphicsPlacementIterator,
        data: KittyGraphicsPlacementData::Type,
        out: *mut ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_placement_get_multi(
        iterator: KittyGraphicsPlacementIterator,
        count: usize,
        keys: *const KittyGraphicsPlacementData::Type,
        values: *mut *mut ::std::os::raw::c_void,
        out_written: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_placement_rect(
        iterator: KittyGraphicsPlacementIterator,
        image: KittyGraphicsImage,
        terminal: Terminal,
        out_selection: *mut Selection,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_placement_pixel_size(
        iterator: KittyGraphicsPlacementIterator,
        image: KittyGraphicsImage,
        terminal: Terminal,
        out_width: *mut u32,
        out_height: *mut u32,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_placement_grid_size(
        iterator: KittyGraphicsPlacementIterator,
        image: KittyGraphicsImage,
        terminal: Terminal,
        out_cols: *mut u32,
        out_rows: *mut u32,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_placement_viewport_pos(
        iterator: KittyGraphicsPlacementIterator,
        image: KittyGraphicsImage,
        terminal: Terminal,
        out_col: *mut i32,
        out_row: *mut i32,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_placement_source_rect(
        iterator: KittyGraphicsPlacementIterator,
        image: KittyGraphicsImage,
        out_x: *mut u32,
        out_y: *mut u32,
        out_width: *mut u32,
        out_height: *mut u32,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_kitty_graphics_placement_render_info(
        iterator: KittyGraphicsPlacementIterator,
        image: KittyGraphicsImage,
        terminal: Terminal,
        out_info: *mut KittyGraphicsPlacementRenderInfo,
    ) -> Result::Type;
}
pub mod TerminalCompressionMode {
    pub type Type = ::std::os::raw::c_int;
    pub const INCREMENTAL: Type = 0;
    pub const FULL: Type = 1;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod TerminalCompressionResult {
    pub type Type = ::std::os::raw::c_int;
    pub const UNSUPPORTED: Type = 0;
    pub const PENDING: Type = 1;
    pub const COMPLETE: Type = 2;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod TerminalScrollViewportTag {
    pub type Type = ::std::os::raw::c_int;
    pub const TOP: Type = 0;
    pub const BOTTOM: Type = 1;
    pub const DELTA: Type = 2;
    pub const ROW: Type = 3;
    pub const DELTA_PROMPT: Type = 4;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union TerminalScrollViewportValue {
    pub delta: isize,
    pub row: usize,
    pub delta_prompt: isize,
    pub _padding: [u64; 2usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalScrollViewportValue"]
        [::std::mem::size_of::<TerminalScrollViewportValue>() - 16usize];
    ["Alignment of TerminalScrollViewportValue"]
        [::std::mem::align_of::<TerminalScrollViewportValue>() - 8usize];
    ["Offset of field: TerminalScrollViewportValue::delta"]
        [::std::mem::offset_of!(TerminalScrollViewportValue, delta) - 0usize];
    ["Offset of field: TerminalScrollViewportValue::row"]
        [::std::mem::offset_of!(TerminalScrollViewportValue, row) - 0usize];
    ["Offset of field: TerminalScrollViewportValue::delta_prompt"]
        [::std::mem::offset_of!(TerminalScrollViewportValue, delta_prompt) - 0usize];
    ["Offset of field: TerminalScrollViewportValue::_padding"]
        [::std::mem::offset_of!(TerminalScrollViewportValue, _padding) - 0usize];
};
impl Default for TerminalScrollViewportValue {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Copy, Clone)]
pub struct TerminalScrollViewport {
    pub tag: TerminalScrollViewportTag::Type,
    pub value: TerminalScrollViewportValue,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalScrollViewport"][::std::mem::size_of::<TerminalScrollViewport>() - 24usize];
    ["Alignment of TerminalScrollViewport"]
        [::std::mem::align_of::<TerminalScrollViewport>() - 8usize];
    ["Offset of field: TerminalScrollViewport::tag"]
        [::std::mem::offset_of!(TerminalScrollViewport, tag) - 0usize];
    ["Offset of field: TerminalScrollViewport::value"]
        [::std::mem::offset_of!(TerminalScrollViewport, value) - 8usize];
};
impl Default for TerminalScrollViewport {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub mod TerminalScreen {
    pub type Type = ::std::os::raw::c_int;
    pub const PRIMARY: Type = 0;
    pub const ALTERNATE: Type = 1;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod TerminalCursorStyle {
    pub type Type = ::std::os::raw::c_int;
    pub const BAR: Type = 0;
    pub const BLOCK: Type = 1;
    pub const UNDERLINE: Type = 2;
    pub const BLOCK_HOLLOW: Type = 3;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod TerminalBoundary {
    pub type Type = ::std::os::raw::c_int;
    pub const GROUND: Type = 1;
    pub const APC_START: Type = 2;
    pub const APC_END: Type = 4;
    pub const DCS_START: Type = 8;
    pub const DCS_END: Type = 16;
    pub const OSC_START: Type = 32;
    pub const OSC_END: Type = 64;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod TerminalStringKind {
    pub type Type = ::std::os::raw::c_int;
    pub const APC: Type = 0;
    pub const DCS: Type = 1;
    pub const OSC: Type = 2;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod TerminalStringIntroducer {
    pub type Type = ::std::os::raw::c_int;
    pub const ESC: Type = 0;
    pub const C1: Type = 1;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod TerminalStringEnd {
    pub type Type = ::std::os::raw::c_int;
    pub const ST_ESC: Type = 0;
    pub const ST_C1: Type = 1;
    pub const CANCELLED: Type = 2;
    pub const ABORTED: Type = 3;
    pub const BEL: Type = 4;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TerminalWriteStop {
    pub size: usize,
    pub consumed: usize,
    pub reached: u32,
    pub kind: TerminalStringKind::Type,
    pub introducer: TerminalStringIntroducer::Type,
    pub end: TerminalStringEnd::Type,
    pub byte: u8,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalWriteStop"][::std::mem::size_of::<TerminalWriteStop>() - 40usize];
    ["Alignment of TerminalWriteStop"][::std::mem::align_of::<TerminalWriteStop>() - 8usize];
    ["Offset of field: TerminalWriteStop::size"]
        [::std::mem::offset_of!(TerminalWriteStop, size) - 0usize];
    ["Offset of field: TerminalWriteStop::consumed"]
        [::std::mem::offset_of!(TerminalWriteStop, consumed) - 8usize];
    ["Offset of field: TerminalWriteStop::reached"]
        [::std::mem::offset_of!(TerminalWriteStop, reached) - 16usize];
    ["Offset of field: TerminalWriteStop::kind"]
        [::std::mem::offset_of!(TerminalWriteStop, kind) - 20usize];
    ["Offset of field: TerminalWriteStop::introducer"]
        [::std::mem::offset_of!(TerminalWriteStop, introducer) - 24usize];
    ["Offset of field: TerminalWriteStop::end"]
        [::std::mem::offset_of!(TerminalWriteStop, end) - 28usize];
    ["Offset of field: TerminalWriteStop::byte"]
        [::std::mem::offset_of!(TerminalWriteStop, byte) - 32usize];
};
impl Default for TerminalWriteStop {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct TerminalScrollbar {
    pub total: u64,
    pub offset: u64,
    pub len: u64,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalScrollbar"][::std::mem::size_of::<TerminalScrollbar>() - 24usize];
    ["Alignment of TerminalScrollbar"][::std::mem::align_of::<TerminalScrollbar>() - 8usize];
    ["Offset of field: TerminalScrollbar::total"]
        [::std::mem::offset_of!(TerminalScrollbar, total) - 0usize];
    ["Offset of field: TerminalScrollbar::offset"]
        [::std::mem::offset_of!(TerminalScrollbar, offset) - 8usize];
    ["Offset of field: TerminalScrollbar::len"]
        [::std::mem::offset_of!(TerminalScrollbar, len) - 16usize];
};
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct TerminalMemoryUsage {
    pub size: usize,
    pub compression_supported: bool,
    pub primary_pages: u64,
    pub primary_virtual_bytes: u64,
    pub primary_resident_bytes: u64,
    pub primary_compressed_pages: u64,
    pub primary_compressed_bytes: u64,
    pub primary_image_bytes: u64,
    pub alternate_pages: u64,
    pub alternate_virtual_bytes: u64,
    pub alternate_resident_bytes: u64,
    pub alternate_compressed_pages: u64,
    pub alternate_compressed_bytes: u64,
    pub alternate_image_bytes: u64,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalMemoryUsage"][::std::mem::size_of::<TerminalMemoryUsage>() - 112usize];
    ["Alignment of TerminalMemoryUsage"][::std::mem::align_of::<TerminalMemoryUsage>() - 8usize];
    ["Offset of field: TerminalMemoryUsage::size"]
        [::std::mem::offset_of!(TerminalMemoryUsage, size) - 0usize];
    ["Offset of field: TerminalMemoryUsage::compression_supported"]
        [::std::mem::offset_of!(TerminalMemoryUsage, compression_supported) - 8usize];
    ["Offset of field: TerminalMemoryUsage::primary_pages"]
        [::std::mem::offset_of!(TerminalMemoryUsage, primary_pages) - 16usize];
    ["Offset of field: TerminalMemoryUsage::primary_virtual_bytes"]
        [::std::mem::offset_of!(TerminalMemoryUsage, primary_virtual_bytes) - 24usize];
    ["Offset of field: TerminalMemoryUsage::primary_resident_bytes"]
        [::std::mem::offset_of!(TerminalMemoryUsage, primary_resident_bytes) - 32usize];
    ["Offset of field: TerminalMemoryUsage::primary_compressed_pages"]
        [::std::mem::offset_of!(TerminalMemoryUsage, primary_compressed_pages) - 40usize];
    ["Offset of field: TerminalMemoryUsage::primary_compressed_bytes"]
        [::std::mem::offset_of!(TerminalMemoryUsage, primary_compressed_bytes) - 48usize];
    ["Offset of field: TerminalMemoryUsage::primary_image_bytes"]
        [::std::mem::offset_of!(TerminalMemoryUsage, primary_image_bytes) - 56usize];
    ["Offset of field: TerminalMemoryUsage::alternate_pages"]
        [::std::mem::offset_of!(TerminalMemoryUsage, alternate_pages) - 64usize];
    ["Offset of field: TerminalMemoryUsage::alternate_virtual_bytes"]
        [::std::mem::offset_of!(TerminalMemoryUsage, alternate_virtual_bytes) - 72usize];
    ["Offset of field: TerminalMemoryUsage::alternate_resident_bytes"]
        [::std::mem::offset_of!(TerminalMemoryUsage, alternate_resident_bytes) - 80usize];
    ["Offset of field: TerminalMemoryUsage::alternate_compressed_pages"]
        [::std::mem::offset_of!(TerminalMemoryUsage, alternate_compressed_pages) - 88usize];
    ["Offset of field: TerminalMemoryUsage::alternate_compressed_bytes"]
        [::std::mem::offset_of!(TerminalMemoryUsage, alternate_compressed_bytes) - 96usize];
    ["Offset of field: TerminalMemoryUsage::alternate_image_bytes"]
        [::std::mem::offset_of!(TerminalMemoryUsage, alternate_image_bytes) - 104usize];
};
pub type TerminalBellFn = ::std::option::Option<
    unsafe extern "C" fn(terminal: Terminal, userdata: *mut ::std::os::raw::c_void),
>;
pub mod TerminalUnknownSequenceTag {
    pub type Type = ::std::os::raw::c_int;
    pub const APC: Type = 0;
    pub const OSC: Type = 1;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TerminalUnknownStringSequence {
    pub truncated: bool,
    pub content: String,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalUnknownStringSequence"]
        [::std::mem::size_of::<TerminalUnknownStringSequence>() - 24usize];
    ["Alignment of TerminalUnknownStringSequence"]
        [::std::mem::align_of::<TerminalUnknownStringSequence>() - 8usize];
    ["Offset of field: TerminalUnknownStringSequence::truncated"]
        [::std::mem::offset_of!(TerminalUnknownStringSequence, truncated) - 0usize];
    ["Offset of field: TerminalUnknownStringSequence::content"]
        [::std::mem::offset_of!(TerminalUnknownStringSequence, content) - 8usize];
};
impl Default for TerminalUnknownStringSequence {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TerminalUnknownOscSequence {
    pub truncated: bool,
    pub content: String,
    pub terminator: OscTerminator::Type,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalUnknownOscSequence"]
        [::std::mem::size_of::<TerminalUnknownOscSequence>() - 32usize];
    ["Alignment of TerminalUnknownOscSequence"]
        [::std::mem::align_of::<TerminalUnknownOscSequence>() - 8usize];
    ["Offset of field: TerminalUnknownOscSequence::truncated"]
        [::std::mem::offset_of!(TerminalUnknownOscSequence, truncated) - 0usize];
    ["Offset of field: TerminalUnknownOscSequence::content"]
        [::std::mem::offset_of!(TerminalUnknownOscSequence, content) - 8usize];
    ["Offset of field: TerminalUnknownOscSequence::terminator"]
        [::std::mem::offset_of!(TerminalUnknownOscSequence, terminator) - 24usize];
};
impl Default for TerminalUnknownOscSequence {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union TerminalUnknownSequenceValue {
    pub apc: TerminalUnknownStringSequence,
    pub osc: TerminalUnknownOscSequence,
    pub _padding: [u64; 16usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalUnknownSequenceValue"]
        [::std::mem::size_of::<TerminalUnknownSequenceValue>() - 128usize];
    ["Alignment of TerminalUnknownSequenceValue"]
        [::std::mem::align_of::<TerminalUnknownSequenceValue>() - 8usize];
    ["Offset of field: TerminalUnknownSequenceValue::apc"]
        [::std::mem::offset_of!(TerminalUnknownSequenceValue, apc) - 0usize];
    ["Offset of field: TerminalUnknownSequenceValue::osc"]
        [::std::mem::offset_of!(TerminalUnknownSequenceValue, osc) - 0usize];
    ["Offset of field: TerminalUnknownSequenceValue::_padding"]
        [::std::mem::offset_of!(TerminalUnknownSequenceValue, _padding) - 0usize];
};
impl Default for TerminalUnknownSequenceValue {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Copy, Clone)]
pub struct TerminalUnknownSequence {
    pub tag: TerminalUnknownSequenceTag::Type,
    pub value: TerminalUnknownSequenceValue,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalUnknownSequence"]
        [::std::mem::size_of::<TerminalUnknownSequence>() - 136usize];
    ["Alignment of TerminalUnknownSequence"]
        [::std::mem::align_of::<TerminalUnknownSequence>() - 8usize];
    ["Offset of field: TerminalUnknownSequence::tag"]
        [::std::mem::offset_of!(TerminalUnknownSequence, tag) - 0usize];
    ["Offset of field: TerminalUnknownSequence::value"]
        [::std::mem::offset_of!(TerminalUnknownSequence, value) - 8usize];
};
impl Default for TerminalUnknownSequence {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub type TerminalUnknownSequenceFn = ::std::option::Option<
    unsafe extern "C" fn(
        terminal: Terminal,
        userdata: *mut ::std::os::raw::c_void,
        sequence: *const TerminalUnknownSequence,
    ),
>;
pub mod ClipboardLocation {
    pub type Type = ::std::os::raw::c_int;
    pub const STANDARD: Type = 0;
    pub const SELECTION: Type = 1;
    pub const PRIMARY: Type = 2;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ClipboardContent {
    pub mime: String,
    pub data: String,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of ClipboardContent"][::std::mem::size_of::<ClipboardContent>() - 32usize];
    ["Alignment of ClipboardContent"][::std::mem::align_of::<ClipboardContent>() - 8usize];
    ["Offset of field: ClipboardContent::mime"]
        [::std::mem::offset_of!(ClipboardContent, mime) - 0usize];
    ["Offset of field: ClipboardContent::data"]
        [::std::mem::offset_of!(ClipboardContent, data) - 16usize];
};
impl Default for ClipboardContent {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub mod ClipboardWriteResult {
    pub type Type = ::std::os::raw::c_int;
    pub const SUCCESS: Type = 0;
    pub const DENIED: Type = 1;
    pub const UNSUPPORTED: Type = 2;
    pub const BUSY: Type = 3;
    pub const INVALID_DATA: Type = 4;
    pub const IO_ERROR: Type = 5;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ClipboardWriteReply {
    pub size: usize,
    pub result: ClipboardWriteResult::Type,
    pub remember: bool,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of ClipboardWriteReply"][::std::mem::size_of::<ClipboardWriteReply>() - 16usize];
    ["Alignment of ClipboardWriteReply"][::std::mem::align_of::<ClipboardWriteReply>() - 8usize];
    ["Offset of field: ClipboardWriteReply::size"]
        [::std::mem::offset_of!(ClipboardWriteReply, size) - 0usize];
    ["Offset of field: ClipboardWriteReply::result"]
        [::std::mem::offset_of!(ClipboardWriteReply, result) - 8usize];
    ["Offset of field: ClipboardWriteReply::remember"]
        [::std::mem::offset_of!(ClipboardWriteReply, remember) - 12usize];
};
impl Default for ClipboardWriteReply {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub type ClipboardWriteReplyFn = ::std::option::Option<
    unsafe extern "C" fn(write: *const ClipboardWrite, reply: *const ClipboardWriteReply),
>;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ClipboardWrite {
    pub size: usize,
    pub location: ClipboardLocation::Type,
    pub contents: *const ClipboardContent,
    pub contents_len: usize,
    pub name: String,
    pub granted: bool,
    pub can_remember: bool,
    pub ctx: *const ::std::os::raw::c_void,
    pub reply: ClipboardWriteReplyFn,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of ClipboardWrite"][::std::mem::size_of::<ClipboardWrite>() - 72usize];
    ["Alignment of ClipboardWrite"][::std::mem::align_of::<ClipboardWrite>() - 8usize];
    ["Offset of field: ClipboardWrite::size"]
        [::std::mem::offset_of!(ClipboardWrite, size) - 0usize];
    ["Offset of field: ClipboardWrite::location"]
        [::std::mem::offset_of!(ClipboardWrite, location) - 8usize];
    ["Offset of field: ClipboardWrite::contents"]
        [::std::mem::offset_of!(ClipboardWrite, contents) - 16usize];
    ["Offset of field: ClipboardWrite::contents_len"]
        [::std::mem::offset_of!(ClipboardWrite, contents_len) - 24usize];
    ["Offset of field: ClipboardWrite::name"]
        [::std::mem::offset_of!(ClipboardWrite, name) - 32usize];
    ["Offset of field: ClipboardWrite::granted"]
        [::std::mem::offset_of!(ClipboardWrite, granted) - 48usize];
    ["Offset of field: ClipboardWrite::can_remember"]
        [::std::mem::offset_of!(ClipboardWrite, can_remember) - 49usize];
    ["Offset of field: ClipboardWrite::ctx"][::std::mem::offset_of!(ClipboardWrite, ctx) - 56usize];
    ["Offset of field: ClipboardWrite::reply"]
        [::std::mem::offset_of!(ClipboardWrite, reply) - 64usize];
};
impl Default for ClipboardWrite {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub type TerminalClipboardWriteFn = ::std::option::Option<
    unsafe extern "C" fn(
        terminal: Terminal,
        userdata: *mut ::std::os::raw::c_void,
        write: *const ClipboardWrite,
    ),
>;
pub mod ClipboardReadResult {
    pub type Type = ::std::os::raw::c_int;
    pub const SUCCESS: Type = 0;
    pub const DENIED: Type = 1;
    pub const UNSUPPORTED: Type = 2;
    pub const BUSY: Type = 3;
    pub const IO_ERROR: Type = 4;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ClipboardReadReply {
    pub size: usize,
    pub result: ClipboardReadResult::Type,
    pub contents: *const ClipboardContent,
    pub contents_len: usize,
    pub available: *const String,
    pub available_len: usize,
    pub remember: bool,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of ClipboardReadReply"][::std::mem::size_of::<ClipboardReadReply>() - 56usize];
    ["Alignment of ClipboardReadReply"][::std::mem::align_of::<ClipboardReadReply>() - 8usize];
    ["Offset of field: ClipboardReadReply::size"]
        [::std::mem::offset_of!(ClipboardReadReply, size) - 0usize];
    ["Offset of field: ClipboardReadReply::result"]
        [::std::mem::offset_of!(ClipboardReadReply, result) - 8usize];
    ["Offset of field: ClipboardReadReply::contents"]
        [::std::mem::offset_of!(ClipboardReadReply, contents) - 16usize];
    ["Offset of field: ClipboardReadReply::contents_len"]
        [::std::mem::offset_of!(ClipboardReadReply, contents_len) - 24usize];
    ["Offset of field: ClipboardReadReply::available"]
        [::std::mem::offset_of!(ClipboardReadReply, available) - 32usize];
    ["Offset of field: ClipboardReadReply::available_len"]
        [::std::mem::offset_of!(ClipboardReadReply, available_len) - 40usize];
    ["Offset of field: ClipboardReadReply::remember"]
        [::std::mem::offset_of!(ClipboardReadReply, remember) - 48usize];
};
impl Default for ClipboardReadReply {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub type ClipboardReadReplyFn = ::std::option::Option<
    unsafe extern "C" fn(read: *const ClipboardRead, reply: *const ClipboardReadReply),
>;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ClipboardRead {
    pub size: usize,
    pub location: ClipboardLocation::Type,
    pub mimes: *const String,
    pub mimes_len: usize,
    pub list: bool,
    pub name: String,
    pub granted: bool,
    pub can_remember: bool,
    pub ctx: *const ::std::os::raw::c_void,
    pub reply: ClipboardReadReplyFn,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of ClipboardRead"][::std::mem::size_of::<ClipboardRead>() - 80usize];
    ["Alignment of ClipboardRead"][::std::mem::align_of::<ClipboardRead>() - 8usize];
    ["Offset of field: ClipboardRead::size"][::std::mem::offset_of!(ClipboardRead, size) - 0usize];
    ["Offset of field: ClipboardRead::location"]
        [::std::mem::offset_of!(ClipboardRead, location) - 8usize];
    ["Offset of field: ClipboardRead::mimes"]
        [::std::mem::offset_of!(ClipboardRead, mimes) - 16usize];
    ["Offset of field: ClipboardRead::mimes_len"]
        [::std::mem::offset_of!(ClipboardRead, mimes_len) - 24usize];
    ["Offset of field: ClipboardRead::list"][::std::mem::offset_of!(ClipboardRead, list) - 32usize];
    ["Offset of field: ClipboardRead::name"][::std::mem::offset_of!(ClipboardRead, name) - 40usize];
    ["Offset of field: ClipboardRead::granted"]
        [::std::mem::offset_of!(ClipboardRead, granted) - 56usize];
    ["Offset of field: ClipboardRead::can_remember"]
        [::std::mem::offset_of!(ClipboardRead, can_remember) - 57usize];
    ["Offset of field: ClipboardRead::ctx"][::std::mem::offset_of!(ClipboardRead, ctx) - 64usize];
    ["Offset of field: ClipboardRead::reply"]
        [::std::mem::offset_of!(ClipboardRead, reply) - 72usize];
};
impl Default for ClipboardRead {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub type TerminalClipboardReadFn = ::std::option::Option<
    unsafe extern "C" fn(
        terminal: Terminal,
        userdata: *mut ::std::os::raw::c_void,
        read: *const ClipboardRead,
    ),
>;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TerminalDesktopNotification {
    pub size: usize,
    pub title: String,
    pub body: String,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalDesktopNotification"]
        [::std::mem::size_of::<TerminalDesktopNotification>() - 40usize];
    ["Alignment of TerminalDesktopNotification"]
        [::std::mem::align_of::<TerminalDesktopNotification>() - 8usize];
    ["Offset of field: TerminalDesktopNotification::size"]
        [::std::mem::offset_of!(TerminalDesktopNotification, size) - 0usize];
    ["Offset of field: TerminalDesktopNotification::title"]
        [::std::mem::offset_of!(TerminalDesktopNotification, title) - 8usize];
    ["Offset of field: TerminalDesktopNotification::body"]
        [::std::mem::offset_of!(TerminalDesktopNotification, body) - 24usize];
};
impl Default for TerminalDesktopNotification {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub type TerminalDesktopNotificationFn = ::std::option::Option<
    unsafe extern "C" fn(
        terminal: Terminal,
        userdata: *mut ::std::os::raw::c_void,
        notification: *const TerminalDesktopNotification,
    ),
>;
pub mod TerminalProgressState {
    pub type Type = ::std::os::raw::c_int;
    pub const REMOVE: Type = 0;
    pub const SET: Type = 1;
    pub const ERROR: Type = 2;
    pub const INDETERMINATE: Type = 3;
    pub const PAUSE: Type = 4;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TerminalProgressReport {
    pub size: usize,
    pub state: TerminalProgressState::Type,
    pub progress: i8,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalProgressReport"][::std::mem::size_of::<TerminalProgressReport>() - 16usize];
    ["Alignment of TerminalProgressReport"]
        [::std::mem::align_of::<TerminalProgressReport>() - 8usize];
    ["Offset of field: TerminalProgressReport::size"]
        [::std::mem::offset_of!(TerminalProgressReport, size) - 0usize];
    ["Offset of field: TerminalProgressReport::state"]
        [::std::mem::offset_of!(TerminalProgressReport, state) - 8usize];
    ["Offset of field: TerminalProgressReport::progress"]
        [::std::mem::offset_of!(TerminalProgressReport, progress) - 12usize];
};
impl Default for TerminalProgressReport {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub type TerminalProgressReportFn = ::std::option::Option<
    unsafe extern "C" fn(
        terminal: Terminal,
        userdata: *mut ::std::os::raw::c_void,
        report: *const TerminalProgressReport,
    ),
>;
pub mod ProgramStatusState {
    pub type Type = ::std::os::raw::c_int;
    pub const IDLE: Type = 0;
    pub const WORKING: Type = 1;
    pub const DONE: Type = 2;
    pub const BLOCKED: Type = 3;
    pub const ERROR: Type = 4;
    pub const CLEAR: Type = 5;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod ProgramStatusKind {
    pub type Type = ::std::os::raw::c_int;
    pub const NONE: Type = 0;
    pub const PERMISSION: Type = 1;
    pub const QUESTION: Type = 2;
    pub const AUTH: Type = 3;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TerminalProgramStatus {
    pub size: usize,
    pub state: ProgramStatusState::Type,
    pub kind: ProgramStatusKind::Type,
    pub progress: i8,
    pub id: String,
    pub app: String,
    pub title: String,
    pub message: String,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalProgramStatus"][::std::mem::size_of::<TerminalProgramStatus>() - 88usize];
    ["Alignment of TerminalProgramStatus"]
        [::std::mem::align_of::<TerminalProgramStatus>() - 8usize];
    ["Offset of field: TerminalProgramStatus::size"]
        [::std::mem::offset_of!(TerminalProgramStatus, size) - 0usize];
    ["Offset of field: TerminalProgramStatus::state"]
        [::std::mem::offset_of!(TerminalProgramStatus, state) - 8usize];
    ["Offset of field: TerminalProgramStatus::kind"]
        [::std::mem::offset_of!(TerminalProgramStatus, kind) - 12usize];
    ["Offset of field: TerminalProgramStatus::progress"]
        [::std::mem::offset_of!(TerminalProgramStatus, progress) - 16usize];
    ["Offset of field: TerminalProgramStatus::id"]
        [::std::mem::offset_of!(TerminalProgramStatus, id) - 24usize];
    ["Offset of field: TerminalProgramStatus::app"]
        [::std::mem::offset_of!(TerminalProgramStatus, app) - 40usize];
    ["Offset of field: TerminalProgramStatus::title"]
        [::std::mem::offset_of!(TerminalProgramStatus, title) - 56usize];
    ["Offset of field: TerminalProgramStatus::message"]
        [::std::mem::offset_of!(TerminalProgramStatus, message) - 72usize];
};
impl Default for TerminalProgramStatus {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub type TerminalProgramStatusFn = ::std::option::Option<
    unsafe extern "C" fn(
        terminal: Terminal,
        userdata: *mut ::std::os::raw::c_void,
        report: *const TerminalProgramStatus,
    ),
>;
pub mod SemanticPromptKind {
    pub type Type = ::std::os::raw::c_int;
    pub const INVALID: Type = 0;
    pub const PROMPT_START: Type = 1;
    pub const INPUT_START: Type = 2;
    pub const OUTPUT_START: Type = 3;
    pub const COMMAND_END: Type = 4;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod SemanticPromptPromptKind {
    pub type Type = ::std::os::raw::c_int;
    pub const PRIMARY: Type = 0;
    pub const RIGHT: Type = 1;
    pub const CONTINUATION: Type = 2;
    pub const SECONDARY: Type = 3;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TerminalSemanticPrompt {
    pub size: usize,
    pub kind: SemanticPromptKind::Type,
    pub prompt_kind: SemanticPromptPromptKind::Type,
    pub has_exit_code: bool,
    pub exit_code: i32,
    pub command: String,
    pub error: String,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalSemanticPrompt"][::std::mem::size_of::<TerminalSemanticPrompt>() - 56usize];
    ["Alignment of TerminalSemanticPrompt"]
        [::std::mem::align_of::<TerminalSemanticPrompt>() - 8usize];
    ["Offset of field: TerminalSemanticPrompt::size"]
        [::std::mem::offset_of!(TerminalSemanticPrompt, size) - 0usize];
    ["Offset of field: TerminalSemanticPrompt::kind"]
        [::std::mem::offset_of!(TerminalSemanticPrompt, kind) - 8usize];
    ["Offset of field: TerminalSemanticPrompt::prompt_kind"]
        [::std::mem::offset_of!(TerminalSemanticPrompt, prompt_kind) - 12usize];
    ["Offset of field: TerminalSemanticPrompt::has_exit_code"]
        [::std::mem::offset_of!(TerminalSemanticPrompt, has_exit_code) - 16usize];
    ["Offset of field: TerminalSemanticPrompt::exit_code"]
        [::std::mem::offset_of!(TerminalSemanticPrompt, exit_code) - 20usize];
    ["Offset of field: TerminalSemanticPrompt::command"]
        [::std::mem::offset_of!(TerminalSemanticPrompt, command) - 24usize];
    ["Offset of field: TerminalSemanticPrompt::error"]
        [::std::mem::offset_of!(TerminalSemanticPrompt, error) - 40usize];
};
impl Default for TerminalSemanticPrompt {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub type TerminalSemanticPromptFn = ::std::option::Option<
    unsafe extern "C" fn(
        terminal: Terminal,
        userdata: *mut ::std::os::raw::c_void,
        event: *const TerminalSemanticPrompt,
    ),
>;
pub type TerminalResetFn = ::std::option::Option<
    unsafe extern "C" fn(terminal: Terminal, userdata: *mut ::std::os::raw::c_void),
>;
pub mod TerminalResizeWindowTag {
    pub type Type = ::std::os::raw::c_int;
    pub const GRID: Type = 0;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct TerminalResizeWindowGrid {
    pub rows: u16,
    pub columns: u16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalResizeWindowGrid"]
        [::std::mem::size_of::<TerminalResizeWindowGrid>() - 4usize];
    ["Alignment of TerminalResizeWindowGrid"]
        [::std::mem::align_of::<TerminalResizeWindowGrid>() - 2usize];
    ["Offset of field: TerminalResizeWindowGrid::rows"]
        [::std::mem::offset_of!(TerminalResizeWindowGrid, rows) - 0usize];
    ["Offset of field: TerminalResizeWindowGrid::columns"]
        [::std::mem::offset_of!(TerminalResizeWindowGrid, columns) - 2usize];
};
#[repr(C)]
#[derive(Copy, Clone)]
pub union TerminalResizeWindowValue {
    pub grid: TerminalResizeWindowGrid,
    pub _padding: [u64; 2usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalResizeWindowValue"]
        [::std::mem::size_of::<TerminalResizeWindowValue>() - 16usize];
    ["Alignment of TerminalResizeWindowValue"]
        [::std::mem::align_of::<TerminalResizeWindowValue>() - 8usize];
    ["Offset of field: TerminalResizeWindowValue::grid"]
        [::std::mem::offset_of!(TerminalResizeWindowValue, grid) - 0usize];
    ["Offset of field: TerminalResizeWindowValue::_padding"]
        [::std::mem::offset_of!(TerminalResizeWindowValue, _padding) - 0usize];
};
impl Default for TerminalResizeWindowValue {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Copy, Clone)]
pub struct TerminalResizeWindow {
    pub tag: TerminalResizeWindowTag::Type,
    pub value: TerminalResizeWindowValue,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalResizeWindow"][::std::mem::size_of::<TerminalResizeWindow>() - 24usize];
    ["Alignment of TerminalResizeWindow"][::std::mem::align_of::<TerminalResizeWindow>() - 8usize];
    ["Offset of field: TerminalResizeWindow::tag"]
        [::std::mem::offset_of!(TerminalResizeWindow, tag) - 0usize];
    ["Offset of field: TerminalResizeWindow::value"]
        [::std::mem::offset_of!(TerminalResizeWindow, value) - 8usize];
};
impl Default for TerminalResizeWindow {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub type TerminalResizeWindowFn = ::std::option::Option<
    unsafe extern "C" fn(
        terminal: Terminal,
        userdata: *mut ::std::os::raw::c_void,
        request: *const TerminalResizeWindow,
    ),
>;
pub type TerminalColorSchemeFn = ::std::option::Option<
    unsafe extern "C" fn(
        terminal: Terminal,
        userdata: *mut ::std::os::raw::c_void,
        out_scheme: *mut ColorScheme::Type,
    ) -> bool,
>;
pub type TerminalDeviceAttributesFn = ::std::option::Option<
    unsafe extern "C" fn(
        terminal: Terminal,
        userdata: *mut ::std::os::raw::c_void,
        out_attrs: *mut DeviceAttributes,
    ) -> bool,
>;
pub type TerminalEnquiryFn = ::std::option::Option<
    unsafe extern "C" fn(terminal: Terminal, userdata: *mut ::std::os::raw::c_void) -> String,
>;
pub type TerminalSizeFn = ::std::option::Option<
    unsafe extern "C" fn(
        terminal: Terminal,
        userdata: *mut ::std::os::raw::c_void,
        out_size: *mut SizeReportSize,
    ) -> bool,
>;
pub type TerminalTitleChangedFn = ::std::option::Option<
    unsafe extern "C" fn(terminal: Terminal, userdata: *mut ::std::os::raw::c_void),
>;
pub type TerminalRenderHoldFn = ::std::option::Option<
    unsafe extern "C" fn(terminal: Terminal, userdata: *mut ::std::os::raw::c_void, held: bool),
>;
pub type TerminalPwdChangedFn = ::std::option::Option<
    unsafe extern "C" fn(terminal: Terminal, userdata: *mut ::std::os::raw::c_void),
>;
pub type TerminalWritePtyFn = ::std::option::Option<
    unsafe extern "C" fn(
        terminal: Terminal,
        userdata: *mut ::std::os::raw::c_void,
        data: *const u8,
        len: usize,
    ),
>;
pub type TerminalXtversionFn = ::std::option::Option<
    unsafe extern "C" fn(terminal: Terminal, userdata: *mut ::std::os::raw::c_void) -> String,
>;
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct TerminalModeConfig {
    pub mode: Mode,
    pub value: bool,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of TerminalModeConfig"][::std::mem::size_of::<TerminalModeConfig>() - 4usize];
    ["Alignment of TerminalModeConfig"][::std::mem::align_of::<TerminalModeConfig>() - 2usize];
    ["Offset of field: TerminalModeConfig::mode"]
        [::std::mem::offset_of!(TerminalModeConfig, mode) - 0usize];
    ["Offset of field: TerminalModeConfig::value"]
        [::std::mem::offset_of!(TerminalModeConfig, value) - 2usize];
};
pub mod TerminalOption {
    pub type Type = ::std::os::raw::c_int;
    pub const USERDATA: Type = 0;
    pub const WRITE_PTY: Type = 1;
    pub const BELL: Type = 2;
    pub const ENQUIRY: Type = 3;
    pub const XTVERSION: Type = 4;
    pub const TITLE_CHANGED: Type = 5;
    pub const SIZE: Type = 6;
    pub const COLOR_SCHEME: Type = 7;
    pub const DEVICE_ATTRIBUTES: Type = 8;
    pub const TITLE: Type = 9;
    pub const PWD: Type = 10;
    pub const COLOR_FOREGROUND: Type = 11;
    pub const COLOR_BACKGROUND: Type = 12;
    pub const COLOR_CURSOR: Type = 13;
    pub const COLOR_PALETTE: Type = 14;
    pub const KITTY_IMAGE_STORAGE_LIMIT: Type = 15;
    pub const KITTY_IMAGE_MEDIUM_FILE: Type = 16;
    pub const KITTY_IMAGE_MEDIUM_TEMP_FILE: Type = 17;
    pub const KITTY_IMAGE_MEDIUM_SHARED_MEM: Type = 18;
    pub const APC_MAX_BYTES: Type = 19;
    pub const APC_MAX_BYTES_KITTY: Type = 20;
    pub const SELECTION: Type = 21;
    pub const DEFAULT_CURSOR_STYLE: Type = 22;
    pub const DEFAULT_CURSOR_BLINK: Type = 23;
    pub const GLYPH_PROTOCOL: Type = 24;
    pub const PWD_CHANGED: Type = 25;
    pub const CLIPBOARD_WRITE: Type = 26;
    pub const SCROLLBACK_MAX_BYTES: Type = 27;
    pub const SCROLLBACK_MAX_LINES: Type = 28;
    pub const DESKTOP_NOTIFICATION: Type = 29;
    pub const PROGRESS_REPORT: Type = 30;
    pub const CONTINUATION_MAX_BYTES: Type = 31;
    pub const TITLE_REPORT: Type = 32;
    pub const MODE_DEFAULT: Type = 33;
    pub const MODE: Type = 34;
    pub const UNKNOWN_SEQUENCE: Type = 35;
    pub const UNKNOWN_MAX_BYTES: Type = 36;
    pub const TERMINFO_NAME: Type = 37;
    pub const CLIPBOARD_READ: Type = 38;
    pub const CLIPBOARD_WRITE_MAX_BYTES: Type = 39;
    pub const RESIZE_PULL_SCROLLBACK: Type = 40;
    pub const RENDER_HOLD: Type = 41;
    pub const SEMANTIC_PROMPT: Type = 42;
    pub const RESET: Type = 43;
    pub const XT_CHECKSUM_REPORT: Type = 44;
    pub const XT_CHECKSUM_EXTENSION: Type = 45;
    pub const PROGRAM_STATUS: Type = 46;
    pub const RESIZE_WINDOW: Type = 47;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod TerminalData {
    pub type Type = ::std::os::raw::c_int;
    pub const INVALID: Type = 0;
    pub const COLS: Type = 1;
    pub const ROWS: Type = 2;
    pub const CURSOR_X: Type = 3;
    pub const CURSOR_Y: Type = 4;
    pub const CURSOR_PENDING_WRAP: Type = 5;
    pub const ACTIVE_SCREEN: Type = 6;
    pub const CURSOR_VISIBLE: Type = 7;
    pub const KITTY_KEYBOARD_FLAGS: Type = 8;
    pub const SCROLLBAR: Type = 9;
    pub const CURSOR_STYLE: Type = 10;
    pub const MOUSE_TRACKING: Type = 11;
    pub const TITLE: Type = 12;
    pub const PWD: Type = 13;
    pub const TOTAL_ROWS: Type = 14;
    pub const SCROLLBACK_ROWS: Type = 15;
    pub const WIDTH_PX: Type = 16;
    pub const HEIGHT_PX: Type = 17;
    pub const COLOR_FOREGROUND: Type = 18;
    pub const COLOR_BACKGROUND: Type = 19;
    pub const COLOR_CURSOR: Type = 20;
    pub const COLOR_PALETTE: Type = 21;
    pub const COLOR_FOREGROUND_DEFAULT: Type = 22;
    pub const COLOR_BACKGROUND_DEFAULT: Type = 23;
    pub const COLOR_CURSOR_DEFAULT: Type = 24;
    pub const COLOR_PALETTE_DEFAULT: Type = 25;
    pub const KITTY_IMAGE_STORAGE_LIMIT: Type = 26;
    pub const KITTY_IMAGE_MEDIUM_FILE: Type = 27;
    pub const KITTY_IMAGE_MEDIUM_TEMP_FILE: Type = 28;
    pub const KITTY_IMAGE_MEDIUM_SHARED_MEM: Type = 29;
    pub const KITTY_GRAPHICS: Type = 30;
    pub const SELECTION: Type = 31;
    pub const VIEWPORT_ACTIVE: Type = 32;
    pub const VT_PROCESSING_ERROR: Type = 33;
    pub const SCROLLBACK_MAX_BYTES: Type = 34;
    pub const SCROLLBACK_MAX_LINES: Type = 35;
    pub const CONTINUATION_MAX_BYTES: Type = 36;
    pub const MODE: Type = 37;
    pub const VT_GROUND: Type = 38;
    pub const CURSOR_AT_PROMPT: Type = 39;
    pub const CLIPBOARD_WRITE_MAX_BYTES: Type = 40;
    pub const MOUSE_SHAPE: Type = 41;
    pub const MEMORY_USAGE: Type = 42;
    pub const MAX_VALUE: Type = 2147483647;
}
unsafe extern "C" {
    pub fn ghostty_terminal_new(
        allocator: *const Allocator,
        terminal: *mut Terminal,
        cols: u16,
        rows: u16,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_free(terminal: Terminal);
}
unsafe extern "C" {
    pub fn ghostty_terminal_clear_screen(terminal: Terminal) -> bool;
}
unsafe extern "C" {
    pub fn ghostty_terminal_reset(terminal: Terminal);
}
unsafe extern "C" {
    pub fn ghostty_terminal_resize(
        terminal: Terminal,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_set(
        terminal: Terminal,
        option: TerminalOption::Type,
        value: *const ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_vt_write(terminal: Terminal, data: *const u8, len: usize);
}
unsafe extern "C" {
    pub fn ghostty_terminal_vt_write_until(
        terminal: Terminal,
        data: *const u8,
        len: usize,
        boundaries: u32,
        out: *mut TerminalWriteStop,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_continuation_write(terminal: Terminal, writer: Writer) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_continuation_buf(
        terminal: Terminal,
        buf: *mut u8,
        buf_len: usize,
        out_written: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_continuation_alloc(
        terminal: Terminal,
        allocator: *const Allocator,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_scroll_viewport(terminal: Terminal, behavior: TerminalScrollViewport);
}
unsafe extern "C" {
    pub fn ghostty_terminal_compression_activity(
        terminal: Terminal,
        out_activity: *mut u64,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_compress(
        terminal: Terminal,
        mode: TerminalCompressionMode::Type,
        out_result: *mut TerminalCompressionResult::Type,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_get(
        terminal: Terminal,
        data: TerminalData::Type,
        out: *mut ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_get_multi(
        terminal: Terminal,
        count: usize,
        keys: *const TerminalData::Type,
        values: *mut *mut ::std::os::raw::c_void,
        out_written: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_grid_ref(
        terminal: Terminal,
        point: Point,
        out_ref: *mut GridRef,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_grid_ref_track(
        terminal: Terminal,
        point: Point,
        out_ref: *mut TrackedGridRef,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_terminal_point_from_grid_ref(
        terminal: Terminal,
        ref_: *const GridRef,
        tag: PointTag::Type,
        out: *mut PointCoordinate,
    ) -> Result::Type;
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct FormatterScreenExtra {
    pub size: usize,
    pub cursor: bool,
    pub style: bool,
    pub hyperlink: bool,
    pub protection: bool,
    pub kitty_keyboard: bool,
    pub charsets: bool,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of FormatterScreenExtra"][::std::mem::size_of::<FormatterScreenExtra>() - 16usize];
    ["Alignment of FormatterScreenExtra"][::std::mem::align_of::<FormatterScreenExtra>() - 8usize];
    ["Offset of field: FormatterScreenExtra::size"]
        [::std::mem::offset_of!(FormatterScreenExtra, size) - 0usize];
    ["Offset of field: FormatterScreenExtra::cursor"]
        [::std::mem::offset_of!(FormatterScreenExtra, cursor) - 8usize];
    ["Offset of field: FormatterScreenExtra::style"]
        [::std::mem::offset_of!(FormatterScreenExtra, style) - 9usize];
    ["Offset of field: FormatterScreenExtra::hyperlink"]
        [::std::mem::offset_of!(FormatterScreenExtra, hyperlink) - 10usize];
    ["Offset of field: FormatterScreenExtra::protection"]
        [::std::mem::offset_of!(FormatterScreenExtra, protection) - 11usize];
    ["Offset of field: FormatterScreenExtra::kitty_keyboard"]
        [::std::mem::offset_of!(FormatterScreenExtra, kitty_keyboard) - 12usize];
    ["Offset of field: FormatterScreenExtra::charsets"]
        [::std::mem::offset_of!(FormatterScreenExtra, charsets) - 13usize];
};
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct FormatterTerminalExtra {
    pub size: usize,
    pub palette: bool,
    pub modes: bool,
    pub scrolling_region: bool,
    pub tabstops: bool,
    pub pwd: bool,
    pub keyboard: bool,
    pub screen: FormatterScreenExtra,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of FormatterTerminalExtra"][::std::mem::size_of::<FormatterTerminalExtra>() - 32usize];
    ["Alignment of FormatterTerminalExtra"]
        [::std::mem::align_of::<FormatterTerminalExtra>() - 8usize];
    ["Offset of field: FormatterTerminalExtra::size"]
        [::std::mem::offset_of!(FormatterTerminalExtra, size) - 0usize];
    ["Offset of field: FormatterTerminalExtra::palette"]
        [::std::mem::offset_of!(FormatterTerminalExtra, palette) - 8usize];
    ["Offset of field: FormatterTerminalExtra::modes"]
        [::std::mem::offset_of!(FormatterTerminalExtra, modes) - 9usize];
    ["Offset of field: FormatterTerminalExtra::scrolling_region"]
        [::std::mem::offset_of!(FormatterTerminalExtra, scrolling_region) - 10usize];
    ["Offset of field: FormatterTerminalExtra::tabstops"]
        [::std::mem::offset_of!(FormatterTerminalExtra, tabstops) - 11usize];
    ["Offset of field: FormatterTerminalExtra::pwd"]
        [::std::mem::offset_of!(FormatterTerminalExtra, pwd) - 12usize];
    ["Offset of field: FormatterTerminalExtra::keyboard"]
        [::std::mem::offset_of!(FormatterTerminalExtra, keyboard) - 13usize];
    ["Offset of field: FormatterTerminalExtra::screen"]
        [::std::mem::offset_of!(FormatterTerminalExtra, screen) - 16usize];
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct FormatterTerminalOptions {
    pub size: usize,
    pub emit: FormatterFormat::Type,
    pub unwrap: bool,
    pub trim: bool,
    pub extra: FormatterTerminalExtra,
    pub selection: *const Selection,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of FormatterTerminalOptions"]
        [::std::mem::size_of::<FormatterTerminalOptions>() - 56usize];
    ["Alignment of FormatterTerminalOptions"]
        [::std::mem::align_of::<FormatterTerminalOptions>() - 8usize];
    ["Offset of field: FormatterTerminalOptions::size"]
        [::std::mem::offset_of!(FormatterTerminalOptions, size) - 0usize];
    ["Offset of field: FormatterTerminalOptions::emit"]
        [::std::mem::offset_of!(FormatterTerminalOptions, emit) - 8usize];
    ["Offset of field: FormatterTerminalOptions::unwrap"]
        [::std::mem::offset_of!(FormatterTerminalOptions, unwrap) - 12usize];
    ["Offset of field: FormatterTerminalOptions::trim"]
        [::std::mem::offset_of!(FormatterTerminalOptions, trim) - 13usize];
    ["Offset of field: FormatterTerminalOptions::extra"]
        [::std::mem::offset_of!(FormatterTerminalOptions, extra) - 16usize];
    ["Offset of field: FormatterTerminalOptions::selection"]
        [::std::mem::offset_of!(FormatterTerminalOptions, selection) - 48usize];
};
impl Default for FormatterTerminalOptions {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
unsafe extern "C" {
    pub fn ghostty_formatter_terminal_new(
        allocator: *const Allocator,
        formatter: *mut Formatter,
        terminal: Terminal,
        options: FormatterTerminalOptions,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_formatter_format(formatter: Formatter, writer: Writer) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_formatter_format_buf(
        formatter: Formatter,
        buf: *mut u8,
        buf_len: usize,
        out_written: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_formatter_format_alloc(
        formatter: Formatter,
        allocator: *const Allocator,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_formatter_free(formatter: Formatter);
}
pub mod RenderStateDirty {
    pub type Type = ::std::os::raw::c_int;
    pub const FALSE: Type = 0;
    pub const PARTIAL: Type = 1;
    pub const FULL: Type = 2;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod RenderStateCursorVisualStyle {
    pub type Type = ::std::os::raw::c_int;
    pub const BAR: Type = 0;
    pub const BLOCK: Type = 1;
    pub const UNDERLINE: Type = 2;
    pub const BLOCK_HOLLOW: Type = 3;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct RenderStateOverscan {
    pub above: u16,
    pub below: u16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of RenderStateOverscan"][::std::mem::size_of::<RenderStateOverscan>() - 4usize];
    ["Alignment of RenderStateOverscan"][::std::mem::align_of::<RenderStateOverscan>() - 2usize];
    ["Offset of field: RenderStateOverscan::above"]
        [::std::mem::offset_of!(RenderStateOverscan, above) - 0usize];
    ["Offset of field: RenderStateOverscan::below"]
        [::std::mem::offset_of!(RenderStateOverscan, below) - 2usize];
};
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct RenderStateRowId {
    pub bits: [u64; 2usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of RenderStateRowId"][::std::mem::size_of::<RenderStateRowId>() - 16usize];
    ["Alignment of RenderStateRowId"][::std::mem::align_of::<RenderStateRowId>() - 8usize];
    ["Offset of field: RenderStateRowId::bits"]
        [::std::mem::offset_of!(RenderStateRowId, bits) - 0usize];
};
pub mod RenderStateData {
    pub type Type = ::std::os::raw::c_int;
    pub const INVALID: Type = 0;
    pub const COLS: Type = 1;
    pub const ROWS: Type = 2;
    pub const DIRTY: Type = 3;
    pub const ROW_ITERATOR: Type = 4;
    pub const COLOR_BACKGROUND: Type = 5;
    pub const COLOR_FOREGROUND: Type = 6;
    pub const COLOR_CURSOR: Type = 7;
    pub const COLOR_CURSOR_HAS_VALUE: Type = 8;
    pub const COLOR_PALETTE: Type = 9;
    pub const CURSOR_VISUAL_STYLE: Type = 10;
    pub const CURSOR_VISIBLE: Type = 11;
    pub const CURSOR_BLINKING: Type = 12;
    pub const CURSOR_PASSWORD_INPUT: Type = 13;
    pub const CURSOR_VIEWPORT_HAS_VALUE: Type = 14;
    pub const CURSOR_VIEWPORT_X: Type = 15;
    pub const CURSOR_VIEWPORT_Y: Type = 16;
    pub const CURSOR_VIEWPORT_WIDE_TAIL: Type = 17;
    pub const CURSOR: Type = 18;
    pub const COLORS: Type = 19;
    pub const OVERSCAN: Type = 20;
    pub const OVERSCAN_REQUEST: Type = 21;
    pub const IMAGE_ITERATOR: Type = 22;
    pub const IMAGE_PLACEMENTS_GENERATION: Type = 23;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod RenderStateOption {
    pub type Type = ::std::os::raw::c_int;
    pub const DIRTY: Type = 0;
    pub const OVERSCAN: Type = 1;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod ImageProtocol {
    pub type Type = ::std::os::raw::c_int;
    pub const KITTY: Type = 0;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod RenderStateImageLayer {
    pub type Type = ::std::os::raw::c_int;
    pub const BELOW_BACKGROUND: Type = 0;
    pub const BELOW_TEXT: Type = 1;
    pub const ABOVE_TEXT: Type = 2;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod RenderStateKittyPlacementKind {
    pub type Type = ::std::os::raw::c_int;
    pub const VIRTUAL: Type = 0;
    pub const RELATIVE: Type = 1;
    pub const FIXED: Type = 2;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod RenderStateImageData {
    pub type Type = ::std::os::raw::c_int;
    pub const INVALID: Type = 0;
    pub const PROTOCOL: Type = 1;
    pub const IMAGE_ID: Type = 2;
    pub const IMAGE_GENERATION: Type = 3;
    pub const LAYER: Type = 4;
    pub const Z: Type = 5;
    pub const VIEWPORT_COL: Type = 6;
    pub const VIEWPORT_ROW: Type = 7;
    pub const GRID_COLS: Type = 8;
    pub const GRID_ROWS: Type = 9;
    pub const CELL_OFFSET_X: Type = 10;
    pub const CELL_OFFSET_Y: Type = 11;
    pub const PIXEL_WIDTH: Type = 12;
    pub const PIXEL_HEIGHT: Type = 13;
    pub const SOURCE_X: Type = 14;
    pub const SOURCE_Y: Type = 15;
    pub const SOURCE_WIDTH: Type = 16;
    pub const SOURCE_HEIGHT: Type = 17;
    pub const KITTY_PLACEMENT_ID: Type = 18;
    pub const KITTY_KIND: Type = 19;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod RenderStateRowData {
    pub type Type = ::std::os::raw::c_int;
    pub const INVALID: Type = 0;
    pub const DIRTY: Type = 1;
    pub const RAW: Type = 2;
    pub const CELLS: Type = 3;
    pub const SELECTION: Type = 4;
    pub const CELLS_RAW: Type = 5;
    pub const VIEWPORT_Y: Type = 6;
    pub const ID: Type = 7;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod RenderStateRowOption {
    pub type Type = ::std::os::raw::c_int;
    pub const DIRTY: Type = 0;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct RenderStateRowSelection {
    pub size: usize,
    pub start_x: u16,
    pub end_x: u16,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of RenderStateRowSelection"][::std::mem::size_of::<RenderStateRowSelection>() - 16usize];
    ["Alignment of RenderStateRowSelection"]
        [::std::mem::align_of::<RenderStateRowSelection>() - 8usize];
    ["Offset of field: RenderStateRowSelection::size"]
        [::std::mem::offset_of!(RenderStateRowSelection, size) - 0usize];
    ["Offset of field: RenderStateRowSelection::start_x"]
        [::std::mem::offset_of!(RenderStateRowSelection, start_x) - 8usize];
    ["Offset of field: RenderStateRowSelection::end_x"]
        [::std::mem::offset_of!(RenderStateRowSelection, end_x) - 10usize];
};
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct RenderStateCursor {
    pub size: usize,
    pub viewport_has_value: bool,
    pub viewport_x: u16,
    pub viewport_y: u16,
    pub wide_tail: bool,
    pub visible: bool,
    pub blinking: bool,
    pub password_input: bool,
    pub visual_style: RenderStateCursorVisualStyle::Type,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of RenderStateCursor"][::std::mem::size_of::<RenderStateCursor>() - 24usize];
    ["Alignment of RenderStateCursor"][::std::mem::align_of::<RenderStateCursor>() - 8usize];
    ["Offset of field: RenderStateCursor::size"]
        [::std::mem::offset_of!(RenderStateCursor, size) - 0usize];
    ["Offset of field: RenderStateCursor::viewport_has_value"]
        [::std::mem::offset_of!(RenderStateCursor, viewport_has_value) - 8usize];
    ["Offset of field: RenderStateCursor::viewport_x"]
        [::std::mem::offset_of!(RenderStateCursor, viewport_x) - 10usize];
    ["Offset of field: RenderStateCursor::viewport_y"]
        [::std::mem::offset_of!(RenderStateCursor, viewport_y) - 12usize];
    ["Offset of field: RenderStateCursor::wide_tail"]
        [::std::mem::offset_of!(RenderStateCursor, wide_tail) - 14usize];
    ["Offset of field: RenderStateCursor::visible"]
        [::std::mem::offset_of!(RenderStateCursor, visible) - 15usize];
    ["Offset of field: RenderStateCursor::blinking"]
        [::std::mem::offset_of!(RenderStateCursor, blinking) - 16usize];
    ["Offset of field: RenderStateCursor::password_input"]
        [::std::mem::offset_of!(RenderStateCursor, password_input) - 17usize];
    ["Offset of field: RenderStateCursor::visual_style"]
        [::std::mem::offset_of!(RenderStateCursor, visual_style) - 20usize];
};
impl Default for RenderStateCursor {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct RenderStateColors {
    pub size: usize,
    pub background: ColorRgb,
    pub foreground: ColorRgb,
    pub cursor: ColorRgb,
    pub cursor_has_value: bool,
    pub palette: [ColorRgb; 256usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of RenderStateColors"][::std::mem::size_of::<RenderStateColors>() - 792usize];
    ["Alignment of RenderStateColors"][::std::mem::align_of::<RenderStateColors>() - 8usize];
    ["Offset of field: RenderStateColors::size"]
        [::std::mem::offset_of!(RenderStateColors, size) - 0usize];
    ["Offset of field: RenderStateColors::background"]
        [::std::mem::offset_of!(RenderStateColors, background) - 8usize];
    ["Offset of field: RenderStateColors::foreground"]
        [::std::mem::offset_of!(RenderStateColors, foreground) - 11usize];
    ["Offset of field: RenderStateColors::cursor"]
        [::std::mem::offset_of!(RenderStateColors, cursor) - 14usize];
    ["Offset of field: RenderStateColors::cursor_has_value"]
        [::std::mem::offset_of!(RenderStateColors, cursor_has_value) - 17usize];
    ["Offset of field: RenderStateColors::palette"]
        [::std::mem::offset_of!(RenderStateColors, palette) - 18usize];
};
impl Default for RenderStateColors {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
unsafe extern "C" {
    pub fn ghostty_render_state_new(
        allocator: *const Allocator,
        state: *mut RenderState,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_free(state: RenderState);
}
unsafe extern "C" {
    pub fn ghostty_render_state_update(state: RenderState, terminal: Terminal) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_begin_update(
        state: RenderState,
        terminal: Terminal,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_end_update(state: RenderState) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_clean(state: RenderState) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_get(
        state: RenderState,
        data: RenderStateData::Type,
        out: *mut ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_get_multi(
        state: RenderState,
        count: usize,
        keys: *const RenderStateData::Type,
        values: *mut *mut ::std::os::raw::c_void,
        out_written: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_set(
        state: RenderState,
        option: RenderStateOption::Type,
        value: *const ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_row_iterator_new(
        allocator: *const Allocator,
        out_iterator: *mut RenderStateRowIterator,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_row_iterator_free(iterator: RenderStateRowIterator);
}
unsafe extern "C" {
    pub fn ghostty_render_state_row_iterator_next(iterator: RenderStateRowIterator) -> bool;
}
unsafe extern "C" {
    pub fn ghostty_render_state_row_iterator_next_dirty(
        iterator: RenderStateRowIterator,
        out_y: *mut u16,
    ) -> bool;
}
unsafe extern "C" {
    pub fn ghostty_render_state_row_get(
        iterator: RenderStateRowIterator,
        data: RenderStateRowData::Type,
        out: *mut ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_row_get_multi(
        iterator: RenderStateRowIterator,
        count: usize,
        keys: *const RenderStateRowData::Type,
        values: *mut *mut ::std::os::raw::c_void,
        out_written: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_row_set(
        iterator: RenderStateRowIterator,
        option: RenderStateRowOption::Type,
        value: *const ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_row_cells_new(
        allocator: *const Allocator,
        out_cells: *mut RenderStateRowCells,
    ) -> Result::Type;
}
pub mod RenderStateRowCellsData {
    pub type Type = ::std::os::raw::c_int;
    pub const INVALID: Type = 0;
    pub const RAW: Type = 1;
    pub const STYLE: Type = 2;
    pub const GRAPHEMES_LEN: Type = 3;
    pub const GRAPHEMES_BUF: Type = 4;
    pub const BG_COLOR: Type = 5;
    pub const FG_COLOR: Type = 6;
    pub const SELECTED: Type = 7;
    pub const HAS_STYLING: Type = 8;
    pub const GRAPHEMES_UTF8: Type = 9;
    pub const MAX_VALUE: Type = 2147483647;
}
unsafe extern "C" {
    pub fn ghostty_render_state_row_cells_next(cells: RenderStateRowCells) -> bool;
}
unsafe extern "C" {
    pub fn ghostty_render_state_row_cells_select(
        cells: RenderStateRowCells,
        x: u16,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_row_cells_get(
        cells: RenderStateRowCells,
        data: RenderStateRowCellsData::Type,
        out: *mut ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_row_cells_get_multi(
        cells: RenderStateRowCells,
        count: usize,
        keys: *const RenderStateRowCellsData::Type,
        values: *mut *mut ::std::os::raw::c_void,
        out_written: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_row_cells_free(cells: RenderStateRowCells);
}
unsafe extern "C" {
    pub fn ghostty_render_state_image_iterator_new(
        allocator: *const Allocator,
        out_iterator: *mut RenderStateImageIterator,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_image_iterator_free(iterator: RenderStateImageIterator);
}
unsafe extern "C" {
    pub fn ghostty_render_state_image_iterator_next(iterator: RenderStateImageIterator) -> bool;
}
unsafe extern "C" {
    pub fn ghostty_render_state_image_get(
        iterator: RenderStateImageIterator,
        data: RenderStateImageData::Type,
        out: *mut ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_render_state_image_get_multi(
        iterator: RenderStateImageIterator,
        count: usize,
        keys: *const RenderStateImageData::Type,
        values: *mut *mut ::std::os::raw::c_void,
        out_written: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_tracked_grid_ref_free(ref_: TrackedGridRef);
}
unsafe extern "C" {
    pub fn ghostty_tracked_grid_ref_has_value(ref_: TrackedGridRef) -> bool;
}
unsafe extern "C" {
    pub fn ghostty_tracked_grid_ref_point(
        ref_: TrackedGridRef,
        tag: PointTag::Type,
        out_point: *mut PointCoordinate,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_tracked_grid_ref_set(
        ref_: TrackedGridRef,
        terminal: Terminal,
        point: Point,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_tracked_grid_ref_snapshot(
        ref_: TrackedGridRef,
        out_ref: *mut GridRef,
    ) -> Result::Type;
}
pub mod SgrAttributeTag {
    pub type Type = ::std::os::raw::c_int;
    pub const UNSET: Type = 0;
    pub const UNKNOWN: Type = 1;
    pub const BOLD: Type = 2;
    pub const RESET_BOLD: Type = 3;
    pub const ITALIC: Type = 4;
    pub const RESET_ITALIC: Type = 5;
    pub const FAINT: Type = 6;
    pub const UNDERLINE: Type = 7;
    pub const UNDERLINE_COLOR: Type = 8;
    pub const UNDERLINE_COLOR_256: Type = 9;
    pub const RESET_UNDERLINE_COLOR: Type = 10;
    pub const OVERLINE: Type = 11;
    pub const RESET_OVERLINE: Type = 12;
    pub const BLINK: Type = 13;
    pub const RESET_BLINK: Type = 14;
    pub const INVERSE: Type = 15;
    pub const RESET_INVERSE: Type = 16;
    pub const INVISIBLE: Type = 17;
    pub const RESET_INVISIBLE: Type = 18;
    pub const STRIKETHROUGH: Type = 19;
    pub const RESET_STRIKETHROUGH: Type = 20;
    pub const DIRECT_COLOR_FG: Type = 21;
    pub const DIRECT_COLOR_BG: Type = 22;
    pub const BG_8: Type = 23;
    pub const FG_8: Type = 24;
    pub const RESET_FG: Type = 25;
    pub const RESET_BG: Type = 26;
    pub const BRIGHT_BG_8: Type = 27;
    pub const BRIGHT_FG_8: Type = 28;
    pub const BG_256: Type = 29;
    pub const FG_256: Type = 30;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod SgrUnderline {
    pub type Type = ::std::os::raw::c_int;
    pub const NONE: Type = 0;
    pub const SINGLE: Type = 1;
    pub const DOUBLE: Type = 2;
    pub const CURLY: Type = 3;
    pub const DOTTED: Type = 4;
    pub const DASHED: Type = 5;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SgrUnknown {
    pub full_ptr: *const u16,
    pub full_len: usize,
    pub partial_ptr: *const u16,
    pub partial_len: usize,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of SgrUnknown"][::std::mem::size_of::<SgrUnknown>() - 32usize];
    ["Alignment of SgrUnknown"][::std::mem::align_of::<SgrUnknown>() - 8usize];
    ["Offset of field: SgrUnknown::full_ptr"]
        [::std::mem::offset_of!(SgrUnknown, full_ptr) - 0usize];
    ["Offset of field: SgrUnknown::full_len"]
        [::std::mem::offset_of!(SgrUnknown, full_len) - 8usize];
    ["Offset of field: SgrUnknown::partial_ptr"]
        [::std::mem::offset_of!(SgrUnknown, partial_ptr) - 16usize];
    ["Offset of field: SgrUnknown::partial_len"]
        [::std::mem::offset_of!(SgrUnknown, partial_len) - 24usize];
};
impl Default for SgrUnknown {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union SgrAttributeValue {
    pub unknown: SgrUnknown,
    pub underline: SgrUnderline::Type,
    pub underline_color: ColorRgb,
    pub underline_color_256: ColorPaletteIndex,
    pub direct_color_fg: ColorRgb,
    pub direct_color_bg: ColorRgb,
    pub bg_8: ColorPaletteIndex,
    pub fg_8: ColorPaletteIndex,
    pub bright_bg_8: ColorPaletteIndex,
    pub bright_fg_8: ColorPaletteIndex,
    pub bg_256: ColorPaletteIndex,
    pub fg_256: ColorPaletteIndex,
    pub _padding: [u64; 8usize],
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of SgrAttributeValue"][::std::mem::size_of::<SgrAttributeValue>() - 64usize];
    ["Alignment of SgrAttributeValue"][::std::mem::align_of::<SgrAttributeValue>() - 8usize];
    ["Offset of field: SgrAttributeValue::unknown"]
        [::std::mem::offset_of!(SgrAttributeValue, unknown) - 0usize];
    ["Offset of field: SgrAttributeValue::underline"]
        [::std::mem::offset_of!(SgrAttributeValue, underline) - 0usize];
    ["Offset of field: SgrAttributeValue::underline_color"]
        [::std::mem::offset_of!(SgrAttributeValue, underline_color) - 0usize];
    ["Offset of field: SgrAttributeValue::underline_color_256"]
        [::std::mem::offset_of!(SgrAttributeValue, underline_color_256) - 0usize];
    ["Offset of field: SgrAttributeValue::direct_color_fg"]
        [::std::mem::offset_of!(SgrAttributeValue, direct_color_fg) - 0usize];
    ["Offset of field: SgrAttributeValue::direct_color_bg"]
        [::std::mem::offset_of!(SgrAttributeValue, direct_color_bg) - 0usize];
    ["Offset of field: SgrAttributeValue::bg_8"]
        [::std::mem::offset_of!(SgrAttributeValue, bg_8) - 0usize];
    ["Offset of field: SgrAttributeValue::fg_8"]
        [::std::mem::offset_of!(SgrAttributeValue, fg_8) - 0usize];
    ["Offset of field: SgrAttributeValue::bright_bg_8"]
        [::std::mem::offset_of!(SgrAttributeValue, bright_bg_8) - 0usize];
    ["Offset of field: SgrAttributeValue::bright_fg_8"]
        [::std::mem::offset_of!(SgrAttributeValue, bright_fg_8) - 0usize];
    ["Offset of field: SgrAttributeValue::bg_256"]
        [::std::mem::offset_of!(SgrAttributeValue, bg_256) - 0usize];
    ["Offset of field: SgrAttributeValue::fg_256"]
        [::std::mem::offset_of!(SgrAttributeValue, fg_256) - 0usize];
    ["Offset of field: SgrAttributeValue::_padding"]
        [::std::mem::offset_of!(SgrAttributeValue, _padding) - 0usize];
};
impl Default for SgrAttributeValue {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
#[repr(C)]
#[derive(Copy, Clone)]
pub struct SgrAttribute {
    pub tag: SgrAttributeTag::Type,
    pub value: SgrAttributeValue,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of SgrAttribute"][::std::mem::size_of::<SgrAttribute>() - 72usize];
    ["Alignment of SgrAttribute"][::std::mem::align_of::<SgrAttribute>() - 8usize];
    ["Offset of field: SgrAttribute::tag"][::std::mem::offset_of!(SgrAttribute, tag) - 0usize];
    ["Offset of field: SgrAttribute::value"][::std::mem::offset_of!(SgrAttribute, value) - 8usize];
};
impl Default for SgrAttribute {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
unsafe extern "C" {
    pub fn ghostty_sgr_new(allocator: *const Allocator, parser: *mut SgrParser) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_sgr_free(parser: SgrParser);
}
unsafe extern "C" {
    pub fn ghostty_sgr_reset(parser: SgrParser);
}
unsafe extern "C" {
    pub fn ghostty_sgr_set_params(
        parser: SgrParser,
        params: *const u16,
        separators: *const ::std::os::raw::c_char,
        len: usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_sgr_next(parser: SgrParser, attr: *mut SgrAttribute) -> bool;
}
unsafe extern "C" {
    pub fn ghostty_sgr_unknown_full(unknown: SgrUnknown, ptr: *mut *const u16) -> usize;
}
unsafe extern "C" {
    pub fn ghostty_sgr_unknown_partial(unknown: SgrUnknown, ptr: *mut *const u16) -> usize;
}
unsafe extern "C" {
    pub fn ghostty_sgr_attribute_tag(attr: SgrAttribute) -> SgrAttributeTag::Type;
}
unsafe extern "C" {
    pub fn ghostty_sgr_attribute_value(attr: *mut SgrAttribute) -> *mut SgrAttributeValue;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SysImage {
    pub width: u32,
    pub height: u32,
    pub data: *mut u8,
    pub data_len: usize,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of SysImage"][::std::mem::size_of::<SysImage>() - 24usize];
    ["Alignment of SysImage"][::std::mem::align_of::<SysImage>() - 8usize];
    ["Offset of field: SysImage::width"][::std::mem::offset_of!(SysImage, width) - 0usize];
    ["Offset of field: SysImage::height"][::std::mem::offset_of!(SysImage, height) - 4usize];
    ["Offset of field: SysImage::data"][::std::mem::offset_of!(SysImage, data) - 8usize];
    ["Offset of field: SysImage::data_len"][::std::mem::offset_of!(SysImage, data_len) - 16usize];
};
impl Default for SysImage {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
pub mod SysLogLevel {
    pub type Type = ::std::os::raw::c_int;
    pub const ERROR: Type = 0;
    pub const WARNING: Type = 1;
    pub const INFO: Type = 2;
    pub const DEBUG: Type = 3;
    pub const MAX_VALUE: Type = 2147483647;
}
pub type SysLogFn = ::std::option::Option<
    unsafe extern "C" fn(
        userdata: *mut ::std::os::raw::c_void,
        level: SysLogLevel::Type,
        scope: *const u8,
        scope_len: usize,
        message: *const u8,
        message_len: usize,
    ),
>;
pub type SysDecodePngFn = ::std::option::Option<
    unsafe extern "C" fn(
        userdata: *mut ::std::os::raw::c_void,
        allocator: *const Allocator,
        data: *const u8,
        data_len: usize,
        out: *mut SysImage,
    ) -> bool,
>;
pub type SysRandomSecureFn = ::std::option::Option<
    unsafe extern "C" fn(userdata: *mut ::std::os::raw::c_void, buf: *mut u8, len: usize) -> bool,
>;
pub mod SysOption {
    pub type Type = ::std::os::raw::c_int;
    pub const USERDATA: Type = 0;
    pub const DECODE_PNG: Type = 1;
    pub const LOG: Type = 2;
    pub const RANDOM_SECURE: Type = 3;
    pub const MAX_VALUE: Type = 2147483647;
}
unsafe extern "C" {
    pub fn ghostty_sys_set(
        option: SysOption::Type,
        value: *const ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_sys_log_stderr(
        userdata: *mut ::std::os::raw::c_void,
        level: SysLogLevel::Type,
        scope: *const u8,
        scope_len: usize,
        message: *const u8,
        message_len: usize,
    );
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct KeyEventImpl {
    _unused: [u8; 0],
}
pub type KeyEvent = *mut KeyEventImpl;
pub mod KeyAction {
    pub type Type = ::std::os::raw::c_int;
    pub const RELEASE: Type = 0;
    pub const PRESS: Type = 1;
    pub const REPEAT: Type = 2;
    pub const MAX_VALUE: Type = 2147483647;
}
pub type Mods = u16;
pub mod Key {
    pub type Type = ::std::os::raw::c_int;
    pub const UNIDENTIFIED: Type = 0;
    pub const BACKQUOTE: Type = 1;
    pub const BACKSLASH: Type = 2;
    pub const BRACKET_LEFT: Type = 3;
    pub const BRACKET_RIGHT: Type = 4;
    pub const COMMA: Type = 5;
    pub const DIGIT_0: Type = 6;
    pub const DIGIT_1: Type = 7;
    pub const DIGIT_2: Type = 8;
    pub const DIGIT_3: Type = 9;
    pub const DIGIT_4: Type = 10;
    pub const DIGIT_5: Type = 11;
    pub const DIGIT_6: Type = 12;
    pub const DIGIT_7: Type = 13;
    pub const DIGIT_8: Type = 14;
    pub const DIGIT_9: Type = 15;
    pub const EQUAL: Type = 16;
    pub const INTL_BACKSLASH: Type = 17;
    pub const INTL_RO: Type = 18;
    pub const INTL_YEN: Type = 19;
    pub const A: Type = 20;
    pub const B: Type = 21;
    pub const C: Type = 22;
    pub const D: Type = 23;
    pub const E: Type = 24;
    pub const F: Type = 25;
    pub const G: Type = 26;
    pub const H: Type = 27;
    pub const I: Type = 28;
    pub const J: Type = 29;
    pub const K: Type = 30;
    pub const L: Type = 31;
    pub const M: Type = 32;
    pub const N: Type = 33;
    pub const O: Type = 34;
    pub const P: Type = 35;
    pub const Q: Type = 36;
    pub const R: Type = 37;
    pub const S: Type = 38;
    pub const T: Type = 39;
    pub const U: Type = 40;
    pub const V: Type = 41;
    pub const W: Type = 42;
    pub const X: Type = 43;
    pub const Y: Type = 44;
    pub const Z: Type = 45;
    pub const MINUS: Type = 46;
    pub const PERIOD: Type = 47;
    pub const QUOTE: Type = 48;
    pub const SEMICOLON: Type = 49;
    pub const SLASH: Type = 50;
    pub const ALT_LEFT: Type = 51;
    pub const ALT_RIGHT: Type = 52;
    pub const BACKSPACE: Type = 53;
    pub const CAPS_LOCK: Type = 54;
    pub const CONTEXT_MENU: Type = 55;
    pub const CONTROL_LEFT: Type = 56;
    pub const CONTROL_RIGHT: Type = 57;
    pub const ENTER: Type = 58;
    pub const META_LEFT: Type = 59;
    pub const META_RIGHT: Type = 60;
    pub const SHIFT_LEFT: Type = 61;
    pub const SHIFT_RIGHT: Type = 62;
    pub const SPACE: Type = 63;
    pub const TAB: Type = 64;
    pub const CONVERT: Type = 65;
    pub const KANA_MODE: Type = 66;
    pub const NON_CONVERT: Type = 67;
    pub const DELETE: Type = 68;
    pub const END: Type = 69;
    pub const HELP: Type = 70;
    pub const HOME: Type = 71;
    pub const INSERT: Type = 72;
    pub const PAGE_DOWN: Type = 73;
    pub const PAGE_UP: Type = 74;
    pub const ARROW_DOWN: Type = 75;
    pub const ARROW_LEFT: Type = 76;
    pub const ARROW_RIGHT: Type = 77;
    pub const ARROW_UP: Type = 78;
    pub const NUM_LOCK: Type = 79;
    pub const NUMPAD_0: Type = 80;
    pub const NUMPAD_1: Type = 81;
    pub const NUMPAD_2: Type = 82;
    pub const NUMPAD_3: Type = 83;
    pub const NUMPAD_4: Type = 84;
    pub const NUMPAD_5: Type = 85;
    pub const NUMPAD_6: Type = 86;
    pub const NUMPAD_7: Type = 87;
    pub const NUMPAD_8: Type = 88;
    pub const NUMPAD_9: Type = 89;
    pub const NUMPAD_ADD: Type = 90;
    pub const NUMPAD_BACKSPACE: Type = 91;
    pub const NUMPAD_CLEAR: Type = 92;
    pub const NUMPAD_CLEAR_ENTRY: Type = 93;
    pub const NUMPAD_COMMA: Type = 94;
    pub const NUMPAD_DECIMAL: Type = 95;
    pub const NUMPAD_DIVIDE: Type = 96;
    pub const NUMPAD_ENTER: Type = 97;
    pub const NUMPAD_EQUAL: Type = 98;
    pub const NUMPAD_MEMORY_ADD: Type = 99;
    pub const NUMPAD_MEMORY_CLEAR: Type = 100;
    pub const NUMPAD_MEMORY_RECALL: Type = 101;
    pub const NUMPAD_MEMORY_STORE: Type = 102;
    pub const NUMPAD_MEMORY_SUBTRACT: Type = 103;
    pub const NUMPAD_MULTIPLY: Type = 104;
    pub const NUMPAD_PAREN_LEFT: Type = 105;
    pub const NUMPAD_PAREN_RIGHT: Type = 106;
    pub const NUMPAD_SUBTRACT: Type = 107;
    pub const NUMPAD_SEPARATOR: Type = 108;
    pub const NUMPAD_UP: Type = 109;
    pub const NUMPAD_DOWN: Type = 110;
    pub const NUMPAD_RIGHT: Type = 111;
    pub const NUMPAD_LEFT: Type = 112;
    pub const NUMPAD_BEGIN: Type = 113;
    pub const NUMPAD_HOME: Type = 114;
    pub const NUMPAD_END: Type = 115;
    pub const NUMPAD_INSERT: Type = 116;
    pub const NUMPAD_DELETE: Type = 117;
    pub const NUMPAD_PAGE_UP: Type = 118;
    pub const NUMPAD_PAGE_DOWN: Type = 119;
    pub const ESCAPE: Type = 120;
    pub const F1: Type = 121;
    pub const F2: Type = 122;
    pub const F3: Type = 123;
    pub const F4: Type = 124;
    pub const F5: Type = 125;
    pub const F6: Type = 126;
    pub const F7: Type = 127;
    pub const F8: Type = 128;
    pub const F9: Type = 129;
    pub const F10: Type = 130;
    pub const F11: Type = 131;
    pub const F12: Type = 132;
    pub const F13: Type = 133;
    pub const F14: Type = 134;
    pub const F15: Type = 135;
    pub const F16: Type = 136;
    pub const F17: Type = 137;
    pub const F18: Type = 138;
    pub const F19: Type = 139;
    pub const F20: Type = 140;
    pub const F21: Type = 141;
    pub const F22: Type = 142;
    pub const F23: Type = 143;
    pub const F24: Type = 144;
    pub const F25: Type = 145;
    pub const FN: Type = 146;
    pub const FN_LOCK: Type = 147;
    pub const PRINT_SCREEN: Type = 148;
    pub const SCROLL_LOCK: Type = 149;
    pub const PAUSE: Type = 150;
    pub const BROWSER_BACK: Type = 151;
    pub const BROWSER_FAVORITES: Type = 152;
    pub const BROWSER_FORWARD: Type = 153;
    pub const BROWSER_HOME: Type = 154;
    pub const BROWSER_REFRESH: Type = 155;
    pub const BROWSER_SEARCH: Type = 156;
    pub const BROWSER_STOP: Type = 157;
    pub const EJECT: Type = 158;
    pub const LAUNCH_APP_1: Type = 159;
    pub const LAUNCH_APP_2: Type = 160;
    pub const LAUNCH_MAIL: Type = 161;
    pub const MEDIA_PLAY_PAUSE: Type = 162;
    pub const MEDIA_SELECT: Type = 163;
    pub const MEDIA_STOP: Type = 164;
    pub const MEDIA_TRACK_NEXT: Type = 165;
    pub const MEDIA_TRACK_PREVIOUS: Type = 166;
    pub const POWER: Type = 167;
    pub const SLEEP: Type = 168;
    pub const AUDIO_VOLUME_DOWN: Type = 169;
    pub const AUDIO_VOLUME_MUTE: Type = 170;
    pub const AUDIO_VOLUME_UP: Type = 171;
    pub const WAKE_UP: Type = 172;
    pub const COPY: Type = 173;
    pub const CUT: Type = 174;
    pub const PASTE: Type = 175;
    pub const MAX_VALUE: Type = 2147483647;
}
unsafe extern "C" {
    pub fn ghostty_key_event_new(allocator: *const Allocator, event: *mut KeyEvent)
        -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_key_event_free(event: KeyEvent);
}
unsafe extern "C" {
    pub fn ghostty_key_event_set_action(event: KeyEvent, action: KeyAction::Type);
}
unsafe extern "C" {
    pub fn ghostty_key_event_get_action(event: KeyEvent) -> KeyAction::Type;
}
unsafe extern "C" {
    pub fn ghostty_key_event_set_key(event: KeyEvent, key: Key::Type);
}
unsafe extern "C" {
    pub fn ghostty_key_event_get_key(event: KeyEvent) -> Key::Type;
}
unsafe extern "C" {
    pub fn ghostty_key_event_set_mods(event: KeyEvent, mods: Mods);
}
unsafe extern "C" {
    pub fn ghostty_key_event_get_mods(event: KeyEvent) -> Mods;
}
unsafe extern "C" {
    pub fn ghostty_key_event_set_consumed_mods(event: KeyEvent, consumed_mods: Mods);
}
unsafe extern "C" {
    pub fn ghostty_key_event_get_consumed_mods(event: KeyEvent) -> Mods;
}
unsafe extern "C" {
    pub fn ghostty_key_event_set_composing(event: KeyEvent, composing: bool);
}
unsafe extern "C" {
    pub fn ghostty_key_event_get_composing(event: KeyEvent) -> bool;
}
unsafe extern "C" {
    pub fn ghostty_key_event_set_utf8(
        event: KeyEvent,
        utf8: *const ::std::os::raw::c_char,
        len: usize,
    );
}
unsafe extern "C" {
    pub fn ghostty_key_event_get_utf8(
        event: KeyEvent,
        len: *mut usize,
    ) -> *const ::std::os::raw::c_char;
}
unsafe extern "C" {
    pub fn ghostty_key_event_set_unshifted_codepoint(event: KeyEvent, codepoint: u32);
}
unsafe extern "C" {
    pub fn ghostty_key_event_get_unshifted_codepoint(event: KeyEvent) -> u32;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct KeyEncoderImpl {
    _unused: [u8; 0],
}
pub type KeyEncoder = *mut KeyEncoderImpl;
pub type KittyKeyFlags = u8;
pub mod OptionAsAlt {
    pub type Type = ::std::os::raw::c_int;
    pub const FALSE: Type = 0;
    pub const TRUE: Type = 1;
    pub const LEFT: Type = 2;
    pub const RIGHT: Type = 3;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod KeyEncoderOption {
    pub type Type = ::std::os::raw::c_int;
    pub const CURSOR_KEY_APPLICATION: Type = 0;
    pub const KEYPAD_KEY_APPLICATION: Type = 1;
    pub const IGNORE_KEYPAD_WITH_NUMLOCK: Type = 2;
    pub const ALT_ESC_PREFIX: Type = 3;
    pub const MODIFY_OTHER_KEYS_STATE_2: Type = 4;
    pub const KITTY_FLAGS: Type = 5;
    pub const MACOS_OPTION_AS_ALT: Type = 6;
    pub const BACKARROW_KEY_MODE: Type = 7;
    pub const MAX_VALUE: Type = 2147483647;
}
unsafe extern "C" {
    pub fn ghostty_key_encoder_new(
        allocator: *const Allocator,
        encoder: *mut KeyEncoder,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_key_encoder_free(encoder: KeyEncoder);
}
unsafe extern "C" {
    pub fn ghostty_key_encoder_setopt(
        encoder: KeyEncoder,
        option: KeyEncoderOption::Type,
        value: *const ::std::os::raw::c_void,
    );
}
unsafe extern "C" {
    pub fn ghostty_key_encoder_setopt_from_terminal(encoder: KeyEncoder, terminal: Terminal);
}
unsafe extern "C" {
    pub fn ghostty_key_encoder_encode(
        encoder: KeyEncoder,
        event: KeyEvent,
        out_buf: *mut ::std::os::raw::c_char,
        out_buf_size: usize,
        out_len: *mut usize,
    ) -> Result::Type;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MouseEventImpl {
    _unused: [u8; 0],
}
pub type MouseEvent = *mut MouseEventImpl;
pub mod MouseAction {
    pub type Type = ::std::os::raw::c_int;
    pub const PRESS: Type = 0;
    pub const RELEASE: Type = 1;
    pub const MOTION: Type = 2;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod MouseButton {
    pub type Type = ::std::os::raw::c_int;
    pub const UNKNOWN: Type = 0;
    pub const LEFT: Type = 1;
    pub const RIGHT: Type = 2;
    pub const MIDDLE: Type = 3;
    pub const FOUR: Type = 4;
    pub const FIVE: Type = 5;
    pub const SIX: Type = 6;
    pub const SEVEN: Type = 7;
    pub const EIGHT: Type = 8;
    pub const NINE: Type = 9;
    pub const TEN: Type = 10;
    pub const ELEVEN: Type = 11;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct MousePosition {
    pub x: f32,
    pub y: f32,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of MousePosition"][::std::mem::size_of::<MousePosition>() - 8usize];
    ["Alignment of MousePosition"][::std::mem::align_of::<MousePosition>() - 4usize];
    ["Offset of field: MousePosition::x"][::std::mem::offset_of!(MousePosition, x) - 0usize];
    ["Offset of field: MousePosition::y"][::std::mem::offset_of!(MousePosition, y) - 4usize];
};
unsafe extern "C" {
    pub fn ghostty_mouse_event_new(
        allocator: *const Allocator,
        event: *mut MouseEvent,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_mouse_event_free(event: MouseEvent);
}
unsafe extern "C" {
    pub fn ghostty_mouse_event_set_action(event: MouseEvent, action: MouseAction::Type);
}
unsafe extern "C" {
    pub fn ghostty_mouse_event_get_action(event: MouseEvent) -> MouseAction::Type;
}
unsafe extern "C" {
    pub fn ghostty_mouse_event_set_button(event: MouseEvent, button: MouseButton::Type);
}
unsafe extern "C" {
    pub fn ghostty_mouse_event_clear_button(event: MouseEvent);
}
unsafe extern "C" {
    pub fn ghostty_mouse_event_get_button(
        event: MouseEvent,
        out_button: *mut MouseButton::Type,
    ) -> bool;
}
unsafe extern "C" {
    pub fn ghostty_mouse_event_set_mods(event: MouseEvent, mods: Mods);
}
unsafe extern "C" {
    pub fn ghostty_mouse_event_get_mods(event: MouseEvent) -> Mods;
}
unsafe extern "C" {
    pub fn ghostty_mouse_event_set_position(event: MouseEvent, position: MousePosition);
}
unsafe extern "C" {
    pub fn ghostty_mouse_event_get_position(event: MouseEvent) -> MousePosition;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MouseEncoderImpl {
    _unused: [u8; 0],
}
pub type MouseEncoder = *mut MouseEncoderImpl;
pub mod MouseTrackingMode {
    pub type Type = ::std::os::raw::c_int;
    pub const NONE: Type = 0;
    pub const X10: Type = 1;
    pub const NORMAL: Type = 2;
    pub const BUTTON: Type = 3;
    pub const ANY: Type = 4;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod MouseFormat {
    pub type Type = ::std::os::raw::c_int;
    pub const X10: Type = 0;
    pub const UTF8: Type = 1;
    pub const SGR: Type = 2;
    pub const URXVT: Type = 3;
    pub const SGR_PIXELS: Type = 4;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct MouseEncoderSize {
    pub size: usize,
    pub screen_width: u32,
    pub screen_height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub padding_top: u32,
    pub padding_bottom: u32,
    pub padding_right: u32,
    pub padding_left: u32,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of MouseEncoderSize"][::std::mem::size_of::<MouseEncoderSize>() - 40usize];
    ["Alignment of MouseEncoderSize"][::std::mem::align_of::<MouseEncoderSize>() - 8usize];
    ["Offset of field: MouseEncoderSize::size"]
        [::std::mem::offset_of!(MouseEncoderSize, size) - 0usize];
    ["Offset of field: MouseEncoderSize::screen_width"]
        [::std::mem::offset_of!(MouseEncoderSize, screen_width) - 8usize];
    ["Offset of field: MouseEncoderSize::screen_height"]
        [::std::mem::offset_of!(MouseEncoderSize, screen_height) - 12usize];
    ["Offset of field: MouseEncoderSize::cell_width"]
        [::std::mem::offset_of!(MouseEncoderSize, cell_width) - 16usize];
    ["Offset of field: MouseEncoderSize::cell_height"]
        [::std::mem::offset_of!(MouseEncoderSize, cell_height) - 20usize];
    ["Offset of field: MouseEncoderSize::padding_top"]
        [::std::mem::offset_of!(MouseEncoderSize, padding_top) - 24usize];
    ["Offset of field: MouseEncoderSize::padding_bottom"]
        [::std::mem::offset_of!(MouseEncoderSize, padding_bottom) - 28usize];
    ["Offset of field: MouseEncoderSize::padding_right"]
        [::std::mem::offset_of!(MouseEncoderSize, padding_right) - 32usize];
    ["Offset of field: MouseEncoderSize::padding_left"]
        [::std::mem::offset_of!(MouseEncoderSize, padding_left) - 36usize];
};
pub mod MouseEncoderOption {
    pub type Type = ::std::os::raw::c_int;
    pub const EVENT: Type = 0;
    pub const FORMAT: Type = 1;
    pub const SIZE: Type = 2;
    pub const ANY_BUTTON_PRESSED: Type = 3;
    pub const TRACK_LAST_CELL: Type = 4;
    pub const MAX_VALUE: Type = 2147483647;
}
unsafe extern "C" {
    pub fn ghostty_mouse_encoder_new(
        allocator: *const Allocator,
        encoder: *mut MouseEncoder,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_mouse_encoder_free(encoder: MouseEncoder);
}
unsafe extern "C" {
    pub fn ghostty_mouse_encoder_setopt(
        encoder: MouseEncoder,
        option: MouseEncoderOption::Type,
        value: *const ::std::os::raw::c_void,
    );
}
unsafe extern "C" {
    pub fn ghostty_mouse_encoder_setopt_from_terminal(encoder: MouseEncoder, terminal: Terminal);
}
unsafe extern "C" {
    pub fn ghostty_mouse_encoder_reset(encoder: MouseEncoder);
}
unsafe extern "C" {
    pub fn ghostty_mouse_encoder_encode(
        encoder: MouseEncoder,
        event: MouseEvent,
        out_buf: *mut ::std::os::raw::c_char,
        out_buf_size: usize,
        out_len: *mut usize,
    ) -> Result::Type;
}
pub mod MouseShape {
    pub type Type = ::std::os::raw::c_int;
    pub const DEFAULT: Type = 0;
    pub const CONTEXT_MENU: Type = 1;
    pub const HELP: Type = 2;
    pub const POINTER: Type = 3;
    pub const PROGRESS: Type = 4;
    pub const WAIT: Type = 5;
    pub const CELL: Type = 6;
    pub const CROSSHAIR: Type = 7;
    pub const TEXT: Type = 8;
    pub const VERTICAL_TEXT: Type = 9;
    pub const ALIAS: Type = 10;
    pub const COPY: Type = 11;
    pub const MOVE: Type = 12;
    pub const NO_DROP: Type = 13;
    pub const NOT_ALLOWED: Type = 14;
    pub const GRAB: Type = 15;
    pub const GRABBING: Type = 16;
    pub const ALL_SCROLL: Type = 17;
    pub const COL_RESIZE: Type = 18;
    pub const ROW_RESIZE: Type = 19;
    pub const N_RESIZE: Type = 20;
    pub const E_RESIZE: Type = 21;
    pub const S_RESIZE: Type = 22;
    pub const W_RESIZE: Type = 23;
    pub const NE_RESIZE: Type = 24;
    pub const NW_RESIZE: Type = 25;
    pub const SE_RESIZE: Type = 26;
    pub const SW_RESIZE: Type = 27;
    pub const EW_RESIZE: Type = 28;
    pub const NS_RESIZE: Type = 29;
    pub const NESW_RESIZE: Type = 30;
    pub const NWSE_RESIZE: Type = 31;
    pub const ZOOM_IN: Type = 32;
    pub const ZOOM_OUT: Type = 33;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod PasteSource {
    pub type Type = ::std::os::raw::c_int;
    pub const CLIPBOARD: Type = 0;
    pub const TEXT: Type = 1;
    pub const MAX_VALUE: Type = 2147483647;
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Paste {
    pub size: usize,
    pub location: ClipboardLocation::Type,
    pub source: PasteSource::Type,
    pub mimes: *const String,
    pub mimes_len: usize,
    pub reader: MimeReader,
    pub allow_unsafe: bool,
}
#[allow(clippy::unnecessary_operation, clippy::identity_op)]
const _: () = {
    ["Size of Paste"][::std::mem::size_of::<Paste>() - 56usize];
    ["Alignment of Paste"][::std::mem::align_of::<Paste>() - 8usize];
    ["Offset of field: Paste::size"][::std::mem::offset_of!(Paste, size) - 0usize];
    ["Offset of field: Paste::location"][::std::mem::offset_of!(Paste, location) - 8usize];
    ["Offset of field: Paste::source"][::std::mem::offset_of!(Paste, source) - 12usize];
    ["Offset of field: Paste::mimes"][::std::mem::offset_of!(Paste, mimes) - 16usize];
    ["Offset of field: Paste::mimes_len"][::std::mem::offset_of!(Paste, mimes_len) - 24usize];
    ["Offset of field: Paste::reader"][::std::mem::offset_of!(Paste, reader) - 32usize];
    ["Offset of field: Paste::allow_unsafe"][::std::mem::offset_of!(Paste, allow_unsafe) - 48usize];
};
impl Default for Paste {
    fn default() -> Self {
        let mut s = ::std::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            ::std::ptr::write_bytes(s.as_mut_ptr(), 0, 1);
            s.assume_init()
        }
    }
}
unsafe extern "C" {
    pub fn ghostty_terminal_paste(
        terminal: Terminal,
        paste: *const Paste,
        out_written: *mut bool,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_paste_is_safe(data: *const ::std::os::raw::c_char, len: usize) -> bool;
}
unsafe extern "C" {
    pub fn ghostty_paste_encode(
        data: *mut ::std::os::raw::c_char,
        data_len: usize,
        bracketed: bool,
        buf: *mut ::std::os::raw::c_char,
        buf_len: usize,
        out_written: *mut usize,
    ) -> Result::Type;
}
pub mod SearchStatus {
    pub type Type = ::std::os::raw::c_int;
    pub const RUNNING: Type = 0;
    pub const FEED_REQUIRED: Type = 1;
    pub const COMPLETE: Type = 2;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod SearchScroll {
    pub type Type = ::std::os::raw::c_int;
    pub const IF_NEEDED: Type = 0;
    pub const NONE: Type = 1;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod SearchData {
    pub type Type = ::std::os::raw::c_int;
    pub const STATUS: Type = 0;
    pub const NEEDLE: Type = 1;
    pub const TOTAL_MATCHES: Type = 2;
    pub const SELECTED_INDEX: Type = 3;
    pub const SELECTED_MATCH: Type = 4;
    pub const MATCHES: Type = 5;
    pub const VIEWPORT_MATCHES: Type = 6;
    pub const SELECT_SCROLL: Type = 7;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod SearchOption {
    pub type Type = ::std::os::raw::c_int;
    pub const NEEDLE: Type = 0;
    pub const SELECT_NEXT: Type = 1;
    pub const SELECT_PREV: Type = 2;
    pub const SELECT_SCROLL: Type = 3;
    pub const MAX_VALUE: Type = 2147483647;
}
unsafe extern "C" {
    pub fn ghostty_search_new(
        allocator: *const Allocator,
        out_search: *mut Search,
        terminal: Terminal,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_search_free(search: Search);
}
unsafe extern "C" {
    pub fn ghostty_search_tick(search: Search, out_status: *mut SearchStatus::Type)
        -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_search_feed(search: Search) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_search_run(search: Search) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_search_set(
        search: Search,
        option: SearchOption::Type,
        value: *const ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_search_get(
        search: Search,
        data: SearchData::Type,
        value: *mut ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_search_get_multi(
        search: Search,
        count: usize,
        keys: *const SearchData::Type,
        values: *mut *mut ::std::os::raw::c_void,
        out_written: *mut usize,
    ) -> Result::Type;
}
pub mod SnapshotDecoderOption {
    pub type Type = ::std::os::raw::c_int;
    pub const MAX_CONTINUATION_BYTES: Type = 0;
    pub const RETAIN_CONTINUATION: Type = 1;
    pub const COMPRESS_HISTORY: Type = 2;
    pub const KITTY_IMAGE_STORAGE_LIMIT: Type = 3;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod SnapshotDecoderProgressKind {
    pub type Type = ::std::os::raw::c_int;
    pub const PAGE: Type = 0;
    pub const IMAGE: Type = 1;
    pub const MAX_VALUE: Type = 2147483647;
}
pub mod SnapshotDecoderData {
    pub type Type = ::std::os::raw::c_int;
    pub const INVALID: Type = 0;
    pub const MAX_CONTINUATION_BYTES: Type = 1;
    pub const SOURCE_OFFSET: Type = 2;
    pub const HISTORY_ROWS_PRIMARY: Type = 3;
    pub const HISTORY_ROWS_ALTERNATE: Type = 4;
    pub const PROGRESS_SCREEN: Type = 5;
    pub const PROGRESS_ROWS: Type = 6;
    pub const PROGRESS_REMAINING: Type = 7;
    pub const RETAIN_CONTINUATION: Type = 8;
    pub const COMPRESS_HISTORY: Type = 9;
    pub const KITTY_IMAGE_STORAGE_LIMIT: Type = 10;
    pub const PROGRESS_KIND: Type = 11;
    pub const PROGRESS_IMAGE_ID: Type = 12;
    pub const PROGRESS_IMAGE_FRAME: Type = 13;
    pub const PROGRESS_IMAGE_BYTES: Type = 14;
    pub const KITTY_IMAGES_PRIMARY: Type = 15;
    pub const KITTY_IMAGES_ALTERNATE: Type = 16;
    pub const KITTY_PENDING_BYTES_PRIMARY: Type = 17;
    pub const KITTY_PENDING_BYTES_ALTERNATE: Type = 18;
    pub const MAX_VALUE: Type = 2147483647;
}
unsafe extern "C" {
    pub fn ghostty_snapshot_encode(terminal: Terminal, writer: Writer) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_snapshot_encode_buf(
        terminal: Terminal,
        buf: *mut u8,
        buf_len: usize,
        out_written: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_snapshot_encode_alloc(
        terminal: Terminal,
        allocator: *const Allocator,
        out_ptr: *mut *mut u8,
        out_len: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_snapshot_decoder_new(
        allocator: *const Allocator,
        decoder: *mut SnapshotDecoder,
        reader: Reader,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_snapshot_decoder_new_buf(
        allocator: *const Allocator,
        decoder: *mut SnapshotDecoder,
        ptr: *const u8,
        len: usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_snapshot_decoder_free(decoder: SnapshotDecoder);
}
unsafe extern "C" {
    pub fn ghostty_snapshot_decoder_set(
        decoder: SnapshotDecoder,
        option: SnapshotDecoderOption::Type,
        value: *const ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_snapshot_decoder_ready(
        decoder: SnapshotDecoder,
        terminal: *mut Terminal,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_snapshot_decoder_next(decoder: SnapshotDecoder) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_snapshot_decoder_decode(
        decoder: SnapshotDecoder,
        terminal: *mut Terminal,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_snapshot_decoder_get(
        decoder: SnapshotDecoder,
        data: SnapshotDecoderData::Type,
        out: *mut ::std::os::raw::c_void,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_snapshot_decoder_get_multi(
        decoder: SnapshotDecoder,
        count: usize,
        keys: *const SnapshotDecoderData::Type,
        values: *mut *mut ::std::os::raw::c_void,
        out_written: *mut usize,
    ) -> Result::Type;
}
unsafe extern "C" {
    pub fn ghostty_unicode_codepoint_width(cp: u32) -> u8;
}
unsafe extern "C" {
    pub fn ghostty_unicode_grapheme_width(cps: *const u32, len: usize, width: *mut u8) -> usize;
}
