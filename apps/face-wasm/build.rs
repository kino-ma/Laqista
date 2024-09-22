use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rustc-link-arg=--import-memory");
    tonic_build::compile_protos("../../proto/face.proto")?;
    Ok(())
}
