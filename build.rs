#[cfg(feature = "rebuild-protobuf")]
extern crate protobuf_codegen;

#[cfg(feature = "rebuild-protobuf")]
fn main() {
	protobuf_codegen::Codegen::new()
		.out_dir("src")
		.inputs(&["proto/Backups.proto"])
		.include("proto")
		.run()
		.expect("Running protoc failed.");
}

#[cfg(not(feature = "rebuild-protobuf"))]
fn main() {}
