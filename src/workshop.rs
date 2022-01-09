use std::{net::Ipv4Addr, thread::JoinHandle, time::Duration, sync::{atomic::AtomicBool, Arc, Mutex}};

use gmod::lua::LuaReference;
use steamworks::{PublishedFileId, SteamError};

use crate::util::AtomicWriteOnceCell;

#[inline]
fn send_callback(callback: Option<LuaReference>, tx: &crossbeam::channel::Sender<(LuaReference, Result<String, ()>)>, result: Result<String, ()>) {
	if let Some(callback) = callback {
		let _ = tx.send((callback, result));
	}
}

pub struct Steam {
	tx: Option<crossbeam::channel::Sender<(PublishedFileId, Option<LuaReference>)>>,
	thread: Option<JoinHandle<()>>,
	result: Arc<AtomicWriteOnceCell<Result<(), steamworks::SteamError>>>,
}
impl Steam {
	pub fn new() -> (Steam, crossbeam::channel::Receiver<(LuaReference, Result<String, ()>)>) {
		let result = Arc::new(AtomicWriteOnceCell::uninit());
		let (tx, rx) = crossbeam::channel::unbounded();
		let (downloaded_tx, downloaded_rx) = crossbeam::channel::unbounded();

		let result_ref = result.clone();
		let thread = std::thread::spawn(move || unsafe {
			static CONNECTED: AtomicBool = AtomicBool::new(false);
			CONNECTED.store(false, std::sync::atomic::Ordering::Release);

			let (server, callbacks) = match steamworks::Server::init(Ipv4Addr::LOCALHOST, 0, 0, 0, steamworks::ServerMode::NoAuthentication, "0") {
				Ok(steam) => {
					result_ref.set(Ok(()));
					steam
				},
				Err(err) => {
					result_ref.set(Err(err));
					return;
				}
			};

			loop {
				let _connect = server.register_callback(|steam_callback: steamworks::SteamServersConnected| {
					CONNECTED.store(true, std::sync::atomic::Ordering::Release);
				});

				let _failed = server.register_callback(|steam_callback: steamworks::SteamServerConnectFailure| {
					println!("[gmsv_downloadugc] Failed to connect to Steam: {}, retrying...", steam_callback.reason);

					CONNECTED.store(false, std::sync::atomic::Ordering::Release);

					// TODO
				});

				server.log_on_anonymous();

				while !CONNECTED.load(std::sync::atomic::Ordering::Relaxed) {
					callbacks.run_callbacks();
					std::thread::sleep(Duration::from_millis(50));
				}

				let ugc = server.ugc();
				while let Ok((workshop_id, callback)) = rx.recv() {
					if let Some(info) = ugc.item_install_info(workshop_id) {
						send_callback(callback, &downloaded_tx, Ok(info.folder));
					} else {
						if !ugc.download_item(workshop_id, false) {
							eprintln!("[gmsv_downloadugc] Item ID {:?} is invalid or the server is not logged onto Steam", workshop_id);
							send_callback(callback, &downloaded_tx, Err(()));
							continue;
						}

						let download_result = Arc::new(AtomicBool::new(false));
						let download_result_ref = download_result.clone();
						let _download_result = server.register_callback(move |steam_callback: steamworks::DownloadItemResult| {
							if let Some(err) = steam_callback.error {
								eprintln!("[gmsv_downloadugc] Error downloading {:?}: {}", workshop_id, err);
							}
							download_result_ref.store(true, std::sync::atomic::Ordering::Release);
						});

						while !download_result.load(std::sync::atomic::Ordering::Relaxed) {
							callbacks.run_callbacks();
							std::thread::sleep(Duration::from_millis(50));
						}

						match server.ugc().item_install_info(workshop_id) {
							Some(info) => {
								send_callback(callback, &downloaded_tx, Ok(info.folder));
							},
							None => {
								eprintln!("[gmsv_downloadugc] Error downloading {:?}: item missing from disk?", workshop_id);
								send_callback(callback, &downloaded_tx, Err(()));
							}
						}
					}
				}
			}
		});

		(
			Steam {
				tx: Some(tx),
				thread: Some(thread),
				result
			},

			downloaded_rx
		)
	}

	pub fn download(&self, workshop_id: PublishedFileId, callback: Option<LuaReference>) {
		if let Some(ref tx) = self.tx {
			let _ = tx.send((workshop_id, callback));
		}
	}
}
impl Drop for Steam {
	fn drop(&mut self) {
		drop(self.tx.take());

		if let Some(thread) = self.thread.take() {
			let _ = thread.join();
		}
	}
}