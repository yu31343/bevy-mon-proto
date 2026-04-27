use std::{env, fs, path::PathBuf};

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should exist"));
    let fonts_dir = manifest_dir.join("assets").join("fonts");

    println!("cargo:rerun-if-changed={}", fonts_dir.display());

    let mut font_paths = fs::read_dir(&fonts_dir)
        .unwrap_or_else(|err| {
            panic!(
                "failed to read assets/fonts directory at {}: {err}",
                fonts_dir.display()
            )
        })
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            matches!(
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.to_ascii_lowercase())
                    .as_deref(),
                Some("ttf") | Some("otf") | Some("ttc")
            )
        })
        .collect::<Vec<_>>();
    font_paths.sort();

    let source_font = font_paths.first().unwrap_or_else(|| {
        panic!(
            "no font file found in {}. Put at least one .ttf/.otf/.ttc file under assets/fonts before building",
            fonts_dir.display()
        )
    });

    println!("cargo:rerun-if-changed={}", source_font.display());
    println!(
        "cargo:warning=Embedding UI font from {}",
        source_font.display()
    );

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR should exist"));
    let embedded_font_path = out_dir.join("embedded_ui_font.bin");

    fs::copy(source_font, &embedded_font_path).unwrap_or_else(|err| {
        panic!(
            "failed to copy font from {} to {}: {err}",
            source_font.display(),
            embedded_font_path.display()
        )
    });
}
