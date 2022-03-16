use std::{path::PathBuf, str::FromStr, io::Read};

struct SteamReqwest {
	client: reqwest::blocking::Client,
	cookies: String
}
impl SteamReqwest {
	fn new(cookies: String) -> Self {
		Self {
			client: reqwest::blocking::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap(),
			cookies
		}
	}

	fn get(&self, url: &str) -> Result<reqwest::blocking::Response, reqwest::Error> {
		self.client.execute(
			self.client.get(url)
			.header("cookie", &self.cookies)
			.header("User-Agent", "ayy lmao")
			.build().unwrap()
		)
	}
}

enum Platform {
	Win32,
	Win64,
	Linux32,
	Linux64,
	OSX
}
impl Platform {
	fn path(&self) -> &'static str {
		match self {
			Platform::Win32 => "sdk/redistributable_bin/steam_api.dll",
			Platform::Win64 => "sdk/redistributable_bin/win64/steam_api64.dll",
			Platform::Linux32 => "sdk/redistributable_bin/linux32/libsteam_api.so",
			Platform::Linux64 => "sdk/redistributable_bin/linux64/libsteam_api.so",
			Platform::OSX => "sdk/redistributable_bin/osx/libsteam_api.dylib"
		}
	}
}
impl FromStr for Platform {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"win32" => Ok(Platform::Win32),
			"win64" => Ok(Platform::Win64),
			"linux32" => Ok(Platform::Linux32),
			"linux64" => Ok(Platform::Linux64),
			"osx" => Ok(Platform::OSX),
			_ => Err(s.to_string()),
		}
	}
}

fn sha256(data: &[u8]) -> [u8; 32] {
	use sha2::Digest;
	let mut sha256 = sha2::Sha256::new();
	sha256.update(data);
	sha256.finalize().try_into().unwrap()
}

fn input() -> String {
	let mut input = String::new();
	std::io::stdin().read_line(&mut input).unwrap();
	input.trim().to_string()
}

fn main() {
	let platform = {
		println!("Platform [win32/win64/linux32/linux64/osx]:");
		Platform::from_str(&input()).expect("Unknown platform").path()
	};

	let dll = {
		println!("Path to Steam API DLL/SO:");
		let dll = PathBuf::from(input());

		if !dll.is_file() {
			eprintln!("DLL doesn't exist");
			std::process::exit(1);
		}

		sha256(&std::fs::read(dll).unwrap())
	};

	let reqwest = {
		println!("Steam cookies as header:");
		SteamReqwest::new(input())
	};

	let versions = {
		println!("Getting versions list...");

		let html = reqwest.get("https://partner.steamgames.com/downloads/list").unwrap();
		let html = String::from_utf8_lossy(&html.bytes().unwrap()).into_owned();

		println!("Parsing...");

		let versions = regex::Regex::new(r#"<a class="contentsLink" href="(.+?)">Download</a>"#).unwrap()
			.captures_iter(&html)
			.map(|cap| cap[1].to_string())
			.collect::<indexmap::IndexSet<_>>();

		println!("Found {} versions", versions.len());

		versions
	};

	for version in versions {
		println!("{version}");

		let zip = reqwest.get(&version).unwrap().bytes().unwrap();
		let mut zip = zip::read::ZipArchive::new(std::io::Cursor::new(zip)).unwrap();
		let file: Result<_, Box<dyn std::error::Error>> = zip.by_name(platform).map_err(Into::into).and_then(|mut f| {
			let mut buf = Vec::with_capacity(f.size() as usize);
			f.read_to_end(&mut buf)?;
			Ok(buf)
		});

		match file {
			Ok(file) => if sha256(&file) == dll {
				println!("Found!");
				std::process::exit(0);
			},

			Err(err) => {
				eprintln!("Failed to read {platform}: {err}");
			}
		}
	}
}