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

pub mod downloads {
	use super::*;

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

	impl Steam {
		pub fn download(&mut self, workshop_id: PublishedFileId, callback: Option<LuaReference>) {
			let lua = crate::lua();
			let ugc = self.server.ugc();

			if let Some(folder) = check_installed!(ugc, workshop_id) {
				return self::callback(lua, callback, Some(folder));
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
				return self::callback(lua, callback, None);
			}

			if let Some(folder) = check_installed!(ugc, workshop_id) {
				return self::callback(lua, callback, Some(folder));
			}

			println!("[gmsv_downloadugc] Downloading {:?}", workshop_id);

			if let Some(callback) = callback {
				self.pending.insert(workshop_id, callback);
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

				steam.pending.drain_filter(|workshop_id, callback| {
					if let Some(folder) = check_installed!(ugc, *workshop_id) {
						self::callback(lua, Some(*callback), Some(folder));
						true
					} else {
						false
					}
				});
			});

			0
		}
	}
}

pub mod query {
	use std::ffi::CStr;

use crate::callbacks;

use super::*;

	fn callback(lua: gmod::lua::State, callback: LuaReference, workshop_id: PublishedFileId, info: Result<steamworks::QueryResults, Option<steamworks::SteamError>>) {
		unsafe {
			lua.from_reference(callback);
			lua.dereference(callback);

			lua.new_table();

			lua.push_integer(workshop_id.0 as _);
			lua.set_field(-2, lua_string!("id"));

			loop {
				// NB: No idea where to put `-2 means Failed to send query`

				let info = match info {
					Err(None) => {
						// Failed to create query
						lua.push_integer(-1);
						lua.set_field(-2, lua_string!("error"));
						break;
					},

					Err(Some(err)) => {
						// Failed to get item data from the response
						lua.push_integer(Into::<steamworks::sys::EResult>::into(err) as i32 as _);
						lua.set_field(-2, lua_string!("error"));
						break;
					},

					Ok(info) => {
						let first = info.get(0);
						let next = info.get(1);
						match (first, next) {
							(Some(_), Some(_)) | (None, None) | (None, Some(_)) => {
								// Received 0 or more than 1 result
								lua.push_integer(-3);
								lua.set_field(-2, lua_string!("error"));
								break;
							},

							(Some(info), None) => info
						}
					}
				};

				if info.m_nPublishedFileId == 0 {
					// Workshop item ID in the response is invalid
					lua.push_integer(-5);
					lua.set_field(-2, lua_string!("error"));
					break;
				}

				if info.m_nPublishedFileId != workshop_id.0 {
					// Workshop item ID in response is mismatching the requested file ID
					lua.push_integer(-6);
					lua.set_field(-2, lua_string!("error"));
					break;
				}

				macro_rules! cstr_to_bytes {
					($expr:expr) => {
						CStr::from_ptr($expr.as_ptr() as *const _).to_bytes()
					}
				}

				lua.push_binary_string(cstr_to_bytes!(info.m_rgchTitle));
				lua.set_field(-2, lua_string!("title"));

				lua.push_binary_string(cstr_to_bytes!(info.m_rgchDescription));
				lua.set_field(-2, lua_string!("description"));

				lua.push_string(&info.m_ulSteamIDOwner.to_string());
				lua.set_field(-2, lua_string!("owner"));

				lua.push_binary_string(cstr_to_bytes!(info.m_rgchTags));
				lua.set_field(-2, lua_string!("owner"));

				lua.push_boolean(info.m_bBanned);
				lua.set_field(-2, lua_string!("banned"));

				lua.push_integer(info.m_rtimeCreated as _);
				lua.set_field(-2, lua_string!("created"));

				lua.push_integer(info.m_rtimeUpdated as _);
				lua.set_field(-2, lua_string!("updated"));

				lua.push_integer(info.m_nFileSize as _);
				lua.set_field(-2, lua_string!("size"));

				lua.push_binary_string(cstr_to_bytes!(info.m_rgchURL));
				lua.set_field(-2, lua_string!("previewurl"));

				lua.push_integer(info.m_hPreviewFile as _);
				lua.set_field(-2, lua_string!("previewid"));

				lua.push_integer(info.m_nPreviewFileSize as _);
				lua.set_field(-2, lua_string!("previewsize"));

				lua.push_integer(info.m_hFile as _);
				lua.set_field(-2, lua_string!("fileid"));

				break;
			}

			lua.pcall_ignore(1, 0);
		}
	}

	impl Steam {
		pub fn file_info(&self, workshop_id: PublishedFileId, callback: LuaReference) {
			let ugc = self.server.ugc();

			#[cfg(debug_assertions)]
			let thread_id = std::thread::current().id();

			let query = match ugc.query_item(workshop_id) {
				Ok(query) => query,
				res @ Err(_) => return self::callback(crate::lua(), callback, workshop_id, res.map_err(|_| None).map(|_| unreachable!()))
			};

			query.allow_cached_response(60).include_children(true).fetch(move |result| {
				debug_assert_eq!(thread_id, std::thread::current().id());
				callbacks::pop();
				self::callback(crate::lua(), callback, workshop_id, result.map_err(Some));
			});

			callbacks::push();
		}
	}
}

pub struct Steam {
	pub server: steamworks::Server,
	pub callbacks: steamworks::SingleClient<steamworks::ServerManager>,
	pub pending: HashMap<PublishedFileId, LuaReference>,
	pub queued: HashMap<PublishedFileId, Option<LuaReference>>,
}
impl Steam {
	pub fn init() -> Steam {
		let (server, callbacks) = unsafe {
			steamworks::Server::from_raw(steamworks::sys::SteamAPI_SteamGameServer_v013())
		};

		let steam = Steam {
			pending: Default::default(),
			queued: Default::default(),
			server, callbacks
		};

		steam
	}
}
