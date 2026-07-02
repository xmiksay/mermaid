//! Regenerates `assets/gallery/<stem>.md`, the per-diagram rendered gallery
//! sections embedded into the crate root documentation by `src/lib.rs`.
//!
//! Run after changing `samples/` or the renderers, and commit the result:
//!
//! ```sh
//! cargo run --example gen-doc-diagrams
//! ```
//!
//! Only files whose content actually changed are rewritten, and each rewrite is
//! printed — so `git status` afterwards shows exactly which diagrams a change
//! affected. The `doc_gallery_up_to_date` integration test fails if any
//! committed file is out of sync.

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/gallery_build.rs"));

fn main() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/gallery");
    std::fs::create_dir_all(dir).expect("create assets/gallery dir");
    for (title, stem, src) in SAMPLES {
        let path = format!("{dir}/{stem}.md");
        let next = gallery_section(title, src);
        if std::fs::read_to_string(&path).ok().as_deref() == Some(next.as_str()) {
            continue;
        }
        std::fs::write(&path, &next).unwrap_or_else(|e| panic!("write {path}: {e}"));
        println!("wrote {path}");
    }
}
