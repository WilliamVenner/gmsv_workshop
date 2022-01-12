// lua_run require("downloadugc") steamworks.DownloadUGC(2711867367, function(path, f) PrintTable({path, f}) PrintTable({game.MountGMA(path)}) end)

#![feature(c_unwind)]
#![feature(hash_drain_filter)]

#[macro_use] extern crate gmod;

mod util;
mod workshop;

thread_local! {
	static STEAM: util::ChadCell<workshop::Steam> = util::ChadCell::new(workshop::Steam::init());
	static LUA: util::ChadCell<Option<gmod::lua::State>> = util::ChadCell::new(None);
}

pub fn lua() -> gmod::lua::State {
	LUA.with(|lua| unsafe { lua.unwrap_unchecked() })
}

unsafe extern "C-unwind" fn download(lua: gmod::lua::State) -> i32 {
	let workshop_id = lua.to_integer(1);

	if workshop_id <= 0 {
		if !lua.is_nil(2) {
			lua.check_function(2);
			lua.push_value(2);
			lua.push_nil();
			lua.push_nil();
			lua.pcall_ignore(2, 0);
		}
		return 0;
	}

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

	lua.push_function(download);
	lua.set_field(-2, lua_string!("DownloadUGC"));

	lua.set_global(lua_string!("steamworks"));

	0
}
