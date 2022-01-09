use iris_types::{DatasourceConfig, DatasourceKind, IrisProject, TruthMode};
fn main() {
    let mut p = IrisProject::new();
    p.datasources.insert("main".into(), DatasourceConfig {
        kind: DatasourceKind::Mysql,
        mode: TruthMode::ManagedPush,
        path: None,
        url: Some("$MYSQL_URL".into()),
    });
    println!("{}", p.to_von().unwrap());
}
