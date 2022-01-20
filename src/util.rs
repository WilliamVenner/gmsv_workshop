use std::cell::UnsafeCell;

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
