//! Recognize cargo-fuzz's conditional direct inclusion of the shared media parser.
fn main() {
    println!("cargo::rustc-check-cfg=cfg(fuzzing)");
}
