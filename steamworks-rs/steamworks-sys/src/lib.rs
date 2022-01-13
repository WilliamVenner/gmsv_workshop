#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]

#[cfg(all(target_os = "windows", target_pointer_width = "32"))]
include!(concat!("bindings_", "win32", ".rs"));
#[cfg(all(target_os = "windows", target_pointer_width = "64"))]
include!(concat!("bindings_", "win64", ".rs"));
#[cfg(all(target_os = "linux", target_pointer_width = "32"))]
include!(concat!("bindings_", "linux32", ".rs"));
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
include!(concat!("bindings_", "linux64", ".rs"));