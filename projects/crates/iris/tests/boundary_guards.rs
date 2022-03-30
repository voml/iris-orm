//! Phase 0 boundary guards: no VOS vendoring, no SQL public surface.

use std::fs;
use std::path::{Path, PathBuf};

/// Cargo workspace root (`projects/iris.rs`).
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("workspace root")
}

/// Product repo root (`iris-orm/`).
fn product_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("product root")
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "target" || name == ".git" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn does_not_vendor_vos_language_sources() {
    let root = product_root();
    for forbidden in [
        "projects/vos.rs",
        "projects/vos.ts",
        "backends/vos",
        "backends/vos-ast",
        "backends/vos-parser",
    ] {
        let path = root.join(forbidden);
        assert!(
            !path.exists(),
            "VOS language sources must not be vendored at {}",
            path.display()
        );
    }
}

#[test]
fn public_iris_crate_has_no_sql_dependencies() {
    let manifest =
        fs::read_to_string(workspace_root().join("iris/Cargo.toml")).expect("iris Cargo.toml");
    for banned in [
        "sqlx",
        "diesel",
        "sea-orm",
        "postgres",
        "mysql",
        "rusqlite",
        "sqlite",
        "tiberius",
        "tokio-postgres",
        "mysql_async",
        "sqlparser",
    ] {
        assert!(
            !manifest.contains(banned),
            "public iris facade must not depend on `{banned}`"
        );
    }
}

#[test]
fn workspace_depends_on_external_vos_facade_only() {
    let workspace =
        fs::read_to_string(workspace_root().join("Cargo.toml")).expect("workspace toml");
    let deps_section = workspace
        .split("[workspace.dependencies]")
        .nth(1)
        .unwrap_or(&workspace);
    assert!(
        deps_section
            .lines()
            .any(|l| l.trim_start().starts_with("vos =")),
        "workspace must declare the public `vos` dependency"
    );
    for banned in [
        "vos-ast =",
        "vos-parser =",
        "vos-inspect =",
        "vos-generator =",
    ] {
        assert!(
            !deps_section.contains(banned),
            "workspace must not depend on internal VOS crate `{banned}`"
        );
    }
    let vos_line = deps_section
        .lines()
        .find(|l| l.trim_start().starts_with("vos ="))
        .expect("vos dependency line");
    assert!(
        vos_line.contains("vos-language"),
        "vos must resolve via voml/vos-language git facade"
    );
    assert!(
        !workspace_root().join("vos").exists(),
        "vos must not be vendored under projects/iris.rs"
    );
}

#[test]
fn public_sources_forbid_sql_product_surface() {
    let ws = workspace_root();
    let mut files = Vec::new();
    collect_files(&ws.join("iris/src"), &mut files);
    collect_files(&ws.join("iris-types/src"), &mut files);
    collect_files(&ws.join("iris-ir/src"), &mut files);

    let patterns = [
        "SELECT ",
        "FROM ",
        "INSERT INTO",
        "query builder",
        "sqlx::",
        "SqlQuery",
        "SQL AST",
    ];

    for path in files {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        // Legacy invariant docs lived under documentation/; product docs are external now.
        if path.ends_with("no-sql-invariant.md") {
            continue;
        }
        for pat in patterns {
            assert!(
                !text.contains(pat),
                "{} must not contain public SQL surface marker `{pat}`",
                path.display()
            );
        }
    }
}

#[test]
fn facade_does_not_reexport_adapters() {
    let lib = fs::read_to_string(workspace_root().join("iris/src/lib.rs")).expect("iris lib");
    for banned in [
        "iris_adapter_",
        "iris_connector_",
        "mysql",
        "postgres",
        "sqlite",
        "redis",
        "rusqlite",
        "sqlx",
    ] {
        assert!(
            !lib.contains(banned),
            "iris facade must not re-export or name adapter internals (`{banned}`)"
        );
    }
}

#[test]
fn types_and_ir_have_no_sql_driver_dependencies() {
    for crate_name in ["iris-types", "iris-ir"] {
        let manifest =
            fs::read_to_string(workspace_root().join(format!("{crate_name}/Cargo.toml")))
                .unwrap_or_else(|_| panic!("{crate_name} Cargo.toml"));
        for banned in [
            "rusqlite",
            "sqlx",
            "diesel",
            "postgres",
            "mysql",
            "sqlparser",
            "tiberius",
        ] {
            assert!(
                !manifest.contains(banned),
                "{crate_name} must not depend on SQL driver `{banned}`"
            );
        }
    }
}

#[test]
fn yydb_connector_does_not_depend_on_foreign_sql_adapters() {
    let manifest = fs::read_to_string(workspace_root().join("iris-connector-yydb/Cargo.toml"))
        .expect("yydb connector Cargo.toml");
    for banned in [
        "iris-adapter-sqlite",
        "iris-adapter-postgres",
        "iris-adapter-mysql",
        "rusqlite",
        "postgres",
        "mysql",
    ] {
        assert!(
            !manifest.contains(banned),
            "YYDB native connector must not depend on `{banned}`"
        );
    }
}

#[test]
fn postgres_and_mysql_adapters_are_separate_crates() {
    let pg = fs::read_to_string(workspace_root().join("iris-adapter-postgres/Cargo.toml"))
        .expect("postgres Cargo.toml");
    let my = fs::read_to_string(workspace_root().join("iris-adapter-mysql/Cargo.toml"))
        .expect("mysql Cargo.toml");
    assert!(pg.contains("postgres") && pg.contains("r2d2"));
    assert!(!pg.contains("mysql"));
    assert!(!pg.contains("rusqlite"));
    assert!(my.contains("mysql"));
    assert!(!my.contains("postgres"));
    assert!(!my.contains("rusqlite"));
}

#[test]
fn redis_adapter_is_keyspace_only() {
    let manifest = fs::read_to_string(workspace_root().join("iris-adapter-redis/Cargo.toml"))
        .expect("redis Cargo.toml");
    assert!(manifest.contains("redis"));
    for banned in ["rusqlite", "postgres", "mysql", "sqlx"] {
        assert!(
            !manifest.contains(banned),
            "redis adapter must not pull relational SQL driver `{banned}`"
        );
    }
}

#[test]
fn typescript_tree_has_no_iris_adapter_packages() {
    let ts_root = product_root().join("projects/iris.ts");
    assert!(
        ts_root.is_dir(),
        "expected TypeScript tree at {}",
        ts_root.display()
    );
    for entry in fs::read_dir(&ts_root).expect("read projects/iris.ts") {
        let entry = entry.expect("iris.ts dir entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        assert!(
            !name.starts_with("iris-adapter-"),
            "TypeScript iris-adapter packages are retired; remove `{}`",
            entry.path().display()
        );
    }
}

#[test]
fn yyds_connector_stays_gated_without_sql() {
    let manifest = fs::read_to_string(workspace_root().join("iris-connector-yyds/Cargo.toml"))
        .expect("yyds connector Cargo.toml");
    for banned in [
        "yydb",
        "rusqlite",
        "postgres",
        "mysql",
        "yyds-odbc",
        "oak-sql",
    ] {
        assert!(
            !manifest.contains(banned),
            "gated YYDS connector must not depend on `{banned}`"
        );
    }
}
