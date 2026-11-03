//! Emit a Python generated client for one VOS schema file.
//!
//! This is the dev-side generation entry used by the install-state Python
//! fixture, where Node tooling is not on the job's dependency list:
//!
//! ```text
//! cargo run -p iris-generator --bin iris-python-generate -- <schema> <out-dir>
//! ```

use std::path::PathBuf;

use iris_generator::write_python_client;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(schema_path), Some(out_dir)) = (args.next(), args.next())
    else {
        eprintln!("usage: iris-python-generate <schema.iris> <out-dir>");
        std::process::exit(64);
    };
    if args.next().is_some() {
        eprintln!("usage: iris-python-generate <schema.iris> <out-dir>");
        std::process::exit(64);
    }

    let source = std::fs::read_to_string(&schema_path)?;
    let model = iris_generator::GenerationModel::from_vos_schema(&source)?;
    let written = write_python_client(&model, &PathBuf::from(&out_dir))?;
    println!(
        "generated python client ({count} files, fingerprint={fingerprint}) -> {out_dir}",
        count = written.len(),
        fingerprint = model.schema_fingerprint
    );
    Ok(())
}
