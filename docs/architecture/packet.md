# Packet — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/packet.rs` · Renderer: `src/svg/packet.rs`.

- Packet fields hard-error on a backwards range (`end < start`, `src/parse/packet.rs`)
  — like upstream's "End must be greater than start" — since it would rewind the
  relative `+N` cursor and overlap earlier fields; `start == end` (single bit via
  a dash) stays valid, and non-contiguous gaps remain a tolerated no-op.
  `config.packet.*` (`bitsPerRow`/`bitWidth`/`rowHeight`/`showBits`/`paddingX`/
  `paddingY`) flows through the preamble → `apply_packet_config` (`src/parse/mod.rs`)
  onto `PacketDiagram.config` (`PacketConfig`, defaults tracking upstream's 32px
  per bit so one-bit flag labels fit their cells); `src/svg/packet.rs` reads
  them (`showBits false` drops the bit numbers, `bitsPerRow` re-wraps rows).
- Bit offsets are printed per field block, not as a single shared ruler above
  the first row: every block carries its own absolute start bit (top-left) and
  end bit (top-right), a single number for a 1-bit block (issue #247). A block
  that wraps a row boundary is split, so each row's segment shows the absolute
  bits it actually covers (e.g. `32`/`63` on the second word).
- Background cells are drawn only for bits no field covers — a field is one
  undivided rectangle, not a grid of per-bit cells (issue #248), matching
  upstream's flat field rectangles.
