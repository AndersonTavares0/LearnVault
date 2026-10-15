//! Build script do crate desktop.
//!
//! `tauri_build::build()` lê `tauri.conf.json`, resolve o ACL a partir de
//! `capabilities/` e emite as instruções de recompilação. Ele não abre sockets nem
//! depende de display; a validação de runtime pertence ao `tauri dev`/`tauri build`.

fn main() {
    tauri_build::build()
}
