#![feature(c_unwind)]

use steamworks::PublishedFileId;

#[macro_use] extern crate gmod;

mod workshop;
mod util;

// TODO refactor

lazy_static::lazy_static! {
	static ref UGC_CHAN: Option<(workshop::Steam, crossbeam::channel::Receiver<(gmod::lua::LuaReference, Result<String, ()>)>)> = Some(workshop::Steam::new());
}

unsafe extern "C-unwind" fn poll(lua: gmod::lua::State) -> i32 {
	let ugc = match &*UGC_CHAN {
		Some(ugc) => &ugc.1,
		None => return 0
	};

	while let Ok((reference, result)) = ugc.try_recv() {
		lua.from_reference(reference);
		lua.dereference(reference);
		if let Ok(folder) = result {
			lua.push_string(&folder);
		} else {
			lua.push_nil();
		}
		lua.pcall_ignore(1, 0);
	}

	0
}

unsafe extern "C-unwind" fn download(lua: gmod::lua::State) -> i32 {
	let workshop_id = lua.to_integer(1);
	if workshop_id <= 0 { return 0; }

	let ugc = match &*UGC_CHAN {
		Some(ugc) => &ugc.0,
		None => return 0
	};

	let callback = if !lua.is_nil(2) {
		lua.check_function(2);
		lua.push_value(2);
		Some(lua.reference())
	} else {
		None
	};

	ugc.download(PublishedFileId(workshop_id as _), callback);

	// TODO remove this hook later
	// TODO use a 1 second timer
	lua.get_global(lua_string!("hook"));
	lua.get_field(-1, lua_string!("Add"));
	lua.push_string("Think");
	lua.push_string("gmsv_downloadugc");
	lua.push_function(poll);
	lua.call(3, 0);
	lua.pop();

	0
}

#[gmod13_open]
unsafe fn gmod13_open(lua: gmod::lua::State) -> i32 {
	lua.push_function(download);
	lua.set_global(lua_string!("gmsv_downloadugc"));
	0
}

#[gmod13_close]
unsafe fn gmod13_close(lua: gmod::lua::State) -> i32 {

	0
}