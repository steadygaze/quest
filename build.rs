use static_files::resource_dir;
use std::{env, path::Path, process::Command};

fn main() -> Result<(), env::VarError> {
    println!("cargo::rerun-if-changed=input.css");
    println!("cargo::rerun-if-changed=tailwind.config.js");
    println!("cargo::rerun-if-changed=templates");

    let ci = match std::env::var("CI") {
        Ok(string) => string
            .parse::<bool>()
            .expect("expecting env var CI to be \"true\" or \"false\""),
        Err(_) => false,
    };
    if ci
        || !match Path::new("./node_modules").try_exists() {
            Ok(b) => b,
            Err(_) => false, // Couldn't determine, so run.
        }
    {
        println!("cargo::warning=running npm install");
        let mut command = Command::new("npm");
        command.arg("install");
        let output = command.output().expect("failed to run npm install");
        assert!(
            output.status.success(),
            "failed to run `npm install` (status nonzero)"
        );
    }

    let tailwind_path = Path::new("./node_modules/.bin/tailwindcss");
    // The tailwindcss binary creates parents directories, so no worries there.
    let output_path = Path::new("./static/css/tailwind.css");
    let debug: bool = env::var("DEBUG")?
        .parse()
        .expect("expecting env var DEBUG to be \"true\" or \"false\"");
    println!("cargo::warning=rerunning tailwind build (debug={})", debug);

    let mut command = Command::new(
        tailwind_path
            .to_str()
            .expect("failed to generate tailwind binary path"),
    );
    if !debug {
        command.arg("--minify");
    }
    command.args([
        "-i",
        "input.css",
        "-o",
        output_path
            .to_str()
            .expect("failed to generate output path"),
    ]);
    let output = command.output().expect(
        "failed to run tailwind; try rerunning with `npm run build` to see the exact error",
    );
    assert!(
        output.status.success(),
        "failed to run tailwindcss (status nonzero); try rerunning with `npm run build` to see the exact error"
    );

    resource_dir("./static")
        .build()
        .expect("failed to collect static resources");

    Ok(())
}
