extern crate bindgen;

use bindgen::Builder;

use std::env;
use std::path::Path;

fn main() {
    // for (var, value) in env::vars() {
    //     eprintln!("{var}={value}", var=var, value=value);
    // }

    // println!("cargo:rustc-link-search=all=C:\\Users\\grani\\Documents\\Rust\\rust_olc_pge\\lib");
    // println!("cargo:rustc-link-lib=static=phonon");

    // let builder = Builder::default()
    //     .layout_tests(false)
    //     .size_t_is_usize(true)
    //     .header("headers/phonon.h")
    //     .rustified_enum("IPL(.*)");

    // let out_dir = ".";//env::var("OUT_DIR").unwrap();
    // let dest_path = Path::new("src/steam_audio_bindgen.rs");

    // // let bindings = builder.generate().unwrap();
    // // bindings.write_to_file(&dest_path).unwrap();

    // eprintln!("Generated Bindings => {:?}", dest_path);
}
