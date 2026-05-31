//! Regenerates `assets/gallery.md`, the rendered diagram gallery embedded into
//! the crate root documentation by `src/lib.rs`.
//!
//! Run after changing `samples/` or the renderers, and commit the result:
//!
//! ```sh
//! cargo run --example gen-doc-diagrams
//! ```
//!
//! The `doc_gallery_up_to_date` integration test fails if the committed file is
//! out of sync.

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/gallery_build.rs"));

fn main() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
    std::fs::create_dir_all(dir).expect("create assets dir");
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/gallery.md");
    std::fs::write(path, build_gallery()).expect("write gallery.md");
    println!("wrote {path}");
}
