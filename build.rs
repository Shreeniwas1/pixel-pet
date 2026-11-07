// Embeds the app icon and version info into the exe by compiling assets/pixel-pet.rc with
// `windres` (MinGW targets) or `rc` (MSVC targets). If the tool is missing the build still
// succeeds, just without the icon.
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/pixel-pet.rc");
    println!("cargo:rerun-if-changed=assets/pixel-pet.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let (obj, status) = if msvc {
        let res = out.join("pixel-pet.res");
        (res.clone(), Command::new("rc").args(["/nologo", "/fo"]).arg(&res).arg("assets/pixel-pet.rc").status())
    } else {
        let obj = out.join("pixel-pet-res.o");
        (
            obj.clone(),
            Command::new("windres")
                .args(["--input", "assets/pixel-pet.rc", "--output-format=coff", "--output"])
                .arg(&obj)
                .args(["--include-dir", "assets"])
                .status(),
        )
    };
    match status {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bins={}", obj.display()),
        other => println!("cargo:warning=could not embed the icon (resource compiler failed: {other:?})"),
    }
}
