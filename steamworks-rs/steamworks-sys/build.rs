#[macro_use] extern crate build_cfg;

#[build_cfg_main]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build.rs");

    use std::env;
    use std::path::{Path, PathBuf};
    use std::fs::{self};

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let sdk_loc = "../../lib/steamworks_157";
    let sdk_loc = Path::new(&sdk_loc);
    println!("cargo:rerun-if-env-changed=STEAM_SDK_LOCATION");

    let triple = env::var("TARGET").unwrap();
    let mut lib = "steam_api";
    let mut link_path = sdk_loc.join("redistributable_bin");
    if triple.contains("windows") {
        if !triple.contains("i686") {
            lib = "steam_api64";
            link_path.push("win64");
        }
    } else if triple.contains("linux") {
        if triple.contains("i686") {
            link_path.push("linux32");
        } else {
            link_path.push("linux64");
        }
    } else if triple.contains("darwin") {
        link_path.push("osx");
    } else {
        panic!("Unsupported OS");
    };

    if triple.contains("windows") {
        let dll_file = format!("{}.dll", lib);
        let lib_file = format!("{}.lib", lib);
        fs::copy(link_path.join(&dll_file), out_path.join(dll_file))?;
        fs::copy(link_path.join(&lib_file), out_path.join(lib_file))?;
    } else if triple.contains("darwin") {
        fs::copy(link_path.join("libsteam_api.dylib"), out_path.join("libsteam_api.dylib"))?;
    } else if triple.contains("linux") {
        fs::copy(link_path.join("libsteam_api.so"), out_path.join("libsteam_api.so"))?;
    }

    println!("cargo:rustc-link-search={}", out_path.display());
    println!("cargo:rustc-link-lib=dylib={}", lib);

	if build_cfg!(feature = "refresh-bindgen") {
		macro_rules! platform_bindings {
			($file:literal) => {{
				if build_cfg!(all(target_os = "windows", target_pointer_width = "32")) {
					concat!($file, "_", "win32", ".rs")
				} else if build_cfg!(all(target_os = "windows", target_pointer_width = "64")) {
					concat!($file, "_", "win64", ".rs")
				} else if build_cfg!(all(target_os = "linux", target_pointer_width = "32")) {
					concat!($file, "_", "linux32", ".rs")
				} else if build_cfg!(all(target_os = "linux", target_pointer_width = "64")) {
					concat!($file, "_", "linux64", ".rs")
				} else {
					unimplemented!()
				}
			}}
		}

		let bindings = bindgen::Builder::default()
			.header(sdk_loc.join("public/steam/steam_api_flat.h").to_string_lossy())
			.header(sdk_loc.join("public/steam/steam_gameserver.h").to_string_lossy())
			.clang_arg("-xc++")
			.clang_arg("-std=c++11")
			.clang_arg(format!("-I{}", sdk_loc.join("public").display()))
			.rustfmt_bindings(true)
			.default_enum_style(bindgen::EnumVariation::Rust {
				non_exhaustive: true
			})
			.generate()
			.expect("Unable to generate bindings");

		bindings
			.write_to_file(
				Path::new(platform_bindings!("src/bindings")).to_owned()
			)
			.expect("Couldn't write bindings!");
	}

    Ok(())
}
