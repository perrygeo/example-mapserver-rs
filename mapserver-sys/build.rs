extern crate bindgen;
extern crate fs_extra;
use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    println!("cargo:rustc-link-lib=dylib=mapserver");
    let bindings = bindgen::Builder::default()
        .header("wrapper.hpp")
        .allowlist_function("ms.*")
        .clang_args(vec![format!("-I{}/dist/include", &out_dir)])
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
