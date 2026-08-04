//! Regenerates `assets/gallery/<stem>.md`, the per-sample rendered gallery
//! fixtures, plus `assets/gallery/README.md`, the browsable index grouping
//! every sample by diagram kind. Only the 23 primary (flat-stem) fixtures are
//! embedded into the crate root documentation by `src/lib.rs`; secondary
//! fixtures (nested-stem, e.g. `flowchart/02-nested-subgraphs`) exist only as
//! render-fixtures and index entries.
//!
//! Run after changing `samples/` or the renderers, and commit the result:
//!
//! ```sh
//! cargo run --example gen-doc-diagrams
//! ```
//!
//! Only files whose content actually changed are rewritten, and each rewrite is
//! printed — so `git status` afterwards shows exactly which diagrams a change
//! affected. The `doc_gallery_up_to_date` and `gallery_index_up_to_date`
//! integration tests fail if any committed file is out of sync.

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/gallery_build.rs"));

fn main() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/gallery");
    std::fs::create_dir_all(dir).expect("create assets/gallery dir");
    for (title, stem, src) in SAMPLES {
        let path = format!("{dir}/{stem}.md");
        if let Some(parent) = std::path::Path::new(&path).parent() {
            std::fs::create_dir_all(parent)
                .unwrap_or_else(|e| panic!("create dir for {path}: {e}"));
        }
        let next = gallery_section(title, src);
        if std::fs::read_to_string(&path).ok().as_deref() == Some(next.as_str()) {
            continue;
        }
        std::fs::write(&path, &next).unwrap_or_else(|e| panic!("write {path}: {e}"));
        println!("wrote {path}");
    }

    let index_path = format!("{dir}/README.md");
    let index = gallery_index();
    if std::fs::read_to_string(&index_path).ok().as_deref() != Some(index.as_str()) {
        std::fs::write(&index_path, &index).unwrap_or_else(|e| panic!("write {index_path}: {e}"));
        println!("wrote {index_path}");
    }
}
