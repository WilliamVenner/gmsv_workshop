use gmod::lua::LuaReference;
use std::{cell::RefCell, collections::HashMap, mem::ManuallyDrop, path::PathBuf};
use steamworks::PublishedFileId;

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

	fn cache_gma<P: Into<PathBuf>>(workshop_id: PublishedFileId, path: P) -> Result<Option<()>, std::io::Error> {
		let path = path.into();

		let cache_path = PathBuf::from(format!("garrysmod/cache/srcds/{}.gma", workshop_id));
		if path == cache_path {
			return Ok(Some(()));
		}

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
				if std::intrinsics::likely(ext.eq_ignore_ascii_case("gma")) {
					// We have a GMA!
					std::fs::copy(&path, cache_path)?;
					return Ok(Some(()));
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
				std::fs::copy(path, cache_path)?;
				return Ok(Some(()));
			}

			// Let's try decompressing this
			compressed = Some(path);
		}

		let compressed = match compressed {
			Some(compressed) => compressed,
			None => return Ok(None)
		};

		std::fs::create_dir_all("garrysmod/cache/srcds")?;

		std::fs::write(&cache_path, {
			let decompressed = gmod_lzma::decompress(&std::fs::read(compressed)?).map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
			if !decompressed.starts_with(b"GMAD") {
				return Err(std::io::ErrorKind::InvalidData.into());
			}
			decompressed
		})?;

		Ok(Some(()))
	}

	fn callback(lua: gmod::lua::State, callback: Option<LuaReference>, workshop_id: PublishedFileId, folder: Option<String>) {
		if let Some(callback) = callback {
			unsafe {
				lua.from_reference(callback);
				lua.dereference(callback);

				match folder.map(|folder| cache_gma(workshop_id, folder)) {
					Some(Ok(Some(_))) => {
						let gma = format!("cache/srcds/{}.gma", workshop_id);

						lua.push_string(&gma);

						lua.get_global(lua_string!("file"));
						lua.get_field(-1, lua_string!("Open"));
						lua.push_string(&gma);
						lua.push_string("rb");
						lua.push_string("GAME");
						lua.call(3, 1);
						lua.remove(lua.get_top() - 1);
					},

					Some(Err(err)) => {
						eprintln!("[gmsv_workshop] Failed to process download: {}", err);
						lua.push_nil();
						lua.push_nil();
					},

					_ => {
						lua.push_nil();
						lua.push_nil();
					}
				}

				lua.pcall_ignore(2, 0);
			}
		}
	}

	impl Steam {
		pub fn download(&self, workshop_id: PublishedFileId, callback: Option<LuaReference>) {
			let lua = crate::lua();
			let ugc = self.server.ugc();

			{
				let cache_path = format!("garrysmod/cache/srcds/{}.gma", workshop_id);
				if PathBuf::from(&cache_path).is_file() {
					return self::callback(lua, callback, workshop_id, Some(cache_path));
				}
			}

			if let Some(folder) = check_installed!(ugc, workshop_id) {
				return self::callback(lua, callback, workshop_id, Some(folder));
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

				self.queued.borrow_mut().insert(workshop_id, callback);

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
				return self::callback(lua, callback, workshop_id, None);
			}

			if let Some(folder) = check_installed!(ugc, workshop_id) {
				return self::callback(lua, callback, workshop_id, Some(folder));
			}

			println!("[gmsv_workshop] Downloading {}", workshop_id);

			if let Some(callback) = callback {
				self.pending.borrow_mut().insert(workshop_id, callback);
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
				if !steam.server.is_logged_in() {
					return 0;
				}

				for (workshop_id, callback) in steam.queued.take() {
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
			let mut queue = Vec::new();

			crate::STEAM.with(|steam| {
				let ugc = steam.server.ugc();

				steam.pending.borrow_mut().retain(|workshop_id, callback| {
					if let Some(folder) = check_installed!(ugc, *workshop_id) {
						queue.push((*callback, *workshop_id, folder));
						false
					} else {
						true
					}
				});
			});

			for (callback, workshop_id, folder) in queue {
				self::callback(lua, Some(callback), workshop_id, Some(folder));
			}

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

			lua.push_string(&workshop_id.0.to_string());
			lua.set_field(-2, lua_string!("id"));

			loop {
				// NB: No idea where to put `-2 means Failed to send query`

				let (info, children) = match info {
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

							(Some(details), None) => (details, info.get_children(0).unwrap_or_default()),
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

				// On Linux64 Valve packs and aligns the struct to 4 bytes, we need to do an unaligned read on some fields :(
				if cfg!(all(target_os = "linux", target_pointer_width = "64")) {
					lua.push_string(&std::ptr::read_unaligned(std::ptr::addr_of!(info.m_ulSteamIDOwner)).to_string());
					lua.set_field(-2, lua_string!("owner"));

					lua.push_string(&std::ptr::read_unaligned(std::ptr::addr_of!(info.m_hPreviewFile)).to_string());
					lua.set_field(-2, lua_string!("previewid"));

					lua.push_string(&std::ptr::read_unaligned(std::ptr::addr_of!(info.m_hFile)).to_string());
					lua.set_field(-2, lua_string!("fileid"));
				} else {
					lua.push_string(&info.m_ulSteamIDOwner.to_string());
					lua.set_field(-2, lua_string!("owner"));

					lua.push_string(&info.m_hPreviewFile.to_string());
					lua.set_field(-2, lua_string!("previewid"));

					lua.push_string(&info.m_hFile.to_string());
					lua.set_field(-2, lua_string!("fileid"));
				}

				lua.push_binary_string(cstr_to_bytes!(info.m_rgchTags));
				lua.set_field(-2, lua_string!("tags"));

				lua.push_boolean(info.m_bBanned);
				lua.set_field(-2, lua_string!("banned"));

				lua.push_number(info.m_rtimeCreated as _);
				lua.set_field(-2, lua_string!("created"));

				lua.push_number(info.m_rtimeUpdated as _);
				lua.set_field(-2, lua_string!("updated"));

				lua.push_number(info.m_nFileSize as _);
				lua.set_field(-2, lua_string!("size"));

				lua.push_binary_string(cstr_to_bytes!(info.m_rgchURL));
				lua.set_field(-2, lua_string!("previewurl"));

				lua.push_number(info.m_nPreviewFileSize as _);
				lua.set_field(-2, lua_string!("previewsize"));

				lua.push_number(info.m_unVotesUp as _);
				lua.set_field(-2, lua_string!("up"));

				lua.push_number(info.m_unVotesDown as _);
				lua.set_field(-2, lua_string!("down"));

				lua.push_number((info.m_unVotesUp as u64 + info.m_unVotesDown as u64) as _);
				lua.set_field(-2, lua_string!("total"));

				lua.push_number(info.m_flScore as _);
				lua.set_field(-2, lua_string!("score"));

				lua.create_table(children.len() as _, 0);
				for (i, child) in children.into_iter().enumerate() {
					lua.push_string(&child.0.to_string());
					lua.raw_seti(-2, (i + 1) as _);
				}
				lua.set_field(-2, lua_string!("children"));

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
	pub pending: RefCell<HashMap<PublishedFileId, LuaReference>>,
	pub queued: RefCell<HashMap<PublishedFileId, Option<LuaReference>>>,
}
impl Steam {
	pub fn init() -> Steam {
		let (server, callbacks) = unsafe {
			steamworks::Server::from_raw({
				steamworks::sys::SteamAPI_SteamGameServer_v015()
			})
		};

		Steam {
			pending: Default::default(),
			queued: Default::default(),
			server: ManuallyDrop::new(server),
			callbacks: ManuallyDrop::new(callbacks)
		}
	}
}
