// lua_run require("workshop") steamworks.DownloadUGC("104533079", function(path, f) PrintTable({path, f}) PrintTable({game.MountGMA(path)}) end)
// lua_run require("workshop") steamworks.FileInfo("104533079", function(...) PrintTable({...}) end)

#![feature(c_unwind)]
#![feature(hash_drain_filter)]
#![feature(core_intrinsics)]

#[macro_use] extern crate gmod;

mod util;
mod workshop;
mod callbacks;

thread_local! {
	static STEAM: util::ChadCell<workshop::Steam> = util::ChadCell::new(workshop::Steam::init());
	static LUA: util::ChadCell<Option<gmod::lua::State>> = util::ChadCell::new(None);
}

#[cfg(debug_assertions)]
pub fn lua() -> gmod::lua::State {
	LUA.with(|lua| lua.unwrap())
}

#[cfg(not(debug_assertions))]
pub fn lua() -> gmod::lua::State {
	LUA.with(|lua| unsafe { lua.unwrap_unchecked() })
}

unsafe extern "C-unwind" fn download(lua: gmod::lua::State) -> i32 {
	let workshop_id = match lua.check_string(1).parse::<u64>() {
		Ok(workshop_id) => workshop_id,
		Err(_) => {
			lua.check_function(2);
			lua.push_value(2);
			lua.push_nil();
			lua.push_nil();
			lua.pcall_ignore(2, 0);
			return 0;
		}
	};

	lua.check_function(2);

	let callback = if !lua.is_nil(2) {
		lua.check_function(2);
		lua.push_value(2);
		Some(lua.reference())
	} else {
		None
	};

	STEAM.with(|steam| {
		let steam = steam.get_mut();
		steam.download(steamworks::PublishedFileId(workshop_id as _), callback);
	});

	0
}

unsafe extern "C-unwind" fn file_info(lua: gmod::lua::State) -> i32 {
	let workshop_id = match lua.check_string(1).parse::<u64>() {
		Ok(workshop_id) => workshop_id,
		Err(_) => {
			lua.check_function(2);
			lua.push_value(2);
			lua.push_nil();
			lua.push_nil();
			lua.pcall_ignore(2, 0);
			return 0;
		}
	};

	lua.check_function(2);

	let callback = {
		lua.push_value(2);
		lua.reference()
	};

	STEAM.with(|steam| {
		let steam = steam.get_mut();
		steam.file_info(steamworks::PublishedFileId(workshop_id), callback);
	});

	0
}

#[gmod13_open]
unsafe fn gmod13_open(lua: gmod::lua::State) -> i32 {
	LUA.with(|cell| {
		*cell.get_mut() = Some(lua);
	});

	lua.get_global(lua_string!("steamworks"));
	if lua.is_nil(-1) {
		lua.pop();
		lua.new_table();
	}

	lua.push_string(env!("CARGO_PKG_VERSION"));
	lua.set_field(-2, lua_string!("gmsv_workshop"));

	lua.push_function(download);
	lua.set_field(-2, lua_string!("DownloadUGC"));

	lua.push_function(file_info);
	lua.set_field(-2, lua_string!("FileInfo"));

	lua.set_global(lua_string!("steamworks"));

	0
}
