use std::{env, path::PathBuf, process::Command};
fn run(command: &mut Command) {
    let status = command
        .status()
        .expect("could not launch native build command");
    assert!(
        status.success(),
        "native build failed; run bash tools/setup-native.sh"
    );
}
fn main() {
    println!("cargo:rerun-if-changed=native/bullet_bridge.cpp");
    println!("cargo:rerun-if-changed=native/bullet_bridge.h");
    let home = env::var("HOME").unwrap_or_else(|_| "/home/oleh".to_string());
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let ndk_root = if PathBuf::from(format!("{home}/soft/android-ndk-r27d")).exists() {
            PathBuf::from(format!("{home}/soft/android-ndk-r27d"))
        } else {
            manifest_dir.join(".tools/android/android-ndk-r27d")
        };
        let tools = ndk_root.join("toolchains/llvm/prebuilt/linux-x86_64/bin");
        let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
        let (triple, suffix, abi) = match arch.as_str() {
            "aarch64" => ("aarch64-linux-android", "", "arm64-v8a"),
            "x86_64" => ("x86_64-linux-android", "-x86_64", "x86_64"),
            other => panic!("unsupported Android architecture {other}"),
        };
        let bullet_inc = PathBuf::from(format!("{home}/soft/android/bullet3/{abi}/include/bullet"));
        let bullet_include = if bullet_inc.exists() {
            bullet_inc
        } else {
            manifest_dir.join(".tools/bullet3/src")
        };
        let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
        run(Command::new(tools.join(format!("{triple}26-clang++")))
            .args(["-std=c++17", "-O2", "-fPIC", "-I"])
            .arg(&bullet_include)
            .args(["-c", "native/bullet_bridge.cpp", "-o"])
            .arg(out.join("bullet_bridge.o")));
        run(Command::new(tools.join("llvm-ar"))
            .arg("crs")
            .arg(out.join("libsmash_bullet.a"))
            .arg(out.join("bullet_bridge.o")));
        println!("cargo:rustc-link-search=native={}", out.display());
        println!("cargo:rustc-link-lib=static=smash_bullet");

        let bullet_lib = PathBuf::from(format!("{home}/soft/android/bullet3/{abi}/lib"));
        if bullet_lib.exists() {
            println!("cargo:rustc-link-search=native={}", bullet_lib.display());
        }
        for name in ["BulletDynamics", "BulletCollision", "LinearMath"] {
            if !bullet_lib.exists() {
                println!(
                    "cargo:rustc-link-search=native={}",
                    manifest_dir
                        .join(format!("build/native/bullet-build{suffix}/src/{name}"))
                        .display()
                );
            }
            println!("cargo:rustc-link-lib=static={name}");
        }
        println!(
            "cargo:rustc-link-search=native={}",
            manifest_dir
                .join(format!("build/native/sdl-build{suffix}"))
                .display()
        );
        println!("cargo:rustc-link-lib=dylib=SDL3");
        let native = env::var_os("SMASH_FILAMENT_NATIVE")
            .map(PathBuf::from)
            .unwrap_or_else(|| manifest_dir.join(format!("build/native/filament-build{suffix}")));
        println!("cargo:rerun-if-env-changed=SMASH_FILAMENT_NATIVE");
        println!(
            "cargo:rerun-if-changed={}",
            native.join("libsmash_filament.so").display()
        );
        println!("cargo:rustc-link-search=native={}", native.display());
        println!("cargo:rustc-link-lib=dylib=smash_filament");
        println!("cargo:rustc-link-lib=dylib=android");
        let system_libs = ndk_root.join(format!(
            "toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/{triple}"
        ));
        // Search the API-specific shared libraries before the generic static archives.
        println!(
            "cargo:rustc-link-search=native={}",
            system_libs.join("26").display()
        );
        println!("cargo:rustc-link-search=native={}", system_libs.display());
        println!("cargo:rustc-link-lib=static=c++_static");
        println!("cargo:rustc-link-lib=static=c++abi");
        for system_library in ["c", "m", "dl"] {
            println!("cargo:rustc-link-lib=dylib={system_library}");
        }
        println!("cargo:rustc-link-arg=-Wl,--no-undefined");
        return;
    }
    let host_bullet_inc = PathBuf::from(format!("{home}/.local/include/bullet"));
    let bullet_include = if host_bullet_inc.exists() {
        host_bullet_inc
    } else {
        PathBuf::from(".tools/bullet3/src")
    };
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    run(Command::new("c++")
        .args(["-std=c++17", "-O2", "-fPIC", "-I"])
        .arg(&bullet_include)
        .args(["-c", "native/bullet_bridge.cpp", "-o"])
        .arg(out.join("bullet_bridge.o")));
    run(Command::new("ar")
        .arg("crs")
        .arg(out.join("libsmash_bullet.a"))
        .arg(out.join("bullet_bridge.o")));
    println!("cargo:rustc-link-search=native={}", out.display());
    let host_bullet_lib = PathBuf::from(format!("{home}/.local/lib"));
    if host_bullet_lib.exists() {
        println!(
            "cargo:rustc-link-search=native={}",
            host_bullet_lib.display()
        );
    }
    for lib in ["BulletDynamics", "BulletCollision", "LinearMath"] {
        if !host_bullet_lib.join("libBulletDynamics.a").exists() {
            println!("cargo:rustc-link-search=native=.tools/bullet-build/src/{lib}");
        }
    }
    for lib in [
        "smash_bullet",
        "BulletDynamics",
        "BulletCollision",
        "LinearMath",
    ] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    println!("cargo:rustc-link-lib=dylib=stdc++");
}
