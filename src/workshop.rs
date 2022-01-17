use gmod::lua::LuaReference;
use std::{collections::HashMap, mem::ManuallyDrop, path::PathBuf};
use steamworks::PublishedFileId;

use crate::util;

macro_rules! check_installed {
	($ugc:ident, $workshop_id:expr) => {
		if let (Some(info), true) = (
			$ugc.item_install_info($workshop_id),
			$ugc.item_state($workshop_id).contains(steamworks::ItemState::INSTALLED),
		) {
			Some(info.folder)
		} else {
			None
		}
	};
}

pub mod downloads {
	use super::*;

	/// Find a GMA file in a directory, **will decompress if needed**
	fn get_gma<P: Into<PathBuf>>(path: P) -> Result<Option<PathBuf>, std::io::Error> {
		let path = path.into();
		let mut compressed = None;

		if path.is_dir() {
			let candidates = path
				.read_dir()?
				.filter_map(|entry| entry.ok())
				.filter_map(|entry| {
					if entry.file_type().ok()?.is_file() {
						Some(entry.path())
					} else {
						None
					}
				})
				.filter_map(|entry| {
					entry.extension().map(|ext| ext.to_owned()).map(|ext| (entry, ext))
				});

			for (path, ext) in candidates {
				let (path, ext) = dbg!((path, ext));

				if std::intrinsics::likely(ext.eq_ignore_ascii_case("gma")) {
					// We have a GMA!
					return Ok(Some(path));
				}

				if std::intrinsics::likely(ext.eq_ignore_ascii_case("bin")) {
					// We'll need to decompress this
					compressed = Some(path);
					break;
				}

				if std::intrinsics::unlikely(compressed.replace(path).is_some()) {
					// Panic!!!
					return Ok(None);
				}
			}
		} else {
			if path.extension().map(|ext| ext.eq_ignore_ascii_case("gma")).unwrap_or(false) {
				// We have a GMA!
				return Ok(Some(path));
			}

			// Let's try decompressing this
			compressed = Some(path);
		}

		let compressed = match compressed {
			Some(compressed) => compressed,
			None => return Ok(None)
		};

		let decompressed = compressed.with_extension("gma");
		if decompressed.is_file() {
			return Ok(Some(decompressed));
		}

		std::fs::write(&decompressed, {
			let decompressed = gmod_lzma::decompress(&std::fs::read(compressed)?).map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
			if !decompressed.starts_with(b"GMAD") {
				return Err(std::io::ErrorKind::InvalidData.into());
			}
			decompressed
		})?;

		Ok(Some(decompressed))
	}

	fn callback(lua: gmod::lua::State, callback: Option<LuaReference>, folder: Option<String>) {
		if let Some(callback) = callback {
			unsafe {
				lua.from_reference(callback);
				lua.dereference(callback);

				if let Some(Ok(Some(gma))) = folder.map(get_gma) {
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
						eprintln!("[gmsv_workshop] Failed to find relative path for {}, please let me know here: https://github.com/WilliamVenner/gmsv_workshop/issues/new", gma.display());
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
					lua.push_string("gmsv_workshop_queued");
					lua.push_function(Self::process_queued);
					lua.call(3, 0);
					lua.pop();
				}

				self.queued.insert(workshop_id, callback);

				println!("[gmsv_workshop] Queued {}", workshop_id);
				return;
			}

			let success = {
				ugc.suspend_downloads(false);
				ugc.download_item(workshop_id, true)
			};
			if !success {
				eprintln!(
					"[gmsv_workshop] Item ID {} is invalid or the server is not logged onto Steam",
					workshop_id
				);
				return self::callback(lua, callback, None);
			}

			if let Some(folder) = check_installed!(ugc, workshop_id) {
				return self::callback(lua, callback, Some(folder));
			}

			println!("[gmsv_workshop] Downloading {}", workshop_id);

			if let Some(callback) = callback {
				self.pending.insert(workshop_id, callback);
			}

			unsafe {
				lua.get_global(lua_string!("timer"));
				lua.get_field(-1, lua_string!("Create"));
				lua.push_string("gmsv_workshop");
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
					lua.push_string("gmsv_workshop_queued");
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
					if let Some(folder) = dbg!(check_installed!(ugc, *workshop_id)) {
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

				// On Linux64 Valve packs and aligns the struct to 4 bytes, we need to do an unaligned read on the SteamID64 :(
				#[cfg(all(target_os = "linux", target_pointer_width = "64"))] {
					lua.push_string(&std::ptr::read_unaligned(std::ptr::addr_of!(info.m_ulSteamIDOwner)).to_string());
				}
				#[cfg(not(all(target_os = "linux", target_pointer_width = "64")))] {
					lua.push_string(&info.m_ulSteamIDOwner.to_string());
				}
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
				#[cfg(debug_assertions)]
				assert_eq!(thread_id, std::thread::current().id());

				callbacks::pop();
				self::callback(crate::lua(), callback, workshop_id, result.map_err(Some));
			});

			callbacks::push();
		}
	}
}

pub struct Steam {
	pub server: ManuallyDrop<steamworks::Server>,
	pub callbacks: ManuallyDrop<steamworks::SingleClient<steamworks::ServerManager>>,
	pub pending: HashMap<PublishedFileId, LuaReference>,
	pub queued: HashMap<PublishedFileId, Option<LuaReference>>,
}
impl Steam {
	pub fn init() -> Steam {
		let (server, callbacks) = unsafe {
			steamworks::Server::from_raw(steamworks::sys_gameserver!())
		};

		let steam = Steam {
			pending: Default::default(),
			queued: Default::default(),
			server: ManuallyDrop::new(server),
			callbacks: ManuallyDrop::new(callbacks)
		};

		steam
	}
}
