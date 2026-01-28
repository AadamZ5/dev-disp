use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    tonic_prost_build::configure()
        .file_descriptor_set_path(out_dir.join("dev_disp_service_descriptor.bin"))
        .compile_protos(&["proto/service.proto"], &["proto"])?;
    Ok(())
}
