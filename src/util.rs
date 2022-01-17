use std::{path::Path, cell::UnsafeCell};

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
