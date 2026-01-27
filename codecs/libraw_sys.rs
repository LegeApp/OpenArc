use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uint};
use std::ptr;

#[repr(C)]
pub struct libraw_data_t {
    pub image: [*mut u16; 4],
    pub sizes: libraw_image_sizes_t,
    pub idata: libraw_iparams_t,
    pub progress_flags: c_uint,
    pub process_warnings: c_uint,
    pub color: libraw_colordata_t,
    pub other: libraw_imgother_t,
    pub thumbnail: libraw_thumbnail_t,
    pub rawdata: libraw_rawdata_t,
    pub parent_class: *mut std::os::raw::c_void,
}

#[repr(C)]
pub struct libraw_image_sizes_t {
    pub raw_height: c_uint,
    pub raw_width: c_uint,
    pub height: c_uint,
    pub width: c_uint,
    pub top_margin: c_uint,
    pub left_margin: c_uint,
    pub iheight: c_uint,
    pub iwidth: c_uint,
    pub raw_pitch: c_uint,
    pub pixel_aspect: f64,
    pub flip: c_int,
    pub mask: [[c_uint; 8]; 4],
}

#[repr(C)]
pub struct libraw_iparams_t {
    pub make: [c_char; 64],
    pub model: [c_char; 64],
    pub raw_count: c_uint,
    pub dng_version: c_uint,
    pub is_foveon: c_uint,
    pub colors: c_uint,
    pub filters: c_uint,
    pub xtrans: [[c_char; 6]; 6],
    pub cdesc: [c_char; 5],
}

#[repr(C)]
pub struct libraw_colordata_t {
    pub make: [c_char; 64],
    pub model: [c_char; 64],
    pub raw_count: c_uint,
    pub dng_version: c_uint,
    pub is_foveon: c_uint,
    pub colors: c_uint,
    pub filters: c_uint,
    pub xtrans: [[c_char; 6]; 6],
    pub cdesc: [c_char; 5],
    // ... simplified for brevity
}

#[repr(C)]
pub struct libraw_imgother_t {
    pub iso_speed: f32,
    pub shutter: f32,
    pub aperture: f32,
    pub focal_len: f32,
    pub timestamp: i64,
    pub shot_order: c_uint,
    pub gpsdata: [c_uint; 32],
    pub desc: [c_char; 512],
    pub artist: [c_char; 64],
}

#[repr(C)]
pub struct libraw_thumbnail_t {
    pub tformat: libraw_thumbnail_formats_t,
    pub twidth: c_uint,
    pub theight: c_uint,
    pub tlength: c_uint,
    pub tcolors: c_uint,
    pub thumb: *mut c_char,
}

#[repr(C)]
pub struct libraw_rawdata_t {
    pub raw_alloc: *mut std::os::raw::c_void,
    pub raw_image: *mut u16,
    pub color4_image: [*mut u16; 4],
    pub color3_image: [*mut u16; 3],
    // ... simplified for brevity
}

#[repr(C)]
pub enum libraw_thumbnail_formats_t {
    LIBRAW_THUMBNAIL_UNKNOWN = 0,
    LIBRAW_THUMBNAIL_JPEG = 1,
    LIBRAW_THUMBNAIL_BITMAP = 2,
    LIBRAW_THUMBNAIL_LAYER = 4,
    LIBRAW_THUMBNAIL_ROLLEI = 5,
}

#[repr(C)]
pub enum libraw_progress_t {
    LIBRAW_PROGRESS_START = 0,
    LIBRAW_PROGRESS_OPEN = 1,
    LIBRAW_PROGRESS_IDENTIFY = 2,
    LIBRAW_PROGRESS_SIZE_ADJUST = 3,
    LIBRAW_PROGRESS_LOAD_RAW = 4,
    LIBRAW_PROGRESS_RAW2IMAGE = 5,
    LIBRAW_PROGRESS_REMOVE_NOISES = 6,
    LIBRAW_PROGRESS_SCALE_COLORS = 7,
    LIBRAW_PROGRESS_PRE_INTERPOLATE = 8,
    LIBRAW_PROGRESS_INTERPOLATE = 9,
    LIBRAW_PROGRESS_POST_INTERPOLATE = 10,
    LIBRAW_PROGRESS_MEDIAN_FILTER = 11,
    LIBRAW_PROGRESS_FILL_HOLES = 12,
    LIBRAW_PROGRESS_BLANK = 13,
    LIBRAW_PROGRESS_CONVERT_RGB = 14,
    LIBRAW_PROGRESS_STRETCH = 15,
    LIBRAW_PROGRESS_FINISH = 16,
}

#[repr(C)]
pub enum libraw_errors_t {
    LIBRAW_SUCCESS = 0,
    LIBRAW_UNSPECIFIED_ERROR = -1,
    LIBRAW_FILE_UNSUPPORTED = -2,
    LIBRAW_REQUEST_FOR_NONEXISTENT_IMAGE = -3,
    LIBRAW_OUT_OF_ORDER_CALL = -4,
    LIBRAW_NO_THUMBNAIL = -5,
    LIBRAW_UNSUPPORTED_THUMBNAIL = -6,
    LIBRAW_INPUT_CLOSED = -7,
    LIBRAW_NOT_IMPLEMENTED = -8,
    LIBRAW_UNSUFFICIENT_MEMORY = -9,
    LIBRAW_DATA_ERROR = -10,
    LIBRAW_IO_ERROR = -11,
    LIBRAW_CANCELLED_BY_CALLBACK = -12,
    LIBRAW_BAD_CROP = -13,
}

#[link(name = "raw")]
extern "C" {
    pub fn libraw_init(flags: c_uint) -> *mut libraw_data_t;
    pub fn libraw_open_file(lr: *mut libraw_data_t, filename: *const c_char) -> c_int;
    pub fn libraw_unpack(lr: *mut libraw_data_t) -> c_int;
    pub fn libraw_unpack_thumb(lr: *mut libraw_data_t) -> c_int;
    pub fn libraw_dcraw_process(lr: *mut libraw_data_t) -> c_int;
    pub fn libraw_dcraw_ppm_tiff_writer(lr: *mut libraw_data_t, filename: *const c_char) -> c_int;
    pub fn libraw_strerror(error: c_int) -> *const c_char;
    pub fn libraw_close(lr: *mut libraw_data_t);
}

pub fn libraw_error_string(error: c_int) -> String {
    unsafe {
        let msg = libraw_strerror(error);
        if msg.is_null() {
            "Unknown error".to_string()
        } else {
            CStr::from_ptr(msg).to_string_lossy().into_owned()
        }
    }
}
