use std::{path::{Path, PathBuf}, cell::UnsafeCell};

/// Find a GMA file in a directory
pub fn find_gma<P: AsRef<Path>>(path: P) -> Result<Option<PathBuf>, std::io::Error> {
	Ok(path
		.as_ref()
		.read_dir()?
		.filter_map(|entry| entry.ok())
		.filter_map(|entry| {
			if entry.file_type().ok()?.is_file() {
				Some(entry.path())
			} else {
				None
			}
		})
		.filter(|path| {
			path.extension()
				.map(|str| str.eq_ignore_ascii_case("gma"))
				.unwrap_or(false)
		})
		.next())
}

/// Strips a path to make it relative to Gmod's BASE_PATH
pub fn base_path_relative<'a>(path: &'a Path) -> Option<&'a Path> {
	thread_local! {
		static BASE_PATH: std::path::PathBuf = std::env::current_exe().expect("Failed to get the path of the current executable...?").parent().expect("The current executable has no parent folder...?").to_path_buf();
	}
	BASE_PATH.with(|base_path| {
		path.strip_prefix(base_path).ok()
	})
}

pub struct ChadCell<T>(UnsafeCell<T>);
impl<T> ChadCell<T> {
	pub const fn new(val: T) -> ChadCell<T> {
		ChadCell(UnsafeCell::new(val))
	}
}
impl<T> ChadCell<T> {
	pub fn get_mut(&self) -> &mut T {
		unsafe { &mut *self.0.get() }
	}
}
impl<T> std::ops::Deref for ChadCell<T> {
	type Target = T;

	#[inline]
	fn deref(&self) -> &Self::Target {
		unsafe { &*self.0.get() }
	}
}
impl<T> std::ops::DerefMut for ChadCell<T> {
	#[inline]
	fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe { &mut *self.0.get() }
	}
}
impl<T: Default> Default for ChadCell<T> {
	fn default() -> Self {
		Self(Default::default())
	}
}
