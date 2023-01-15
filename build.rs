use std::path::PathBuf;

fn main() {
    let dir: PathBuf = ["tree-sitter-markdown", "src"].iter().collect();

    cc::Build::new()
        .include(&dir)
        .file(dir.join("parser.c"))
        .file(dir.join("scanner.cc"))
        .warnings(false)
        .compile("tree-sitter-markdown");

    println!("cargo:rustc-link-lib=stdc++");
}
