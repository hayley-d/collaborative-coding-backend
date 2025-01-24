use std::error::Error;

use tonic_build::Config;

fn main() -> Result<(), Box<dyn Error>> {
    let mut config = Config::new();
    config.out_dir("src/proto");

    let file_path = "proto/rate_limiter.proto";

    tonic_build::configure()
        .out_dir("src/proto")
        .compile_protos(&[file_path], &["proto"])
        .expect("Failed to complie");
    Ok(())
}
