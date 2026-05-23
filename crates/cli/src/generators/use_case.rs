use std::path::Path;

use crate::generators::bootstrap::regenerate_bootstrap;
use crate::generators::naming::{pascal_to_snake, to_pascal_case, write_file};
use crate::generators::render::render;
use crate::patchers::lib_rs::patch_business_lib_use_case;

pub fn run_use_case(
    entity: &str,
    action: &str,
    base: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let pascal = to_pascal_case(entity);
    let snake = pascal_to_snake(&pascal);
    let uc = action.to_string();
    let uc_pascal = to_pascal_case(&uc);

    // Errors if entity not in puerto.toml
    crate::puerto_toml::add_use_case(base, &pascal, &uc)?;

    let mut ctx = tera::Context::new();
    ctx.insert("pascal", &pascal);
    ctx.insert("snake", &snake);
    ctx.insert("uc", &uc);
    ctx.insert("uc_pascal", &uc_pascal);

    write_file(
        &base.join(format!("business/src/domain/{snake}/use_cases/{uc}.rs")),
        &render("use_case/trait.tera", &ctx)?,
    )?;
    write_file(
        &base.join(format!("business/src/application/{snake}/{uc}.rs")),
        &render("use_case/impl.tera", &ctx)?,
    )?;

    patch_business_lib_use_case(base, &snake, &uc)?;
    regenerate_bootstrap(base)?;

    println!("✓ Use case {uc_pascal} added to {pascal} (2 files).");
    println!("  puerto.toml updated + bootstrap.rs regenerated.");

    Ok(())
}
