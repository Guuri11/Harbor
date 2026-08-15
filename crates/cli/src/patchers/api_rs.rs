use std::{fs, path::Path};

pub fn patch_api_rs(base: &Path, snake: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = base.join("presentation/src/api.rs");
    let mut src = fs::read_to_string(&path)?;
    let original = src.clone();

    // Every generated `routes.rs` imports `crate::api::tags::ApiTags`, and `regenerate_bootstrap`
    // always writes `api/tags.rs` — so the declaration must exist whatever state `api.rs` is in
    // (stripped by `--no-demo`, hand-edited by the user).
    if !src.contains("pub mod tags;") {
        if !src.ends_with('\n') && !src.is_empty() {
            src.push('\n');
        }
        src.push_str("pub mod tags;\n");
    }

    let mod_line = format!("pub mod {snake};\n");
    if !src.contains(&mod_line) {
        if !src.ends_with('\n') && !src.is_empty() {
            src.push('\n');
        }
        src.push_str(&mod_line);
    }

    if src != original {
        fs::write(&path, src)?;
    }
    Ok(())
}
