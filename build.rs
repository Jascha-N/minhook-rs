use std::{mem, env, fs, io};
use std::fs::File;
use std::io::prelude::*;
use std::process::Command;
use std::path::{Path, PathBuf};

fn copy_recursive(from: &Path, to: &Path) -> io::Result<()> {
    if try!(fs::metadata(from)).is_dir() {
        try!(fs::create_dir(to));
        for entry in try!(fs::read_dir(from)) {
            let entry = try!(entry);
            let mut to = PathBuf::from(to);
            to.push(entry.file_name());
            try!(copy_recursive(&entry.path(), &to));
        }
    } else {
        try!(fs::copy(from, to));
    }

    Ok(())
}

fn patch_project(project: &Path) {
    let mut src = File::open(project).unwrap();
    let mut data = String::new();
    src.read_to_string(&mut data).unwrap();
    mem::drop(src);

    let data = data.replace("<RuntimeLibrary>MultiThreadedDebug</RuntimeLibrary>",
                            "<RuntimeLibrary>MultiThreadedDebugDLL</RuntimeLibrary>")
                   .replace("<RuntimeLibrary>MultiThreaded</RuntimeLibrary>",
                            "<RuntimeLibrary>MultiThreadedDLL</RuntimeLibrary>");

    let mut dst = File::create(project).unwrap();
    dst.write_all(data.as_bytes()).unwrap();
}

fn main() {
    let root_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir  = env::var("OUT_DIR").unwrap();
    let target   = env::var("TARGET").unwrap();

    let parts = target.splitn(4, '-').collect::<Vec<_>>();
    let arch = parts[0];
    let sys  = parts[2];
    let abi  = parts[3];

    if sys != "windows" {
        panic!("Platform '{}' not supported.", sys);
    }

    let platform = match arch {
        "i686"   => "Win32",
        "x86_64" => "x64",
        _        => panic!("Architecture '{}' not supported.", arch)
    };

    let _ = fs::remove_dir_all(&out_dir);
    copy_recursive(&Path::new(&root_dir).join("minhook"), out_dir.as_ref()).expect("Error copying sources");

    match abi {
        "gnu" => {
            let status = Command::new("make")
                                 .current_dir(&out_dir)
                                 .arg("-f")
                                 .arg("build/MinGW/Makefile")
                                 .arg("libMinHook.a")
                                 .status()
                                 .expect("Error executing make");

            if !status.success() {
                panic!("'make' exited with code: {}.", status.code().unwrap());
            }
        }

        "msvc" => {
            let profile = env::var("PROFILE").unwrap();

            let version = match env::var("VisualStudioVersion").ok().as_ref().map(|s| s as &str) {
                Some("14.0") => "VC14",
                Some("12.0") => "VC12",
                //Some("11.0") => "VC11",
                //Some("10.0") => "VC10",
                //Some("9.0")  => "VC9",
                Some(_)      => panic!("Unsupported Visual Studio version."),
                None         => panic!("'VisualStudioVersion' environment variable not set or malformed.")
            };

            let mut project_path = PathBuf::from(&out_dir);
            project_path.push("build");
            project_path.push(&version);
            project_path.push("libMinHook.vcxproj");

            patch_project(&project_path);

            let status = Command::new("MSBuild")
                                 .arg("/nologo")
                                 .arg("/property:TargetName=MinHook")
                                 .arg(&format!("/property:Configuration={}", profile))
                                 .arg(&format!("/property:Platform={}", platform))
                                 .arg(&format!("/property:OutDir={}\\", out_dir))
                                 .arg(&project_path)
                                 .status()
                                 .expect("Error executing MSBuild");

            if !status.success() {
                panic!("'MSBuild' exited with code: {}.", status.code().unwrap());
            }
        }

        abi =>
            panic!("ABI '{}' not supported.", abi)
     };

     println!("cargo:rustc-link-search=native={}", out_dir);
     println!("cargo:rustc-link-lib=MinHook");
}