//! Copyright (c) 2026 Bitdancer (github.com/RealBitdancer).
//! SPDX-License-Identifier: MIT

use std::env;
use std::path::{Path, PathBuf};

const OPAL_VERSION: &str = "3.0.0";

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let opal_src = resolve_opal_src(&manifest_dir);
    let include_dir = opal_src.join("include");
    let opal_c = opal_src.join("src").join("opal.c");
    let opal_h = include_dir.join("opal").join("opal.h");
    let abi_check = manifest_dir.join("src").join("abi_check.c");

    assert!(
        opal_c.is_file(),
        "opal {OPAL_VERSION} source not found at {}",
        opal_c.display()
    );
    assert!(
        opal_h.is_file(),
        "opal {OPAL_VERSION} header not found under {}",
        include_dir.display()
    );

    println!("cargo::rerun-if-env-changed=OPAL_SRC");
    println!("cargo::rerun-if-changed={}", opal_c.display());
    println!("cargo::rerun-if-changed={}", opal_h.display());
    println!("cargo::rerun-if-changed={}", abi_check.display());
    println!("cargo::metadata=include={}", include_dir.display());
    println!("cargo::metadata=version={OPAL_VERSION}");

    cc::Build::new()
        .file(&opal_c)
        .include(&include_dir)
        .std("c11")
        .warnings(false)
        .compile("opal");

    cc::Build::new()
        .file(&abi_check)
        .include(&include_dir)
        .std("c11")
        .warnings(false)
        .cargo_metadata(false)
        .compile("opal_abi");
    println!("cargo::rustc-link-search=native={}", out_dir.display());
}

fn resolve_opal_src(manifest_dir: &Path) -> PathBuf {
    if let Ok(path) = env::var("OPAL_SRC") {
        let path = PathBuf::from(path);
        assert!(
            path.is_dir(),
            "OPAL_SRC is set but is not a directory: {}",
            path.display()
        );
        return path;
    }

    manifest_dir.join("vendor").join("opal")
}
