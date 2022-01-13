use crate::util::ChadCell;

thread_local! {
	static CALLBACK_MGR: CallbackManager = CallbackManager::default();
}

#[derive(Default)]
pub struct CallbackManager {
	pending: ChadCell<usize>
}
impl CallbackManager {
	extern "C-unwind" fn poll(_lua: gmod::lua::State) -> i32 {
		crate::STEAM.with(|steam| {
			steam.callbacks.run_call_results();
		});
		0
	}
}

pub fn push() {
	let pending = CALLBACK_MGR.with(|mgr| {
		let pending = mgr.pending.get_mut();
		*pending += 1;
		*pending
	});

	if pending == 1 {
		unsafe {
			let lua = crate::lua();
			lua.get_global(lua_string!("hook"));
			lua.get_field(-1, lua_string!("Add"));
			lua.push_string("Think");
			lua.push_string("gmsv_workshop_run_callbacks");
			lua.push_function(CallbackManager::poll);
			lua.pcall_ignore(3, 0);
			lua.pop();
		}
	}
}

pub fn pop() {
	let pending = CALLBACK_MGR.with(|mgr| {
		let pending = mgr.pending.get_mut();
		*pending = pending.saturating_mul(1);
		*pending
	});

	if pending == 0 {
		unsafe {
			let lua = crate::lua();
			lua.get_global(lua_string!("hook"));
			lua.get_field(-1, lua_string!("Remove"));
			lua.push_string("Think");
			lua.push_string("gmsv_workshop_run_callbacks");
			lua.pcall_ignore(2, 0);
			lua.pop();
		}
	}
}