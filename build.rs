use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/mless.proto")?;

    println!("cargo::rerun-if-changed=apps/face-wasm/src/lib.rs");

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("./apps/face-wasm");
    let path_str = path.display();
    let pkg = format!("file://{path_str}");

    let output = std::process::Command::new("cargo")
        .args(["build", "--package", &pkg, "--release"])
        .output()?;

    if !output.status.success() {
        let out = String::from_utf8(output.stdout)?;
        let err = String::from_utf8(output.stderr)?;

        let text = format!("Wasm build failed.\nStdout:\n{out}\n\nStderr:\n{err}");

        return Err(text.into());
    } else {
        let out = String::from_utf8(output.stdout)?;
        let err = String::from_utf8(output.stderr)?;

        let text = format!("Wasm build succeeded.\nStdout:\n{out}\n\nStderr:\n{err}");
        eprintln!("{text}");
    }

    Ok(())
}
