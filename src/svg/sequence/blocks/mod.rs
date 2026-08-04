//! Layout pass (event collection) and block-frame drawing.

mod frames;
mod layout;

#[cfg(test)]
mod tests;

pub(in crate::svg::sequence) use frames::{
    block_label_right_overhang, draw_block_frames, draw_rect_bands,
};
pub(in crate::svg::sequence) use layout::layout_items;
