#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(improper_ctypes)]
extern crate libc;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod test {
    use super::msLoadMapFromString;

    use std::ffi::CString;
    use std::os::raw::c_char;

    #[test]
    fn load_map() {
        unsafe {
            let mapfile_contents = "MAP END".to_string();
            let mapfile_cstr = CString::new(mapfile_contents).unwrap();
            let buffer = mapfile_cstr.as_ptr() as *mut c_char;

            let new_mappath = std::ptr::null_mut() as *mut c_char;

            let map = msLoadMapFromString(buffer, new_mappath);
        }
    }
}
