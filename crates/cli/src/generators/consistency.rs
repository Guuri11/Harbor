//! Does `puerto.toml` describe the code that is actually on disk?
//!
//! `generated/bootstrap.rs` is written from the manifest alone: one `use` per entity, one per use
//! case, with no check that any of it exists. An entity declared but never scaffolded produced a
//! bootstrap full of E0433s — in the *generated* project, at `cargo build`, far from the command
//! that caused it.
//!
//! The list below is deliberately not "every file scaffold writes" (that is `conflict::entity_files`)
//! but exactly the files `bootstrap.rs` imports. Those are the ones whose absence does not compile.

use std::path::{Path, PathBuf};

use crate::generators::naming::pascal_to_snake;
use crate::puerto_toml::{Entity, PuertoConfig};

/// One entity whose declared code is not on disk.
#[derive(Debug)]
pub struct EntityDrift {
    pub entity: String,
    pub missing: Vec<PathBuf>,
}

/// The files `bootstrap.rs` will import for this entity.
pub fn bootstrap_required_files(entity: &Entity) -> Vec<PathBuf> {
    let snake = pascal_to_snake(&entity.name);
    let mut files = vec![
        PathBuf::from(format!("business/src/domain/{snake}/repository.rs")),
        PathBuf::from(format!("infrastructure/src/{snake}/repository.rs")),
        PathBuf::from(format!("presentation/src/api/{snake}/routes.rs")),
    ];
    files.extend(
        entity
            .use_cases
            .iter()
            .map(|uc| PathBuf::from(format!("business/src/application/{snake}/{uc}.rs"))),
    );
    files
}

/// Whether `base` looks like a generated project at all.
///
/// Without this, every unit test that writes a bare `puerto.toml` into a temp dir would report
/// every entity as drifted. A project with no `business/src/domain` has nothing to be
/// inconsistent *with*.
fn has_generated_tree(base: &Path) -> bool {
    base.join("business/src/domain").is_dir()
}

/// Entities declared in the manifest whose code is missing.
pub fn missing_entity_files(base: &Path, config: &PuertoConfig) -> Vec<EntityDrift> {
    if !has_generated_tree(base) {
        return vec![];
    }
    config
        .entity
        .iter()
        .filter_map(|entity| {
            let missing: Vec<PathBuf> = bootstrap_required_files(entity)
                .into_iter()
                .filter(|f| !base.join(f).exists())
                .collect();
            (!missing.is_empty()).then(|| EntityDrift {
                entity: entity.name.clone(),
                missing,
            })
        })
        .collect()
}

/// Domain modules on disk with no `[[entity]]` block — the inverse drift. Not an error: the code
/// compiles, it is just invisible to every Puerto command.
pub fn orphan_entity_dirs(base: &Path, config: &PuertoConfig) -> Vec<String> {
    if !has_generated_tree(base) {
        return vec![];
    }
    let declared: Vec<String> = config
        .entity
        .iter()
        .map(|e| pascal_to_snake(&e.name))
        .collect();

    let Ok(entries) = std::fs::read_dir(base.join("business/src/domain")) else {
        return vec![];
    };

    let mut orphans: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        // `shared` holds the shared value objects, not an entity.
        .filter(|name| name != "shared" && !declared.contains(name))
        .collect();
    orphans.sort();
    orphans
}

/// The command that fixes this drift.
///
/// Mid-way through the domain-first workflow the domain layer is already written and only the
/// later ones are missing — telling the user to run `scaffold` there is wrong twice over: it would
/// refuse (the files exist) and, forced, it would overwrite the domain they just wrote.
fn suggested_command(entity: &str, missing: &[PathBuf]) -> String {
    let is_missing = |needle: &str| missing.iter().any(|p| p.to_string_lossy().contains(needle));

    if is_missing("business/src/domain") {
        format!("puerto generate scaffold {entity}")
    } else if is_missing("business/src/application") {
        format!("puerto generate application {entity}")
    } else if is_missing("infrastructure/src") {
        format!("puerto generate repository {entity}")
    } else {
        format!("puerto generate presentation {entity}")
    }
}

/// One human-readable line per drifted entity, for error and warning output.
pub fn drift_lines(drifts: &[EntityDrift]) -> Vec<String> {
    drifts
        .iter()
        .map(|d| {
            let paths: Vec<String> = d.missing.iter().map(|p| p.display().to_string()).collect();
            format!(
                "entity '{}' is in puerto.toml but its code is missing ({}). Run `{}`",
                d.entity,
                paths.join(", "),
                suggested_command(&d.entity, &d.missing)
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_files_cover_every_use_case() {
        let entity = Entity {
            name: "OrderItem".into(),
            use_cases: vec!["create_order_item".into(), "list_order_item".into()],
            db: false,
            fields: vec![],
        };
        let files = bootstrap_required_files(&entity);

        assert!(files.contains(&PathBuf::from(
            "business/src/application/order_item/create_order_item.rs"
        )));
        assert!(files.contains(&PathBuf::from(
            "business/src/application/order_item/list_order_item.rs"
        )));
        assert!(files.contains(&PathBuf::from("presentation/src/api/order_item/routes.rs")));
    }

    #[test]
    fn suggests_scaffold_when_nothing_exists_and_the_next_layer_otherwise() {
        let all = vec![
            PathBuf::from("business/src/domain/invoice/repository.rs"),
            PathBuf::from("infrastructure/src/invoice/repository.rs"),
        ];
        assert_eq!(
            suggested_command("Invoice", &all),
            "puerto generate scaffold Invoice"
        );

        // Domain already written (not in the missing list) — the domain-first flow's next step.
        let mid_flow = vec![
            PathBuf::from("business/src/application/invoice/create_invoice.rs"),
            PathBuf::from("infrastructure/src/invoice/repository.rs"),
        ];
        assert_eq!(
            suggested_command("Invoice", &mid_flow),
            "puerto generate application Invoice"
        );

        let only_presentation = vec![PathBuf::from("presentation/src/api/invoice/routes.rs")];
        assert_eq!(
            suggested_command("Invoice", &only_presentation),
            "puerto generate presentation Invoice"
        );
    }

    #[test]
    fn a_directory_without_a_generated_tree_never_drifts() {
        let config = PuertoConfig {
            project: crate::puerto_toml::Project {
                name: "x".into(),
                db: false,
            },
            entity: vec![Entity {
                name: "Ghost".into(),
                use_cases: vec![],
                db: false,
                fields: vec![],
            }],
            value_object: vec![],
        };
        let dir = std::env::temp_dir();
        assert!(missing_entity_files(&dir, &config).is_empty());
    }
}
