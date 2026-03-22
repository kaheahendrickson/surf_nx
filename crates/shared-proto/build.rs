fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_files = &["../../proto/message.proto"];
    let out_dir = std::path::Path::new("src/generated");

    std::fs::create_dir_all(out_dir)?;

    let mut config = prost_build::Config::new();
    config.out_dir(out_dir);

    config.compile_protos(proto_files, &["../../proto"])?;

    Ok(())
}
