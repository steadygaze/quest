use std::{env, path::Path, process::Command};

#[allow(dead_code)]
fn run_npm_install() {
    let output = Command::new("npm")
        .arg("install")
        .output()
        .expect("failed to run `npm install`");
    if !output.status.success() {
        println!(
            "cargo::warning=`npm install` status {} is nonzero; try running tailwind manually",
            output.status
        );
    }
}

fn main() -> Result<(), env::VarError> {
    println!("cargo::rerun-if-changed=tailwind.config.js");
    println!("cargo::rerun-if-changed=templates");

    let out_dir = env::var("OUT_DIR")?;
    let tailwind_path = Path::new("node_modules").join(".bin").join("tailwindcss");
    let output_path = Path::new(&out_dir).join("tailwind.css");
    let debug: bool = env::var("DEBUG")?
        .parse()
        .expect("env var DEBUG has invalid value (expecting \"true\" or \"false\")");
    println!("cargo::warning=rerunning tailwind build (debug={})", debug);

    let mut command = Command::new(
        tailwind_path
            .to_str()
            .expect("failed to generate tailwind binary path"),
    );
    command.args([
        "-i",
        "input.css",
        "-o",
        output_path
            .to_str()
            .expect("failed to generate output path"),
    ]);
    if !debug {
        command.arg("--minify");
    }
    let output = command.output().expect("failed to execute tailwind");
    if !output.status.success() {
        println!(
            "cargo::warning=tailwind status {} is nonzero; try running tailwind manually",
            output.status
        );
    }

    Ok(())
}
