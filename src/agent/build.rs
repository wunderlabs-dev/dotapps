use std::fs;
use std::io::Write;
use std::path::Path;

/// Copy file only if content differs, to avoid triggering Tauri's file watcher.
fn copy_if_changed(src: &Path, dest: &Path) {
    let new_content = fs::read(src).expect("failed to read generated file");
    if let Ok(existing) = fs::read(dest) {
        if existing == new_content {
            return;
        }
    }
    fs::write(dest, new_content).expect("failed to write generated file");
}

fn main() {
    // Only re-run codegen when proto or build script changes.
    println!("cargo:rerun-if-changed=../proto/agent.proto");
    println!("cargo:rerun-if-changed=build.rs");

    // Generate into OUT_DIR first to avoid touching src/generated/ unnecessarily.
    // Tauri's file watcher monitors src/agent/ and would trigger an infinite
    // rebuild loop if we wrote directly to src/generated/ every time.
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let temp_dir = Path::new(&out_dir).join("ttrpc_gen");
    fs::create_dir_all(&temp_dir).expect("failed to create temp dir");

    ttrpc_codegen::Codegen::new()
        .out_dir(temp_dir.to_str().expect("OUT_DIR path is not valid UTF-8"))
        .inputs(["../proto/agent.proto"])
        .include("../proto")
        .rust_protobuf()
        .run()
        .expect("failed to generate ttrpc code");

    // ttrpc-codegen generates agent_ttrpc.rs but does not include it in mod.rs.
    // Append the module declaration so the ttrpc client/server stubs are accessible.
    let temp_mod_path = temp_dir.join("mod.rs");
    let contents = fs::read_to_string(&temp_mod_path).expect("failed to read generated mod.rs");
    if !contents.contains("pub mod agent_ttrpc;") {
        let mut f = fs::OpenOptions::new()
            .append(true)
            .open(&temp_mod_path)
            .expect("failed to open generated mod.rs for appending");
        writeln!(f, "pub mod agent_ttrpc;").expect("failed to write to generated mod.rs");
    }

    // Copy to src/generated/ only if content actually changed.
    let gen_dir = Path::new("src/generated");
    for entry in fs::read_dir(&temp_dir).expect("failed to read temp dir") {
        let entry = entry.expect("failed to read dir entry");
        if entry.file_type().is_ok_and(|ft| ft.is_file()) {
            copy_if_changed(&entry.path(), &gen_dir.join(entry.file_name()));
        }
    }
}
