fn main() -> Result<(), Box<dyn std::error::Error>> {
    let builder = tonic_build::configure();
    let builder = builder.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
    builder.compile(&["../../proto/face.proto"], &["../../proto"])?;
    Ok(())
}
