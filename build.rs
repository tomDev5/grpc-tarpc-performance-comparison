fn main() {
    tonic_build::configure()
        .out_dir("src")
        .compile_protos(&["helloworld.proto"], &["."])
        .unwrap();
}
