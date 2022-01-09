use std::{sync::atomic::AtomicBool, mem::MaybeUninit, cell::UnsafeCell};

pub struct AtomicWriteOnceCell<T> {
	value: UnsafeCell<MaybeUninit<T>>,
	initialized: AtomicBool
}
impl<T> AtomicWriteOnceCell<T> {
	pub const fn uninit() -> AtomicWriteOnceCell<T> {
		AtomicWriteOnceCell {
			value: UnsafeCell::new(MaybeUninit::uninit()),
			initialized: AtomicBool::new(false)
		}
	}

	pub const fn new(val: T) -> AtomicWriteOnceCell<T> {
		AtomicWriteOnceCell {
			value: UnsafeCell::new(MaybeUninit::new(val)),
			initialized: AtomicBool::new(true)
		}
	}

	pub fn get(&self) -> Option<&T> {
		if self.initialized.load(std::sync::atomic::Ordering::Relaxed) {
			Some(unsafe { (&*self.value.get()).assume_init_ref() })
		} else {
			None
		}
	}

	/// Panics if the value is already initialized
	///
	/// Unsafe because this can potentially produce a data race. Should only ever be called by a single thread once.
	pub unsafe fn set(&self, val: T) {
		if self.initialized.load(std::sync::atomic::Ordering::Acquire) {
			panic!("Value is already initialized");
		}
		(&mut *self.value.get()).as_mut_ptr().write(val);
		self.initialized.store(true, std::sync::atomic::Ordering::Release);
	}
}
impl<T> From<Option<T>> for AtomicWriteOnceCell<T> {
    fn from(opt: Option<T>) -> Self {
        opt.map(|val| {
			AtomicWriteOnceCell::new(val)
		})
		.unwrap_or_else(|| {
			AtomicWriteOnceCell::uninit()
		})
    }
}
unsafe impl<T> Send for AtomicWriteOnceCell<T> {}
unsafe impl<T> Sync for AtomicWriteOnceCell<T> {}