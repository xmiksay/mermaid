//! End-to-end benchmarks: parse + render, one sample per diagram type.
//!
//! Run with `cargo bench`. Each diagram type gets two cases:
//! - `parse/<kind>`  — parse only
//! - `render/<kind>` — parse + render to SVG string

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use mermaid_svg::{parse, render};

/// (name, source) pairs for every supported diagram type.
fn samples() -> Vec<(&'static str, &'static str)> {
    vec![
        ("pie", include_str!("samples/pie.mmd")),
        ("sequence", include_str!("samples/sequence.mmd")),
        ("flowchart", include_str!("samples/flowchart.mmd")),
        ("state", include_str!("samples/state.mmd")),
        ("class", include_str!("samples/class.mmd")),
        ("er", include_str!("samples/er.mmd")),
        ("gantt", include_str!("samples/gantt.mmd")),
        ("journey", include_str!("samples/journey.mmd")),
        ("timeline", include_str!("samples/timeline.mmd")),
        ("sankey", include_str!("samples/sankey.mmd")),
        ("quadrant", include_str!("samples/quadrant.mmd")),
        ("xychart", include_str!("samples/xychart.mmd")),
        ("radar", include_str!("samples/radar.mmd")),
        ("packet", include_str!("samples/packet.mmd")),
        ("mindmap", include_str!("samples/mindmap.mmd")),
        ("gitgraph", include_str!("samples/gitgraph.mmd")),
        ("requirement", include_str!("samples/requirement.mmd")),
        ("c4", include_str!("samples/c4.mmd")),
        ("block", include_str!("samples/block.mmd")),
        ("architecture", include_str!("samples/architecture.mmd")),
        ("kanban", include_str!("samples/kanban.mmd")),
        ("treemap", include_str!("samples/treemap.mmd")),
        ("zenuml", include_str!("samples/zenuml.mmd")),
    ]
}

fn bench_parse(c: &mut Criterion) {
    let mut g = c.benchmark_group("parse");
    for (name, src) in samples() {
        g.throughput(Throughput::Bytes(src.len() as u64));
        g.bench_with_input(BenchmarkId::from_parameter(name), src, |b, s| {
            b.iter(|| parse(black_box(s)).unwrap());
        });
    }
    g.finish();
}

fn bench_render(c: &mut Criterion) {
    let mut g = c.benchmark_group("render");
    for (name, src) in samples() {
        g.throughput(Throughput::Bytes(src.len() as u64));
        g.bench_with_input(BenchmarkId::from_parameter(name), src, |b, s| {
            b.iter(|| render(black_box(s)).unwrap());
        });
    }
    g.finish();
}

criterion_group!(benches, bench_parse, bench_render);
criterion_main!(benches);
