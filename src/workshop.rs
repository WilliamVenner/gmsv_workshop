use gmod::lua::LuaReference;
use std::collections::HashMap;
use steamworks::PublishedFileId;

use crate::util;

macro_rules! check_installed {
	($ugc:ident, $workshop_id:expr) => {
		if let (Some(info), steamworks::ItemState::INSTALLED) = (
			$ugc.item_install_info($workshop_id),
			$ugc.item_state($workshop_id),
		) {
			Some(info.folder)
		} else {
			None
		}
	};
}

pub struct Steam {
	pub server: steamworks::Server,
	pub callbacks: HashMap<PublishedFileId, LuaReference>,
	pub queued: HashMap<PublishedFileId, Option<LuaReference>>,
}
impl Steam {
	pub fn init() -> Steam {
		let (server, _) = unsafe {
			steamworks::Server::from_raw(steamworks::sys::SteamAPI_SteamGameServer_v013())
		};

		let steam = Steam {
			callbacks: Default::default(),
			queued: Default::default(),
			server,
		};

		steam
	}

	pub fn download(&mut self, workshop_id: PublishedFileId, callback: Option<LuaReference>) {
		let lua = crate::lua();
		let ugc = self.server.ugc();

		if let Some(folder) = check_installed!(ugc, workshop_id) {
			return Self::callback(lua, callback, Some(folder));
		}

		if !self.server.is_logged_in() {
			unsafe {
				lua.get_global(lua_string!("hook"));
				lua.get_field(-1, lua_string!("Add"));
				lua.push_string("Think");
				lua.push_string("gmsv_downloadugc_queued");
				lua.push_function(Self::process_queued);
				lua.call(3, 0);
				lua.pop();
			}

			self.queued.insert(workshop_id, callback);

			println!("[gmsv_downloadugc] Queued {:?}", workshop_id);
			return;
		}

		let success = {
			ugc.suspend_downloads(false);
			ugc.download_item(workshop_id, true)
		};
		if !success {
			eprintln!(
				"[gmsv_downloadugc] Item ID {:?} is invalid or the server is not logged onto Steam",
				workshop_id
			);
			return Self::callback(lua, callback, None);
		}

		if let Some(folder) = check_installed!(ugc, workshop_id) {
			return Self::callback(lua, callback, Some(folder));
		}

		println!("[gmsv_downloadugc] Downloading {:?}", workshop_id);

		if let Some(callback) = callback {
			self.callbacks.insert(workshop_id, callback);
		}

		unsafe {
			lua.get_global(lua_string!("timer"));
			lua.get_field(-1, lua_string!("Create"));
			lua.push_string("gmsv_downloadugc");
			lua.push_integer(1);
			lua.push_integer(0);
			lua.push_function(Self::poll);
			lua.call(4, 0);
			lua.pop();
		}
	}

	extern "C-unwind" fn process_queued(lua: gmod::lua::State) -> i32 {
		crate::STEAM.with(|steam| {
			let steam = steam.get_mut();

			if !steam.server.is_logged_in() {
				return 0;
			};

			for (workshop_id, callback) in std::mem::take(&mut steam.queued) {
				steam.download(workshop_id, callback);
			}

			unsafe {
				lua.get_global(lua_string!("hook"));
				lua.get_field(-1, lua_string!("Remove"));
				lua.push_string("Think");
				lua.push_string("gmsv_downloadugc_queued");
				lua.call(2, 0);
				lua.pop();
			}

			0
		})
	}

	unsafe extern "C-unwind" fn poll(lua: gmod::lua::State) -> i32 {
		crate::STEAM.with(|steam| {
			let steam = steam.get_mut();
			let ugc = steam.server.ugc();

			steam.callbacks.drain_filter(|workshop_id, callback| {
				if let Some(folder) = check_installed!(ugc, *workshop_id) {
					Self::callback(lua, Some(*callback), Some(folder));
					true
				} else {
					false
				}
			});
		});

		0
	}

	fn callback(lua: gmod::lua::State, callback: Option<LuaReference>, folder: Option<String>) {
		if let Some(callback) = callback {
			unsafe {
				lua.from_reference(callback);
				lua.dereference(callback);

				if let Some(gma) = folder.map(util::find_gma).and_then(|res| res.ok().flatten()) {
					lua.push_string(gma.to_string_lossy().as_ref());

					if let Some(relative_path) = util::base_path_relative(&gma) {
						lua.get_global(lua_string!("file"));
						lua.get_field(-1, lua_string!("Open"));
						lua.push_string(relative_path.to_string_lossy().as_ref());
						lua.push_string("rb");
						lua.push_string("BASE_PATH");
						lua.call(3, 1);
						lua.remove(lua.get_top() - 1);
					} else {
						eprintln!("[gmsv_downloadugc] Failed to find relative path for {:?}, please let me know here: https://github.com/WilliamVenner/gmsv_downloadugc/issues/new", gma);
						lua.push_nil();
					}
				} else {
					lua.push_nil();
					lua.push_nil();
				}

				lua.pcall_ignore(2, 0);
			}
		}
	}
}

unsafe impl Sync for Steam {}
