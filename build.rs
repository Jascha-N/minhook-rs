extern crate gcc;

use std::env;
use std::path::Path;

use gcc::Config;

fn main() {
    let root_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let target = env::var("TARGET").unwrap();

    let parts = target.splitn(4, '-').collect::<Vec<_>>();
    let arch = parts[0];
    let sys  = parts[2];

    if sys != "windows" {
        panic!("Platform `{}` not supported.", sys);
    }

    let hde_suffix = match arch {
        "i686"   => "32",
        "x86_64" => "64",
        _        => panic!("Architecture `{}` not supported.", arch)
    };

    let src_dir = Path::new(&root_dir).join("lib/minhook/src");

    Config::new()
           .file(src_dir.join("buffer.c"))
           .file(src_dir.join("hook.c"))
           .file(src_dir.join("trampoline.c"))
           .file(src_dir.join(format!("HDE/hde{}.c", hde_suffix)))
           .compile("libminhook.a");
}