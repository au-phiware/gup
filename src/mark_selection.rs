// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Interactive mark selection system for GPU-accelerated visualizations.
//!
//! This module provides an efficient selection state management system for
//! large datasets, with bitset-based storage, undo/redo support, configurable
//! visual styles, and integration with the existing interaction system.
//!
//! # Architecture
//!
//! The selection system is built from three main components:
//!
//! - [`SelectionState`] — Bitset-backed selection tracking with undo/redo
//! - [`SelectionStyle`] — Visual feedback configuration for selected/hovered marks
//! - [`SelectionTool`] — Input-driven selection tools (point, rectangle, lasso)
//!
//! # Examples
//!
//! ```rust
//! use gup::mark_selection::{SelectionState, SelectionMode, SelectionStyle};
//!
//! // Create selection state for 100K marks
//! let mut state = SelectionState::new(100_000);
//!
//! // Select individual marks
//! state.select(42);
//! state.select(99);
//! assert_eq!(state.count(), 2);
//!
//! // Toggle selection
//! state.toggle(42);
//! assert!(!state.is_selected(42));
//!
//! // Undo the toggle
//! state.undo();
//! assert!(state.is_selected(42));
//!
//! // Rectangle selection (select a range)
//! state.select_range(10..20);
//! assert_eq!(state.count(), 12); // 42, 99, 10..20
//! ```

use crate::error::GupResult;
use crate::interaction::{ElementData, InteractionSystem, Rect, Vec2};
use serde::{Deserialize, Serialize};
use std::ops::Range;

// ---------------------------------------------------------------------------
// Bitset — compact selection storage
// ---------------------------------------------------------------------------

/// A compact bitset for tracking selection state of up to millions of marks.
///
/// Uses 1 bit per mark, so 1M marks requires only ~122 KB of memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BitSet {
    /// Underlying storage: each `u64` holds 64 bits.
    blocks: Vec<u64>,
    /// Total number of bits (marks) this bitset tracks.
    len: usize,
}

impl BitSet {
    /// Create a new bitset with all bits cleared.
    pub fn new(len: usize) -> Self {
        let block_count = len.div_ceil(64);
        Self {
            blocks: vec![0u64; block_count],
            len,
        }
    }

    /// Returns the number of bits (marks) in this bitset.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the bitset is empty (zero capacity).
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Set the bit at `index` (mark it as selected).
    pub fn set(&mut self, index: usize) {
        if index < self.len {
            self.blocks[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear the bit at `index` (mark it as unselected).
    pub fn clear_bit(&mut self, index: usize) {
        if index < self.len {
            self.blocks[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Toggle the bit at `index`.
    pub fn toggle(&mut self, index: usize) {
        if index < self.len {
            self.blocks[index / 64] ^= 1u64 << (index % 64);
        }
    }

    /// Returns `true` if the bit at `index` is set.
    pub fn get(&self, index: usize) -> bool {
        if index < self.len {
            (self.blocks[index / 64] >> (index % 64)) & 1 == 1
        } else {
            false
        }
    }

    /// Clear all bits.
    pub fn clear_all(&mut self) {
        for block in &mut self.blocks {
            *block = 0;
        }
    }

    /// Set all bits.
    pub fn set_all(&mut self) {
        for block in &mut self.blocks {
            *block = u64::MAX;
        }
        // Clear trailing bits beyond len
        let remainder = self.len % 64;
        if remainder > 0
            && let Some(last) = self.blocks.last_mut()
        {
            *last &= (1u64 << remainder) - 1;
        }
    }

    /// Count the number of set bits (popcount).
    pub fn count_ones(&self) -> usize {
        self.blocks.iter().map(|b| b.count_ones() as usize).sum()
    }

    /// Iterate over the indices of all set bits.
    pub fn ones(&self) -> BitSetOnes<'_> {
        BitSetOnes {
            bitset: self,
            block_idx: 0,
            current_block: self.blocks.first().copied().unwrap_or(0),
        }
    }

    /// Resize the bitset to `new_len`. New bits are initialised to zero.
    pub fn resize(&mut self, new_len: usize) {
        let new_blocks = new_len.div_ceil(64);
        self.blocks.resize(new_blocks, 0);
        // Clear any trailing bits in the old last block if shrinking
        if new_len < self.len {
            let remainder = new_len % 64;
            if remainder > 0
                && let Some(last) = self.blocks.last_mut()
            {
                *last &= (1u64 << remainder) - 1;
            }
        }
        self.len = new_len;
    }

    /// Compute the intersection of two bitsets (AND).
    pub fn intersect(&self, other: &BitSet) -> BitSet {
        let len = self.len.min(other.len);
        let block_count = len.div_ceil(64);
        let blocks: Vec<u64> = self.blocks[..block_count]
            .iter()
            .zip(&other.blocks[..block_count])
            .map(|(&a, &b)| a & b)
            .collect();
        BitSet { blocks, len }
    }

    /// Compute the union of two bitsets (OR).
    pub fn union(&self, other: &BitSet) -> BitSet {
        let len = self.len.max(other.len);
        let block_count = len.div_ceil(64);
        let mut blocks = vec![0u64; block_count];
        for (i, b) in blocks.iter_mut().enumerate() {
            let a_val = self.blocks.get(i).copied().unwrap_or(0);
            let b_val = other.blocks.get(i).copied().unwrap_or(0);
            *b = a_val | b_val;
        }
        BitSet { blocks, len }
    }

    /// Memory usage in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.blocks.len() * 8
    }
}

/// Iterator over set bit indices in a [`BitSet`].
pub struct BitSetOnes<'a> {
    bitset: &'a BitSet,
    block_idx: usize,
    current_block: u64,
}

impl Iterator for BitSetOnes<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        loop {
            if self.current_block != 0 {
                let bit = self.current_block.trailing_zeros() as usize;
                self.current_block &= self.current_block - 1; // clear lowest set bit
                let index = self.block_idx * 64 + bit;
                if index < self.bitset.len {
                    return Some(index);
                } else {
                    return None;
                }
            }
            self.block_idx += 1;
            if self.block_idx >= self.bitset.blocks.len() {
                return None;
            }
            self.current_block = self.bitset.blocks[self.block_idx];
        }
    }
}

// ---------------------------------------------------------------------------
// Selection operations (for undo/redo)
// ---------------------------------------------------------------------------

/// A recorded selection operation that can be undone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SelectionOperation {
    /// Selected a set of mark IDs.
    Select(Vec<u32>),
    /// Deselected a set of mark IDs.
    Deselect(Vec<u32>),
    /// Toggled a set of mark IDs.
    Toggle(Vec<u32>),
    /// Cleared all selections. Stores the previous selection for undo.
    Clear(BitSet),
    /// Set all marks selected. Stores previous selection for undo.
    SelectAll(BitSet),
    /// Rectangle selection. Stores affected IDs and previous selection state.
    RectangleSelect {
        ids: Vec<u32>,
        additive: bool,
        previous: BitSet,
    },
    /// Lasso selection. Stores affected IDs and previous selection state.
    LassoSelect {
        ids: Vec<u32>,
        additive: bool,
        previous: BitSet,
    },
}

// ---------------------------------------------------------------------------
// Selection mode
// ---------------------------------------------------------------------------

/// Selection behaviour mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SelectionMode {
    /// Clicking selects one mark, deselecting all others.
    #[default]
    Single,
    /// Clicking toggles the mark without affecting others (like Ctrl+Click).
    Toggle,
    /// Clicking adds to the current selection (like Shift+Click).
    Additive,
    /// Clicking removes from the current selection.
    Subtractive,
}

// ---------------------------------------------------------------------------
// Selection state
// ---------------------------------------------------------------------------

/// Manages the selection state for a set of marks with undo/redo support.
///
/// Uses a compact bitset internally so that even 1M+ marks use minimal memory
/// (~122 KB for the bitset plus undo history).
#[derive(Debug, Clone)]
pub struct SelectionState {
    /// Current selection as a bitset.
    selected: BitSet,
    /// Currently hovered mark (at most one).
    hover: Option<u32>,
    /// Current selection mode.
    mode: SelectionMode,
    /// Undo stack of operations.
    undo_stack: Vec<SelectionOperation>,
    /// Redo stack of operations.
    redo_stack: Vec<SelectionOperation>,
    /// Maximum undo history size.
    max_undo_history: usize,
}

impl SelectionState {
    /// Create a new selection state for `mark_count` marks.
    pub fn new(mark_count: usize) -> Self {
        Self {
            selected: BitSet::new(mark_count),
            hover: None,
            mode: SelectionMode::default(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_undo_history: 100,
        }
    }

    /// Create a new selection state with a specific mode.
    pub fn with_mode(mark_count: usize, mode: SelectionMode) -> Self {
        let mut state = Self::new(mark_count);
        state.mode = mode;
        state
    }

    /// Returns the current selection mode.
    pub fn mode(&self) -> SelectionMode {
        self.mode
    }

    /// Set the selection mode.
    pub fn set_mode(&mut self, mode: SelectionMode) {
        self.mode = mode;
    }

    /// Returns `true` if mark `id` is selected.
    pub fn is_selected(&self, id: u32) -> bool {
        self.selected.get(id as usize)
    }

    /// Returns the currently hovered mark ID, if any.
    pub fn hover(&self) -> Option<u32> {
        self.hover
    }

    /// Set the currently hovered mark.
    pub fn set_hover(&mut self, id: Option<u32>) {
        self.hover = id;
    }

    /// Returns the number of selected marks.
    pub fn count(&self) -> usize {
        self.selected.count_ones()
    }

    /// Returns the total number of marks tracked.
    pub fn mark_count(&self) -> usize {
        self.selected.len()
    }

    /// Returns `true` if no marks are selected.
    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    /// Select a single mark.
    pub fn select(&mut self, id: u32) {
        if !self.selected.get(id as usize) {
            self.push_undo(SelectionOperation::Select(vec![id]));
            self.selected.set(id as usize);
        }
    }

    /// Deselect a single mark.
    pub fn deselect(&mut self, id: u32) {
        if self.selected.get(id as usize) {
            self.push_undo(SelectionOperation::Deselect(vec![id]));
            self.selected.clear_bit(id as usize);
        }
    }

    /// Toggle the selection of a single mark.
    pub fn toggle(&mut self, id: u32) {
        self.push_undo(SelectionOperation::Toggle(vec![id]));
        self.selected.toggle(id as usize);
    }

    /// Select multiple marks at once.
    pub fn select_many(&mut self, ids: &[u32]) {
        let new_ids: Vec<u32> = ids
            .iter()
            .copied()
            .filter(|&id| !self.selected.get(id as usize))
            .collect();
        if !new_ids.is_empty() {
            self.push_undo(SelectionOperation::Select(new_ids.clone()));
            for id in &new_ids {
                self.selected.set(*id as usize);
            }
        }
    }

    /// Deselect multiple marks at once.
    pub fn deselect_many(&mut self, ids: &[u32]) {
        let old_ids: Vec<u32> = ids
            .iter()
            .copied()
            .filter(|&id| self.selected.get(id as usize))
            .collect();
        if !old_ids.is_empty() {
            self.push_undo(SelectionOperation::Deselect(old_ids.clone()));
            for id in &old_ids {
                self.selected.clear_bit(*id as usize);
            }
        }
    }

    /// Select a range of marks.
    pub fn select_range(&mut self, range: Range<u32>) {
        let ids: Vec<u32> = range.collect();
        self.select_many(&ids);
    }

    /// Clear all selections.
    pub fn clear(&mut self) {
        if self.count() > 0 {
            let previous = self.selected.clone();
            self.push_undo(SelectionOperation::Clear(previous));
            self.selected.clear_all();
        }
    }

    /// Select all marks.
    pub fn select_all(&mut self) {
        let previous = self.selected.clone();
        self.push_undo(SelectionOperation::SelectAll(previous));
        self.selected.set_all();
    }

    /// Apply a click at a specific mark ID, respecting the current selection mode.
    pub fn click(&mut self, id: u32) {
        match self.mode {
            SelectionMode::Single => {
                let previous = self.selected.clone();
                self.push_undo(SelectionOperation::RectangleSelect {
                    ids: vec![id],
                    additive: false,
                    previous,
                });
                self.selected.clear_all();
                self.selected.set(id as usize);
            }
            SelectionMode::Toggle => {
                self.toggle(id);
            }
            SelectionMode::Additive => {
                self.select(id);
            }
            SelectionMode::Subtractive => {
                self.deselect(id);
            }
        }
    }

    /// Apply a rectangle selection to a set of mark IDs found within the rect.
    pub fn rect_select(&mut self, ids: &[u32], additive: bool) {
        let previous = self.selected.clone();
        self.push_undo(SelectionOperation::RectangleSelect {
            ids: ids.to_vec(),
            additive,
            previous,
        });
        if !additive {
            self.selected.clear_all();
        }
        for &id in ids {
            self.selected.set(id as usize);
        }
    }

    /// Apply a lasso selection to a set of mark IDs found within the lasso path.
    pub fn lasso_select(&mut self, ids: &[u32], additive: bool) {
        let previous = self.selected.clone();
        self.push_undo(SelectionOperation::LassoSelect {
            ids: ids.to_vec(),
            additive,
            previous,
        });
        if !additive {
            self.selected.clear_all();
        }
        for &id in ids {
            self.selected.set(id as usize);
        }
    }

    /// Undo the last selection operation. Returns `true` if an operation was undone.
    pub fn undo(&mut self) -> bool {
        if let Some(op) = self.undo_stack.pop() {
            self.apply_undo(&op);
            self.redo_stack.push(op);
            true
        } else {
            false
        }
    }

    /// Redo the last undone operation. Returns `true` if an operation was redone.
    pub fn redo(&mut self) -> bool {
        if let Some(op) = self.redo_stack.pop() {
            self.apply_redo(&op);
            self.undo_stack.push(op);
            true
        } else {
            false
        }
    }

    /// Returns `true` if there are operations to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns `true` if there are operations to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Iterate over the IDs of all selected marks.
    pub fn selected_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.selected.ones().map(|i| i as u32)
    }

    /// Get a reference to the underlying bitset.
    pub fn bitset(&self) -> &BitSet {
        &self.selected
    }

    /// Resize the selection state to accommodate a new mark count.
    ///
    /// This preserves existing selections for marks that still exist.
    pub fn resize(&mut self, new_mark_count: usize) {
        self.selected.resize(new_mark_count);
    }

    /// Get selection statistics.
    pub fn statistics(&self) -> SelectionStatistics {
        SelectionStatistics {
            total_marks: self.selected.len(),
            selected_count: self.count(),
            hover_id: self.hover,
            mode: self.mode,
            undo_depth: self.undo_stack.len(),
            redo_depth: self.redo_stack.len(),
            memory_bytes: self.selected.memory_bytes()
                + self.undo_stack.len() * std::mem::size_of::<SelectionOperation>(),
        }
    }

    /// Serialize the selection state to bytes.
    pub fn serialize(&self) -> Vec<u32> {
        self.selected_ids().collect()
    }

    /// Restore selection state from a list of selected IDs.
    pub fn deserialize(&mut self, ids: &[u32]) {
        self.selected.clear_all();
        for &id in ids {
            self.selected.set(id as usize);
        }
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    // -- private helpers --

    fn push_undo(&mut self, op: SelectionOperation) {
        self.redo_stack.clear();
        self.undo_stack.push(op);
        if self.undo_stack.len() > self.max_undo_history {
            self.undo_stack.remove(0);
        }
    }

    fn apply_undo(&mut self, op: &SelectionOperation) {
        match op {
            SelectionOperation::Select(ids) => {
                for &id in ids {
                    self.selected.clear_bit(id as usize);
                }
            }
            SelectionOperation::Deselect(ids) => {
                for &id in ids {
                    self.selected.set(id as usize);
                }
            }
            SelectionOperation::Toggle(ids) => {
                for &id in ids {
                    self.selected.toggle(id as usize);
                }
            }
            SelectionOperation::Clear(previous) | SelectionOperation::SelectAll(previous) => {
                self.selected = previous.clone();
            }
            SelectionOperation::RectangleSelect { previous, .. }
            | SelectionOperation::LassoSelect { previous, .. } => {
                self.selected = previous.clone();
            }
        }
    }

    fn apply_redo(&mut self, op: &SelectionOperation) {
        match op {
            SelectionOperation::Select(ids) => {
                for &id in ids {
                    self.selected.set(id as usize);
                }
            }
            SelectionOperation::Deselect(ids) => {
                for &id in ids {
                    self.selected.clear_bit(id as usize);
                }
            }
            SelectionOperation::Toggle(ids) => {
                for &id in ids {
                    self.selected.toggle(id as usize);
                }
            }
            SelectionOperation::Clear(_) => {
                self.selected.clear_all();
            }
            SelectionOperation::SelectAll(_) => {
                self.selected.set_all();
            }
            SelectionOperation::RectangleSelect { ids, additive, .. }
            | SelectionOperation::LassoSelect { ids, additive, .. } => {
                if !additive {
                    self.selected.clear_all();
                }
                for &id in ids {
                    self.selected.set(id as usize);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Selection statistics
// ---------------------------------------------------------------------------

/// Summary statistics about the current selection state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionStatistics {
    /// Total marks tracked.
    pub total_marks: usize,
    /// Number of currently selected marks.
    pub selected_count: usize,
    /// Currently hovered mark ID.
    pub hover_id: Option<u32>,
    /// Current selection mode.
    pub mode: SelectionMode,
    /// Number of undo operations available.
    pub undo_depth: usize,
    /// Number of redo operations available.
    pub redo_depth: usize,
    /// Approximate memory usage in bytes.
    pub memory_bytes: usize,
}

// ---------------------------------------------------------------------------
// Selection style
// ---------------------------------------------------------------------------

/// Visual style configuration for selection feedback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionStyle {
    /// Colour multiplier applied to selected marks.
    pub selected_color: [f32; 4],
    /// Outline colour for selected marks.
    pub selected_outline_color: [f32; 4],
    /// Outline width for selected marks.
    pub selected_outline_width: f32,
    /// Scale factor for selected marks (1.0 = no change).
    pub selected_scale: f32,

    /// Colour multiplier applied to hovered marks.
    pub hover_color: [f32; 4],
    /// Outline colour for hovered marks.
    pub hover_outline_color: [f32; 4],
    /// Outline width for hovered marks.
    pub hover_outline_width: f32,
    /// Scale factor for hovered marks.
    pub hover_scale: f32,

    /// Opacity for non-selected marks when any marks are selected (dimming).
    pub unselected_opacity: f32,
}

impl Default for SelectionStyle {
    fn default() -> Self {
        Self {
            selected_color: [1.0, 1.0, 1.0, 1.0],
            selected_outline_color: [0.2, 0.5, 1.0, 1.0],
            selected_outline_width: 2.0,
            selected_scale: 1.0,
            hover_color: [1.0, 1.0, 1.0, 1.0],
            hover_outline_color: [1.0, 0.8, 0.0, 1.0],
            hover_outline_width: 1.5,
            hover_scale: 1.1,
            unselected_opacity: 0.3,
        }
    }
}

impl SelectionStyle {
    /// A highlight-only style that brightens selected marks without outlines.
    pub fn highlight() -> Self {
        Self {
            selected_color: [1.2, 1.2, 1.2, 1.0],
            selected_outline_color: [0.0, 0.0, 0.0, 0.0],
            selected_outline_width: 0.0,
            selected_scale: 1.05,
            hover_color: [1.1, 1.1, 1.1, 1.0],
            hover_outline_color: [0.0, 0.0, 0.0, 0.0],
            hover_outline_width: 0.0,
            hover_scale: 1.1,
            unselected_opacity: 0.4,
        }
    }

    /// A bold outline style for clearly marking selection.
    pub fn outline() -> Self {
        Self {
            selected_color: [1.0, 1.0, 1.0, 1.0],
            selected_outline_color: [0.0, 0.4, 1.0, 1.0],
            selected_outline_width: 3.0,
            selected_scale: 1.0,
            hover_color: [1.0, 1.0, 1.0, 1.0],
            hover_outline_color: [1.0, 0.6, 0.0, 1.0],
            hover_outline_width: 2.0,
            hover_scale: 1.0,
            unselected_opacity: 0.25,
        }
    }
}

// ---------------------------------------------------------------------------
// Selection tool
// ---------------------------------------------------------------------------

/// The type of selection tool currently active.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum SelectionToolKind {
    /// Single-click point selection.
    #[default]
    Point,
    /// Drag rectangle selection.
    Rectangle,
    /// Free-form lasso path selection.
    Lasso,
}

/// Input state for an active selection tool.
#[derive(Debug, Clone)]
pub enum ToolState {
    /// Tool is idle, waiting for user input.
    Idle,
    /// Rectangle drag in progress.
    DraggingRect {
        /// Starting point (anchor) of the rectangle.
        start: Vec2,
        /// Current drag endpoint.
        current: Vec2,
    },
    /// Lasso path in progress.
    DrawingLasso {
        /// Accumulated path points.
        points: Vec<Vec2>,
    },
}

/// Interactive selection tool that processes input events.
#[derive(Debug, Clone)]
pub struct SelectionTool {
    /// Which tool is active.
    pub kind: SelectionToolKind,
    /// Current input state.
    pub state: ToolState,
}

impl SelectionTool {
    /// Create a new selection tool of the given kind.
    pub fn new(kind: SelectionToolKind) -> Self {
        Self {
            kind,
            state: ToolState::Idle,
        }
    }

    /// Create a point selection tool (default).
    pub fn point() -> Self {
        Self::new(SelectionToolKind::Point)
    }

    /// Create a rectangle selection tool.
    pub fn rectangle() -> Self {
        Self::new(SelectionToolKind::Rectangle)
    }

    /// Create a lasso selection tool.
    pub fn lasso() -> Self {
        Self::new(SelectionToolKind::Lasso)
    }

    /// Begin a drag/draw operation at the given position.
    pub fn begin(&mut self, position: Vec2) {
        self.state = match self.kind {
            SelectionToolKind::Point => ToolState::Idle,
            SelectionToolKind::Rectangle => ToolState::DraggingRect {
                start: position,
                current: position,
            },
            SelectionToolKind::Lasso => ToolState::DrawingLasso {
                points: vec![position],
            },
        };
    }

    /// Update the drag/draw position.
    pub fn update(&mut self, position: Vec2) {
        match &mut self.state {
            ToolState::DraggingRect { current, .. } => {
                *current = position;
            }
            ToolState::DrawingLasso { points } => {
                points.push(position);
            }
            ToolState::Idle => {}
        }
    }

    /// Finish the drag/draw operation and return the resulting geometry.
    pub fn finish(&mut self) -> ToolResult {
        let result = match &self.state {
            ToolState::Idle => ToolResult::None,
            ToolState::DraggingRect { start, current } => {
                let rect = Rect::new(
                    Vec2::new(start.x.min(current.x), start.y.min(current.y)),
                    Vec2::new(start.x.max(current.x), start.y.max(current.y)),
                );
                ToolResult::Rectangle(rect)
            }
            ToolState::DrawingLasso { points } => {
                if points.len() < 3 {
                    ToolResult::None
                } else {
                    ToolResult::Lasso(points.clone())
                }
            }
        };
        self.state = ToolState::Idle;
        result
    }

    /// Cancel the current operation.
    pub fn cancel(&mut self) {
        self.state = ToolState::Idle;
    }

    /// Returns `true` if the tool is actively processing input.
    pub fn is_active(&self) -> bool {
        !matches!(self.state, ToolState::Idle)
    }

    /// Get the current rectangle being dragged, if any.
    pub fn current_rect(&self) -> Option<Rect> {
        match &self.state {
            ToolState::DraggingRect { start, current } => Some(Rect::new(
                Vec2::new(start.x.min(current.x), start.y.min(current.y)),
                Vec2::new(start.x.max(current.x), start.y.max(current.y)),
            )),
            _ => None,
        }
    }

    /// Get the current lasso path points, if any.
    pub fn current_lasso_points(&self) -> Option<&[Vec2]> {
        match &self.state {
            ToolState::DrawingLasso { points } => Some(points),
            _ => None,
        }
    }
}

/// Result of completing a selection tool operation.
#[derive(Debug, Clone)]
pub enum ToolResult {
    /// No result (idle or cancelled).
    None,
    /// A rectangle region was selected.
    Rectangle(Rect),
    /// A lasso path was completed.
    Lasso(Vec<Vec2>),
}

// ---------------------------------------------------------------------------
// Keyboard modifier integration
// ---------------------------------------------------------------------------

/// Keyboard modifier state for selection behaviour.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyModifiers {
    /// Ctrl key (toggle mode on macOS: Cmd).
    pub ctrl: bool,
    /// Shift key (additive mode).
    pub shift: bool,
    /// Alt key (subtractive mode).
    pub alt: bool,
}

impl KeyModifiers {
    /// Determine the effective selection mode from current modifiers.
    pub fn effective_mode(&self, base_mode: SelectionMode) -> SelectionMode {
        if self.ctrl {
            SelectionMode::Toggle
        } else if self.shift {
            SelectionMode::Additive
        } else if self.alt {
            SelectionMode::Subtractive
        } else {
            base_mode
        }
    }
}

// ---------------------------------------------------------------------------
// Point-in-polygon test for lasso selection
// ---------------------------------------------------------------------------

/// Test whether a point is inside a polygon defined by `vertices`.
///
/// Uses the ray-casting algorithm (Jordan curve theorem).
pub fn point_in_polygon(point: Vec2, vertices: &[Vec2]) -> bool {
    if vertices.len() < 3 {
        return false;
    }

    let mut inside = false;
    let n = vertices.len();
    let mut j = n - 1;
    for i in 0..n {
        let vi = vertices[i];
        let vj = vertices[j];

        if ((vi.y > point.y) != (vj.y > point.y))
            && (point.x < (vj.x - vi.x) * (point.y - vi.y) / (vj.y - vi.y) + vi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

// ---------------------------------------------------------------------------
// Selection event callbacks
// ---------------------------------------------------------------------------

/// Events emitted by the selection system.
#[derive(Debug, Clone)]
pub enum SelectionEvent {
    /// Selection changed (new selection count).
    Changed { count: usize },
    /// Hover changed to a new mark.
    HoverChanged { id: Option<u32> },
    /// Selection was cleared.
    Cleared,
}

// ---------------------------------------------------------------------------
// MarkSelectionSystem — ties selection state, tools, and interaction together
// ---------------------------------------------------------------------------

/// High-level system that integrates [`SelectionState`], [`SelectionTool`],
/// and the existing GPU interaction system to provide a complete interactive
/// mark selection workflow.
///
/// # GPU-Accelerated Hit Testing
///
/// When mark positions are registered via [`set_positions`](Self::set_positions),
/// the system can dispatch hit tests to the GPU via an [`InteractionSystem`].
/// Use [`hit_test_gpu`](Self::hit_test_gpu) for point queries and
/// [`rect_hit_test_gpu`](Self::rect_hit_test_gpu) for rectangular selections.
/// If no `InteractionSystem` is available, the system falls back to CPU-based
/// hit testing automatically via [`hit_test`](Self::hit_test) and
/// [`filter_by_rect`](Self::filter_by_rect).
///
/// # Usage
///
/// ```rust
/// use gup::mark_selection::{
///     MarkSelectionSystem, SelectionMode, SelectionStyle, SelectionToolKind,
/// };
///
/// let mut system = MarkSelectionSystem::new(10_000);
/// system.set_tool(SelectionToolKind::Rectangle);
///
/// // Process mouse events
/// system.on_mouse_down([100.0, 200.0]);
/// system.on_mouse_move([150.0, 250.0]);
///
/// // Finish selection with hit IDs
/// let hit_ids = vec![5, 12, 27];
/// system.on_mouse_up([150.0, 250.0], &hit_ids);
///
/// assert_eq!(system.state().count(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct MarkSelectionSystem {
    /// Selection state (bitset, undo/redo).
    state: SelectionState,
    /// Active selection tool.
    tool: SelectionTool,
    /// Visual style for selection feedback.
    style: SelectionStyle,
    /// Current keyboard modifiers.
    modifiers: KeyModifiers,
    /// Events emitted since last drain.
    pending_events: Vec<SelectionEvent>,
    /// Cached mark positions for hit testing (indexed by mark ID).
    /// When set, enables both CPU fallback and GPU-accelerated hit testing.
    positions: Option<Vec<[f32; 2]>>,
    /// Cached mark sizes for hit testing (radius or half-extents).
    sizes: Option<Vec<[f32; 2]>>,
    /// Monotonically increasing version counter for element data.
    /// Incremented whenever positions or sizes change, enabling the
    /// [`InteractionSystem`] to skip redundant GPU uploads.
    element_data_version: u64,
}

impl MarkSelectionSystem {
    /// Create a new system for `mark_count` marks.
    pub fn new(mark_count: usize) -> Self {
        Self {
            state: SelectionState::new(mark_count),
            tool: SelectionTool::point(),
            style: SelectionStyle::default(),
            modifiers: KeyModifiers::default(),
            pending_events: Vec::new(),
            positions: None,
            sizes: None,
            element_data_version: 0,
        }
    }

    /// Create a new system with a custom style.
    pub fn with_style(mark_count: usize, style: SelectionStyle) -> Self {
        let mut system = Self::new(mark_count);
        system.style = style;
        system
    }

    /// Get the selection state.
    pub fn state(&self) -> &SelectionState {
        &self.state
    }

    /// Get a mutable reference to the selection state.
    pub fn state_mut(&mut self) -> &mut SelectionState {
        &mut self.state
    }

    /// Get the current selection style.
    pub fn style(&self) -> &SelectionStyle {
        &self.style
    }

    /// Set the selection style.
    pub fn set_style(&mut self, style: SelectionStyle) {
        self.style = style;
    }

    /// Get the current tool kind.
    pub fn tool_kind(&self) -> &SelectionToolKind {
        &self.tool.kind
    }

    /// Set the active selection tool.
    pub fn set_tool(&mut self, kind: SelectionToolKind) {
        self.tool.cancel();
        self.tool = SelectionTool::new(kind);
    }

    /// Update keyboard modifiers (call from input handler).
    pub fn set_modifiers(&mut self, modifiers: KeyModifiers) {
        self.modifiers = modifiers;
    }

    /// Get the current keyboard modifiers.
    pub fn modifiers(&self) -> KeyModifiers {
        self.modifiers
    }

    /// Get the effective selection mode considering current modifiers.
    pub fn effective_mode(&self) -> SelectionMode {
        self.modifiers.effective_mode(self.state.mode())
    }

    // -- Input event handling --

    /// Handle a mouse-down / touch-start event.
    pub fn on_mouse_down(&mut self, position: [f32; 2]) {
        let pos = Vec2::new(position[0], position[1]);
        self.tool.begin(pos);
    }

    /// Handle a mouse-move / touch-move event.
    pub fn on_mouse_move(&mut self, position: [f32; 2]) {
        let pos = Vec2::new(position[0], position[1]);
        if self.tool.is_active() {
            self.tool.update(pos);
        }
    }

    /// Handle a mouse-up / touch-end event.
    ///
    /// `hit_ids` are the mark IDs that fall within the tool's selection area.
    /// For point tools, this should be the IDs returned by a hit test at the
    /// mouse position. For rectangle/lasso tools, these are the IDs within
    /// the completed shape.
    pub fn on_mouse_up(&mut self, _position: [f32; 2], hit_ids: &[u32]) {
        let mode = self.effective_mode();
        let additive = matches!(mode, SelectionMode::Additive | SelectionMode::Toggle);

        match self.tool.kind.clone() {
            SelectionToolKind::Point => {
                if let Some(&first) = hit_ids.first() {
                    match mode {
                        SelectionMode::Single => self.state.click(first),
                        SelectionMode::Toggle => self.state.toggle(first),
                        SelectionMode::Additive => self.state.select(first),
                        SelectionMode::Subtractive => self.state.deselect(first),
                    }
                } else if matches!(mode, SelectionMode::Single) {
                    self.state.clear();
                    self.pending_events.push(SelectionEvent::Cleared);
                }
            }
            SelectionToolKind::Rectangle => {
                self.tool.finish();
                if !hit_ids.is_empty() {
                    self.state.rect_select(hit_ids, additive);
                } else if !additive {
                    self.state.clear();
                    self.pending_events.push(SelectionEvent::Cleared);
                }
            }
            SelectionToolKind::Lasso => {
                self.tool.finish();
                if !hit_ids.is_empty() {
                    self.state.lasso_select(hit_ids, additive);
                } else if !additive {
                    self.state.clear();
                    self.pending_events.push(SelectionEvent::Cleared);
                }
            }
        }

        self.pending_events.push(SelectionEvent::Changed {
            count: self.state.count(),
        });
    }

    /// Handle a hover event at a specific mark ID.
    pub fn on_hover(&mut self, id: Option<u32>) {
        if self.state.hover() != id {
            self.state.set_hover(id);
            self.pending_events
                .push(SelectionEvent::HoverChanged { id });
        }
    }

    /// Cancel the current tool operation (e.g., on Escape key).
    pub fn cancel(&mut self) {
        self.tool.cancel();
    }

    /// Undo the last selection operation.
    pub fn undo(&mut self) -> bool {
        let result = self.state.undo();
        if result {
            self.pending_events.push(SelectionEvent::Changed {
                count: self.state.count(),
            });
        }
        result
    }

    /// Redo the last undone operation.
    pub fn redo(&mut self) -> bool {
        let result = self.state.redo();
        if result {
            self.pending_events.push(SelectionEvent::Changed {
                count: self.state.count(),
            });
        }
        result
    }

    /// Drain pending events.
    pub fn drain_events(&mut self) -> Vec<SelectionEvent> {
        std::mem::take(&mut self.pending_events)
    }

    // -- Query helpers --

    /// Get the current drag rectangle (for rendering feedback).
    pub fn current_drag_rect(&self) -> Option<Rect> {
        self.tool.current_rect()
    }

    /// Get the current lasso path (for rendering feedback).
    pub fn current_lasso_points(&self) -> Option<&[Vec2]> {
        self.tool.current_lasso_points()
    }

    /// Returns `true` if the tool is currently active (dragging).
    pub fn is_tool_active(&self) -> bool {
        self.tool.is_active()
    }

    /// Filter a list of mark positions to only those within the lasso path.
    ///
    /// `positions` should be indexed by mark ID (i.e., `positions[id]` is
    /// the position of mark `id`).
    pub fn filter_by_lasso(path: &[Vec2], positions: &[[f32; 2]]) -> Vec<u32> {
        positions
            .iter()
            .enumerate()
            .filter(|(_, pos)| point_in_polygon(Vec2::new(pos[0], pos[1]), path))
            .map(|(i, _)| i as u32)
            .collect()
    }

    /// Filter mark positions to those within a rectangle.
    pub fn filter_by_rect(rect: &Rect, positions: &[[f32; 2]]) -> Vec<u32> {
        positions
            .iter()
            .enumerate()
            .filter(|(_, pos)| {
                pos[0] >= rect.min.x
                    && pos[0] <= rect.max.x
                    && pos[1] >= rect.min.y
                    && pos[1] <= rect.max.y
            })
            .map(|(i, _)| i as u32)
            .collect()
    }

    /// Get statistics about the selection system.
    pub fn statistics(&self) -> SelectionStatistics {
        self.state.statistics()
    }

    /// Resize to accommodate a new mark count.
    pub fn resize(&mut self, new_mark_count: usize) {
        self.state.resize(new_mark_count);
    }

    /// Export the currently selected mark indices.
    pub fn export_selected(&self) -> Vec<u32> {
        self.state.serialize()
    }

    /// Import a set of selected mark indices.
    pub fn import_selected(&mut self, ids: &[u32]) {
        self.state.deserialize(ids);
        self.pending_events.push(SelectionEvent::Changed {
            count: self.state.count(),
        });
    }

    /// Apply selection as a filter: returns the indices of selected marks.
    ///
    /// This is useful for extracting selected data from a dataset.
    pub fn selected_indices(&self) -> Vec<usize> {
        self.state.selected_ids().map(|id| id as usize).collect()
    }

    /// Apply selection style to get the effective opacity for a mark.
    ///
    /// Returns the opacity multiplier: 1.0 for selected/hovered marks,
    /// `unselected_opacity` for non-selected marks when a selection exists.
    pub fn mark_opacity(&self, mark_id: u32) -> f32 {
        if self.state.is_empty() {
            // No selection active — all marks at full opacity
            return 1.0;
        }
        if self.state.is_selected(mark_id) || self.state.hover() == Some(mark_id) {
            1.0
        } else {
            self.style.unselected_opacity
        }
    }

    /// Apply selection style to get the effective scale for a mark.
    pub fn mark_scale(&self, mark_id: u32) -> f32 {
        if self.state.hover() == Some(mark_id) {
            self.style.hover_scale
        } else if self.state.is_selected(mark_id) {
            self.style.selected_scale
        } else {
            1.0
        }
    }

    /// Apply selection style to get the outline properties for a mark.
    ///
    /// Returns `(outline_color, outline_width)` or `None` if no outline.
    pub fn mark_outline(&self, mark_id: u32) -> Option<([f32; 4], f32)> {
        if self.state.hover() == Some(mark_id) {
            Some((
                self.style.hover_outline_color,
                self.style.hover_outline_width,
            ))
        } else if self.state.is_selected(mark_id) {
            Some((
                self.style.selected_outline_color,
                self.style.selected_outline_width,
            ))
        } else {
            None
        }
    }

    // -- Mark position management (for CPU and GPU hit testing) --

    /// Register mark positions for hit testing.
    ///
    /// Positions are indexed by mark ID: `positions[id]` is `[x, y]` of mark `id`.
    /// This enables the system to perform CPU-based hit testing internally,
    /// and is required for GPU-accelerated hit testing via
    /// [`hit_test_gpu`](Self::hit_test_gpu).
    ///
    /// Sizes default to `[0.01, 0.01]` per mark. Use [`set_positions_with_sizes`](Self::set_positions_with_sizes)
    /// to provide per-mark sizes.
    pub fn set_positions(&mut self, positions: Vec<[f32; 2]>) {
        let default_size = vec![[0.01f32, 0.01]; positions.len()];
        self.sizes = Some(default_size);
        self.positions = Some(positions);
        self.element_data_version += 1;
    }

    /// Register mark positions and sizes for hit testing.
    ///
    /// `sizes[id]` is `[half_width, half_height]` (or `[radius, 0]` for circles).
    pub fn set_positions_with_sizes(&mut self, positions: Vec<[f32; 2]>, sizes: Vec<[f32; 2]>) {
        assert_eq!(
            positions.len(),
            sizes.len(),
            "positions and sizes must have the same length"
        );
        self.positions = Some(positions);
        self.sizes = Some(sizes);
        self.element_data_version += 1;
    }

    /// Returns the cached positions, if any.
    pub fn positions(&self) -> Option<&[[f32; 2]]> {
        self.positions.as_deref()
    }

    /// Returns the current element data version.
    ///
    /// This counter is incremented each time positions or sizes are updated
    /// via [`set_positions`](Self::set_positions) or
    /// [`set_positions_with_sizes`](Self::set_positions_with_sizes). It is
    /// used by the GPU interaction system to avoid redundant data uploads.
    pub fn element_data_version(&self) -> u64 {
        self.element_data_version
    }

    // -- CPU hit testing --

    /// Perform a CPU-based point hit test at `position`.
    ///
    /// Returns mark IDs within hit distance, sorted by distance (closest first).
    /// Requires positions to have been set via [`set_positions`](Self::set_positions).
    /// Returns an empty vec if no positions are registered.
    pub fn hit_test(&self, position: [f32; 2], hit_radius: f32) -> Vec<u32> {
        let positions = match &self.positions {
            Some(p) => p,
            None => return Vec::new(),
        };
        let mut hits: Vec<(u32, f32)> = positions
            .iter()
            .enumerate()
            .filter_map(|(i, pos)| {
                let dx = pos[0] - position[0];
                let dy = pos[1] - position[1];
                let dist = (dx * dx + dy * dy).sqrt();
                if dist <= hit_radius {
                    Some((i as u32, dist))
                } else {
                    None
                }
            })
            .collect();
        hits.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        hits.into_iter().map(|(id, _)| id).collect()
    }

    /// Perform a CPU-based rectangle hit test.
    ///
    /// Returns mark IDs whose positions fall within `rect`.
    /// Requires positions to have been set via [`set_positions`](Self::set_positions).
    pub fn rect_hit_test(&self, rect: &Rect) -> Vec<u32> {
        match &self.positions {
            Some(positions) => Self::filter_by_rect(rect, positions),
            None => Vec::new(),
        }
    }

    /// Perform a CPU-based lasso hit test.
    ///
    /// Returns mark IDs whose positions fall within the lasso `path`.
    /// Requires positions to have been set via [`set_positions`](Self::set_positions).
    pub fn lasso_hit_test(&self, path: &[Vec2]) -> Vec<u32> {
        match &self.positions {
            Some(positions) => Self::filter_by_lasso(path, positions),
            None => Vec::new(),
        }
    }

    // -- GPU-accelerated hit testing --

    /// Build element data for the GPU interaction system from cached positions.
    fn build_element_data(&self) -> Vec<ElementData> {
        let positions = match &self.positions {
            Some(p) => p,
            None => return Vec::new(),
        };
        let sizes = self.sizes.as_deref();
        positions
            .iter()
            .enumerate()
            .map(|(i, pos)| {
                let size = sizes
                    .and_then(|s| s.get(i))
                    .copied()
                    .unwrap_or([0.01, 0.01]);
                ElementData {
                    position: *pos,
                    size,
                    mark_type: 0, // circle
                    element_id: i as u32,
                    selection_id: 0,
                    _padding: 0,
                }
            })
            .collect()
    }

    /// Perform a GPU-accelerated point hit test.
    ///
    /// Dispatches the query to `interaction_system` and returns mark IDs sorted
    /// by distance (closest first). Falls back to CPU if positions are not set.
    ///
    /// Element data is uploaded to the GPU on the first call and cached for
    /// subsequent queries. The cache is automatically invalidated when positions
    /// change via [`set_positions`](Self::set_positions).
    ///
    /// # Arguments
    ///
    /// * `position` — Query point in world/clip coordinates.
    /// * `interaction_system` — The GPU interaction system to use.
    /// * `hit_radius` — Fallback hit radius for CPU path (GPU uses element sizes).
    pub async fn hit_test_gpu(
        &self,
        position: [f32; 2],
        interaction_system: &mut InteractionSystem,
        hit_radius: f32,
    ) -> GupResult<Vec<u32>> {
        if self.positions.is_none() {
            return Ok(self.hit_test(position, hit_radius));
        }

        let elements = self.build_element_data();
        if elements.is_empty() {
            return Ok(Vec::new());
        }

        // Upload with caching — only re-uploads if version changed.
        interaction_system
            .upload_element_data_cached(&elements, self.element_data_version)
            .await?;

        // Query using cached GPU-resident data.
        let query_pos = Vec2::new(position[0], position[1]);
        let hits = interaction_system.query_point_cached(query_pos).await?;

        let mut result: Vec<(u32, f32)> = hits.iter().map(|h| (h.element_id, h.distance)).collect();
        result.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(result.into_iter().map(|(id, _)| id).collect())
    }

    /// Perform a GPU-accelerated rectangle hit test.
    ///
    /// Dispatches the query to `interaction_system` and returns mark IDs
    /// within the rectangle. Falls back to CPU if positions are not set.
    ///
    /// Element data is cached on the GPU — see [`hit_test_gpu`](Self::hit_test_gpu).
    pub async fn rect_hit_test_gpu(
        &self,
        rect: &Rect,
        interaction_system: &mut InteractionSystem,
    ) -> GupResult<Vec<u32>> {
        if self.positions.is_none() {
            return Ok(self.rect_hit_test(rect));
        }

        let elements = self.build_element_data();
        if elements.is_empty() {
            return Ok(Vec::new());
        }

        // Upload with caching.
        interaction_system
            .upload_element_data_cached(&elements, self.element_data_version)
            .await?;

        // Query using cached GPU-resident data.
        let hits = interaction_system.query_region_cached(*rect).await?;

        Ok(hits.iter().map(|h| h.element_id).collect())
    }

    /// Perform a GPU-accelerated lasso hit test.
    ///
    /// Uses the GPU interaction system for spatial candidate filtering, then
    /// applies the CPU point-in-polygon test on the candidates. For large
    /// datasets this is much faster than testing every mark.
    ///
    /// Falls back to CPU if positions are not set.
    pub async fn lasso_hit_test_gpu(
        &self,
        path: &[Vec2],
        interaction_system: &mut InteractionSystem,
    ) -> GupResult<Vec<u32>> {
        if path.len() < 3 {
            return Ok(Vec::new());
        }

        let positions = match &self.positions {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        // Compute the bounding box of the lasso path for GPU candidate filtering.
        let (mut min_x, mut min_y) = (f32::MAX, f32::MAX);
        let (mut max_x, mut max_y) = (f32::MIN, f32::MIN);
        for p in path {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        let bounding_rect = Rect::new(Vec2::new(min_x, min_y), Vec2::new(max_x, max_y));

        // Use GPU to find candidates within the bounding rect.
        let candidates = self
            .rect_hit_test_gpu(&bounding_rect, interaction_system)
            .await?;

        // Refine candidates with CPU point-in-polygon test.
        let result: Vec<u32> = candidates
            .into_iter()
            .filter(|&id| {
                if let Some(pos) = positions.get(id as usize) {
                    point_in_polygon(Vec2::new(pos[0], pos[1]), path)
                } else {
                    false
                }
            })
            .collect();

        Ok(result)
    }

    /// Perform a hit test using the best available method.
    ///
    /// If `interaction_system` is `Some`, uses GPU-accelerated hit testing.
    /// Otherwise falls back to CPU-based hit testing.
    ///
    /// This is the recommended entry point for hit testing as it handles
    /// the GPU/CPU fallback automatically.
    pub async fn hit_test_auto(
        &self,
        position: [f32; 2],
        hit_radius: f32,
        interaction_system: Option<&mut InteractionSystem>,
    ) -> GupResult<Vec<u32>> {
        match interaction_system {
            Some(is) => self.hit_test_gpu(position, is, hit_radius).await,
            None => Ok(self.hit_test(position, hit_radius)),
        }
    }

    /// Perform a rectangle hit test using the best available method.
    pub async fn rect_hit_test_auto(
        &self,
        rect: &Rect,
        interaction_system: Option<&mut InteractionSystem>,
    ) -> GupResult<Vec<u32>> {
        match interaction_system {
            Some(is) => self.rect_hit_test_gpu(rect, is).await,
            None => Ok(self.rect_hit_test(rect)),
        }
    }

    /// Perform a lasso hit test using the best available method.
    pub async fn lasso_hit_test_auto(
        &self,
        path: &[Vec2],
        interaction_system: Option<&mut InteractionSystem>,
    ) -> GupResult<Vec<u32>> {
        match interaction_system {
            Some(is) => self.lasso_hit_test_gpu(path, is).await,
            None => Ok(self.lasso_hit_test(path)),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
/// Adapter that wraps a `Vec<ElementData>` to implement [`Renderable`]
/// for use with the GPU [`InteractionSystem`].
#[derive(Debug)]
struct ElementDataRenderable {
    elements: Vec<crate::interaction::InteractionElement>,
}

#[cfg(test)]
impl ElementDataRenderable {
    fn new(data: Vec<ElementData>) -> Self {
        let elements = data
            .into_iter()
            .map(|e| crate::interaction::InteractionElement {
                position: e.position,
                size: e.size,
                mark_type: e.mark_type,
            })
            .collect();
        Self { elements }
    }
}

#[cfg(test)]
impl crate::interaction::Renderable for ElementDataRenderable {
    fn get_elements_for_interaction(
        &self,
    ) -> GupResult<Vec<crate::interaction::InteractionElement>> {
        Ok(self.elements.clone())
    }

    fn selection_id(&self) -> u32 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- BitSet tests --

    #[test]
    fn test_bitset_new() {
        let bs = BitSet::new(100);
        assert_eq!(bs.len(), 100);
        assert_eq!(bs.count_ones(), 0);
        assert!(!bs.get(0));
        assert!(!bs.get(99));
    }

    #[test]
    fn test_bitset_set_clear_get() {
        let mut bs = BitSet::new(256);
        bs.set(0);
        bs.set(63);
        bs.set(64);
        bs.set(255);
        assert!(bs.get(0));
        assert!(bs.get(63));
        assert!(bs.get(64));
        assert!(bs.get(255));
        assert!(!bs.get(1));
        assert_eq!(bs.count_ones(), 4);

        bs.clear_bit(63);
        assert!(!bs.get(63));
        assert_eq!(bs.count_ones(), 3);
    }

    #[test]
    fn test_bitset_toggle() {
        let mut bs = BitSet::new(10);
        bs.toggle(5);
        assert!(bs.get(5));
        bs.toggle(5);
        assert!(!bs.get(5));
    }

    #[test]
    fn test_bitset_set_all_clear_all() {
        let mut bs = BitSet::new(100);
        bs.set_all();
        assert_eq!(bs.count_ones(), 100);
        // Ensure trailing bits are not set beyond len
        assert!(!bs.get(100));

        bs.clear_all();
        assert_eq!(bs.count_ones(), 0);
    }

    #[test]
    fn test_bitset_ones_iterator() {
        let mut bs = BitSet::new(200);
        bs.set(5);
        bs.set(64);
        bs.set(128);
        let ones: Vec<usize> = bs.ones().collect();
        assert_eq!(ones, vec![5, 64, 128]);
    }

    #[test]
    fn test_bitset_resize() {
        let mut bs = BitSet::new(50);
        bs.set(10);
        bs.set(49);
        bs.resize(100);
        assert!(bs.get(10));
        assert!(bs.get(49));
        assert!(!bs.get(50));
        assert_eq!(bs.len(), 100);

        bs.resize(20);
        assert!(bs.get(10));
        assert!(!bs.get(49)); // no longer valid
        assert_eq!(bs.len(), 20);
    }

    #[test]
    fn test_bitset_union_intersect() {
        let mut a = BitSet::new(64);
        let mut b = BitSet::new(64);
        a.set(1);
        a.set(2);
        b.set(2);
        b.set(3);

        let u = a.union(&b);
        assert_eq!(u.count_ones(), 3); // 1, 2, 3
        let i = a.intersect(&b);
        assert_eq!(i.count_ones(), 1); // 2
    }

    #[test]
    fn test_bitset_memory() {
        let bs = BitSet::new(1_000_000);
        // 1M bits = 15625 * u64 = 125000 bytes ≈ 122 KB
        assert!(bs.memory_bytes() <= 125_008); // allow small rounding
    }

    // -- SelectionState tests --

    #[test]
    fn test_selection_state_basic() {
        let mut state = SelectionState::new(100);
        assert_eq!(state.count(), 0);
        assert!(state.is_empty());

        state.select(5);
        assert!(state.is_selected(5));
        assert_eq!(state.count(), 1);

        state.deselect(5);
        assert!(!state.is_selected(5));
        assert_eq!(state.count(), 0);
    }

    #[test]
    fn test_selection_state_toggle() {
        let mut state = SelectionState::new(100);
        state.toggle(42);
        assert!(state.is_selected(42));
        state.toggle(42);
        assert!(!state.is_selected(42));
    }

    #[test]
    fn test_selection_state_undo_redo() {
        let mut state = SelectionState::new(100);
        state.select(10);
        state.select(20);
        assert_eq!(state.count(), 2);

        // Undo last select
        assert!(state.undo());
        assert_eq!(state.count(), 1);
        assert!(!state.is_selected(20));
        assert!(state.is_selected(10));

        // Redo
        assert!(state.redo());
        assert_eq!(state.count(), 2);
        assert!(state.is_selected(20));
    }

    #[test]
    fn test_selection_state_undo_clear() {
        let mut state = SelectionState::new(100);
        state.select(1);
        state.select(2);
        state.select(3);
        assert_eq!(state.count(), 3);

        state.clear();
        assert_eq!(state.count(), 0);

        state.undo();
        assert_eq!(state.count(), 3);
    }

    #[test]
    fn test_selection_state_select_all() {
        let mut state = SelectionState::new(10);
        state.select_all();
        assert_eq!(state.count(), 10);

        state.undo();
        assert_eq!(state.count(), 0);
    }

    #[test]
    fn test_selection_state_click_single_mode() {
        let mut state = SelectionState::with_mode(100, SelectionMode::Single);
        state.select(5);
        state.select(10);
        assert_eq!(state.count(), 2);

        // Click in single mode: should clear all and select only the clicked mark
        state.click(42);
        assert_eq!(state.count(), 1);
        assert!(state.is_selected(42));
        assert!(!state.is_selected(5));
        assert!(!state.is_selected(10));
    }

    #[test]
    fn test_selection_state_rect_select() {
        let mut state = SelectionState::new(100);
        state.select(1);

        // Non-additive rect select replaces existing selection
        state.rect_select(&[10, 20, 30], false);
        assert_eq!(state.count(), 3);
        assert!(!state.is_selected(1));
        assert!(state.is_selected(10));

        // Undo restores previous state
        state.undo();
        assert_eq!(state.count(), 1);
        assert!(state.is_selected(1));
    }

    #[test]
    fn test_selection_state_rect_select_additive() {
        let mut state = SelectionState::new(100);
        state.select(1);
        state.rect_select(&[10, 20], true);
        assert_eq!(state.count(), 3);
        assert!(state.is_selected(1));
        assert!(state.is_selected(10));
    }

    #[test]
    fn test_selection_state_hover() {
        let mut state = SelectionState::new(100);
        assert_eq!(state.hover(), None);
        state.set_hover(Some(42));
        assert_eq!(state.hover(), Some(42));
        state.set_hover(None);
        assert_eq!(state.hover(), None);
    }

    #[test]
    fn test_selection_state_statistics() {
        let mut state = SelectionState::new(1000);
        state.select(5);
        state.select(10);
        state.set_hover(Some(15));

        let stats = state.statistics();
        assert_eq!(stats.total_marks, 1000);
        assert_eq!(stats.selected_count, 2);
        assert_eq!(stats.hover_id, Some(15));
        assert_eq!(stats.undo_depth, 2);
        assert_eq!(stats.redo_depth, 0);
    }

    #[test]
    fn test_selection_state_serialize_deserialize() {
        let mut state = SelectionState::new(100);
        state.select(5);
        state.select(10);
        state.select(50);

        let serialized = state.serialize();
        assert_eq!(serialized, vec![5, 10, 50]);

        let mut state2 = SelectionState::new(100);
        state2.deserialize(&serialized);
        assert_eq!(state2.count(), 3);
        assert!(state2.is_selected(5));
        assert!(state2.is_selected(10));
        assert!(state2.is_selected(50));
    }

    #[test]
    fn test_selection_state_resize() {
        let mut state = SelectionState::new(50);
        state.select(10);
        state.select(49);
        state.resize(100);
        assert!(state.is_selected(10));
        assert!(state.is_selected(49));
        assert_eq!(state.mark_count(), 100);
    }

    // -- SelectionMode tests --

    #[test]
    fn test_key_modifiers_effective_mode() {
        let mods = KeyModifiers {
            ctrl: true,
            shift: false,
            alt: false,
        };
        assert_eq!(
            mods.effective_mode(SelectionMode::Single),
            SelectionMode::Toggle
        );

        let mods = KeyModifiers {
            ctrl: false,
            shift: true,
            alt: false,
        };
        assert_eq!(
            mods.effective_mode(SelectionMode::Single),
            SelectionMode::Additive
        );

        let mods = KeyModifiers {
            ctrl: false,
            shift: false,
            alt: true,
        };
        assert_eq!(
            mods.effective_mode(SelectionMode::Single),
            SelectionMode::Subtractive
        );

        let mods = KeyModifiers::default();
        assert_eq!(
            mods.effective_mode(SelectionMode::Single),
            SelectionMode::Single
        );
    }

    // -- SelectionTool tests --

    #[test]
    fn test_tool_point() {
        let mut tool = SelectionTool::point();
        assert!(!tool.is_active());
        tool.begin(Vec2::new(10.0, 20.0));
        // Point tool stays idle since clicks are handled directly
        assert!(!tool.is_active());
    }

    #[test]
    fn test_tool_rectangle() {
        let mut tool = SelectionTool::rectangle();
        tool.begin(Vec2::new(10.0, 20.0));
        assert!(tool.is_active());

        tool.update(Vec2::new(50.0, 60.0));
        let rect = tool.current_rect().unwrap();
        assert_eq!(rect.min.x, 10.0);
        assert_eq!(rect.min.y, 20.0);
        assert_eq!(rect.max.x, 50.0);
        assert_eq!(rect.max.y, 60.0);

        let result = tool.finish();
        assert!(matches!(result, ToolResult::Rectangle(_)));
        assert!(!tool.is_active());
    }

    #[test]
    fn test_tool_rectangle_reversed() {
        let mut tool = SelectionTool::rectangle();
        tool.begin(Vec2::new(50.0, 60.0));
        tool.update(Vec2::new(10.0, 20.0));
        let rect = tool.current_rect().unwrap();
        // Normalised so min < max
        assert_eq!(rect.min.x, 10.0);
        assert_eq!(rect.min.y, 20.0);
        assert_eq!(rect.max.x, 50.0);
        assert_eq!(rect.max.y, 60.0);
    }

    #[test]
    fn test_tool_lasso() {
        let mut tool = SelectionTool::lasso();
        tool.begin(Vec2::new(0.0, 0.0));
        tool.update(Vec2::new(10.0, 0.0));
        tool.update(Vec2::new(10.0, 10.0));
        tool.update(Vec2::new(0.0, 10.0));

        let points = tool.current_lasso_points().unwrap();
        assert_eq!(points.len(), 4);

        let result = tool.finish();
        assert!(matches!(result, ToolResult::Lasso(_)));
    }

    #[test]
    fn test_tool_cancel() {
        let mut tool = SelectionTool::rectangle();
        tool.begin(Vec2::new(10.0, 20.0));
        assert!(tool.is_active());
        tool.cancel();
        assert!(!tool.is_active());
    }

    // -- Point-in-polygon tests --

    #[test]
    fn test_point_in_polygon_basic() {
        let square = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        ];

        assert!(point_in_polygon(Vec2::new(5.0, 5.0), &square));
        assert!(!point_in_polygon(Vec2::new(15.0, 5.0), &square));
        assert!(!point_in_polygon(Vec2::new(-1.0, -1.0), &square));
    }

    #[test]
    fn test_point_in_polygon_triangle() {
        let triangle = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(5.0, 10.0),
        ];

        assert!(point_in_polygon(Vec2::new(5.0, 3.0), &triangle));
        assert!(!point_in_polygon(Vec2::new(0.0, 10.0), &triangle));
    }

    #[test]
    fn test_point_in_polygon_degenerate() {
        // Too few points
        assert!(!point_in_polygon(Vec2::new(0.0, 0.0), &[]));
        assert!(!point_in_polygon(
            Vec2::new(0.0, 0.0),
            &[Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0)]
        ));
    }

    // -- SelectionStyle tests --

    #[test]
    fn test_selection_style_defaults() {
        let style = SelectionStyle::default();
        assert_eq!(style.selected_scale, 1.0);
        assert_eq!(style.hover_scale, 1.1);
        assert!(style.unselected_opacity > 0.0);
        assert!(style.unselected_opacity < 1.0);
    }

    #[test]
    fn test_selection_style_presets() {
        let highlight = SelectionStyle::highlight();
        assert!(highlight.selected_outline_width < f32::EPSILON);

        let outline = SelectionStyle::outline();
        assert!(outline.selected_outline_width > 0.0);
    }

    // -- MarkSelectionSystem tests --

    #[test]
    fn test_system_point_click() {
        let mut system = MarkSelectionSystem::new(100);
        system.on_mouse_down([50.0, 50.0]);
        system.on_mouse_up([50.0, 50.0], &[5]);
        assert_eq!(system.state().count(), 1);
        assert!(system.state().is_selected(5));
    }

    #[test]
    fn test_system_point_click_empty() {
        let mut system = MarkSelectionSystem::new(100);
        system.state_mut().select(5);
        // Click on empty space in single mode → clears selection
        system.on_mouse_down([50.0, 50.0]);
        system.on_mouse_up([50.0, 50.0], &[]);
        assert_eq!(system.state().count(), 0);
    }

    #[test]
    fn test_system_rect_select() {
        let mut system = MarkSelectionSystem::new(100);
        system.set_tool(SelectionToolKind::Rectangle);
        system.on_mouse_down([10.0, 10.0]);
        system.on_mouse_move([50.0, 50.0]);
        system.on_mouse_up([50.0, 50.0], &[3, 7, 12]);
        assert_eq!(system.state().count(), 3);
    }

    #[test]
    fn test_system_modifiers() {
        let mut system = MarkSelectionSystem::new(100);
        system.on_mouse_down([10.0, 10.0]);
        system.on_mouse_up([10.0, 10.0], &[5]);

        // Shift+click → additive
        system.set_modifiers(KeyModifiers {
            ctrl: false,
            shift: true,
            alt: false,
        });
        system.on_mouse_down([20.0, 20.0]);
        system.on_mouse_up([20.0, 20.0], &[10]);
        assert_eq!(system.state().count(), 2);
        assert!(system.state().is_selected(5));
        assert!(system.state().is_selected(10));
    }

    #[test]
    fn test_system_hover() {
        let mut system = MarkSelectionSystem::new(100);
        system.on_hover(Some(42));
        assert_eq!(system.state().hover(), Some(42));

        let events = system.drain_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            SelectionEvent::HoverChanged { id: Some(42) }
        ));
    }

    #[test]
    fn test_system_undo_redo() {
        let mut system = MarkSelectionSystem::new(100);
        system.on_mouse_down([10.0, 10.0]);
        system.on_mouse_up([10.0, 10.0], &[5]);
        assert_eq!(system.state().count(), 1);

        assert!(system.undo());
        assert_eq!(system.state().count(), 0);

        assert!(system.redo());
        assert_eq!(system.state().count(), 1);
    }

    #[test]
    fn test_system_filter_by_rect() {
        let positions = vec![
            [5.0, 5.0],
            [15.0, 15.0],
            [25.0, 25.0],
            [35.0, 35.0],
            [50.0, 50.0],
        ];
        let rect = Rect::new(Vec2::new(10.0, 10.0), Vec2::new(40.0, 40.0));
        let ids = MarkSelectionSystem::filter_by_rect(&rect, &positions);
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn test_system_filter_by_lasso() {
        let lasso = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(20.0, 0.0),
            Vec2::new(20.0, 20.0),
            Vec2::new(0.0, 20.0),
        ];
        let positions = vec![[10.0, 10.0], [30.0, 30.0], [5.0, 5.0]];
        let ids = MarkSelectionSystem::filter_by_lasso(&lasso, &positions);
        assert_eq!(ids, vec![0, 2]);
    }

    #[test]
    fn test_system_mark_opacity() {
        let mut system = MarkSelectionSystem::new(100);
        // No selection → all opaque
        assert_eq!(system.mark_opacity(5), 1.0);

        system.state_mut().select(5);
        assert_eq!(system.mark_opacity(5), 1.0);
        assert_eq!(system.mark_opacity(10), system.style().unselected_opacity);
    }

    #[test]
    fn test_system_mark_scale() {
        let mut system = MarkSelectionSystem::new(100);
        system.state_mut().select(5);
        system.state_mut().set_hover(Some(10));

        assert_eq!(system.mark_scale(5), system.style().selected_scale);
        assert_eq!(system.mark_scale(10), system.style().hover_scale);
        assert_eq!(system.mark_scale(20), 1.0);
    }

    #[test]
    fn test_system_mark_outline() {
        let mut system = MarkSelectionSystem::new(100);
        system.state_mut().select(5);
        system.state_mut().set_hover(Some(10));

        let outline = system.mark_outline(5).unwrap();
        assert_eq!(outline.0, system.style().selected_outline_color);

        let hover_outline = system.mark_outline(10).unwrap();
        assert_eq!(hover_outline.0, system.style().hover_outline_color);

        assert!(system.mark_outline(20).is_none());
    }

    #[test]
    fn test_system_export_import() {
        let mut system = MarkSelectionSystem::new(100);
        system.state_mut().select(5);
        system.state_mut().select(10);

        let exported = system.export_selected();
        assert_eq!(exported, vec![5, 10]);

        let mut system2 = MarkSelectionSystem::new(100);
        system2.import_selected(&exported);
        assert_eq!(system2.state().count(), 2);
        assert!(system2.state().is_selected(5));
        assert!(system2.state().is_selected(10));
    }

    #[test]
    fn test_system_drain_events() {
        let mut system = MarkSelectionSystem::new(100);
        system.on_mouse_down([10.0, 10.0]);
        system.on_mouse_up([10.0, 10.0], &[5]);

        let events = system.drain_events();
        assert!(!events.is_empty());

        // Second drain should be empty
        let events2 = system.drain_events();
        assert!(events2.is_empty());
    }

    #[test]
    fn test_system_cancel() {
        let mut system = MarkSelectionSystem::new(100);
        system.set_tool(SelectionToolKind::Rectangle);
        system.on_mouse_down([10.0, 10.0]);
        assert!(system.is_tool_active());
        system.cancel();
        assert!(!system.is_tool_active());
    }

    #[test]
    fn test_system_resize() {
        let mut system = MarkSelectionSystem::new(50);
        system.state_mut().select(10);
        system.resize(100);
        assert!(system.state().is_selected(10));
        assert_eq!(system.state().mark_count(), 100);
    }

    // -- Position-based hit testing tests --

    #[test]
    fn test_set_positions() {
        let mut system = MarkSelectionSystem::new(3);
        assert!(system.positions().is_none());

        system.set_positions(vec![[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]]);
        assert!(system.positions().is_some());
        assert_eq!(system.positions().unwrap().len(), 3);
    }

    #[test]
    fn test_set_positions_with_sizes() {
        let mut system = MarkSelectionSystem::new(2);
        system.set_positions_with_sizes(vec![[0.0, 0.0], [1.0, 1.0]], vec![[0.1, 0.1], [0.2, 0.2]]);
        assert_eq!(system.positions().unwrap().len(), 2);
    }

    #[test]
    fn test_hit_test_cpu() {
        let mut system = MarkSelectionSystem::new(5);
        system.set_positions(vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [0.0, 1.0],
            [1.0, 1.0],
            [5.0, 5.0],
        ]);

        // Hit test near origin — should find mark 0
        let hits = system.hit_test([0.05, 0.05], 0.1);
        assert_eq!(hits, vec![0]);

        // Hit test near (1, 0) — should find mark 1
        let hits = system.hit_test([1.0, 0.0], 0.1);
        assert_eq!(hits, vec![1]);

        // Hit test far from all marks — should find nothing
        let hits = system.hit_test([10.0, 10.0], 0.1);
        assert!(hits.is_empty());
    }

    #[test]
    fn test_hit_test_cpu_sorted_by_distance() {
        let mut system = MarkSelectionSystem::new(3);
        system.set_positions(vec![[0.0, 0.0], [0.1, 0.0], [0.05, 0.0]]);

        // Large radius to hit all three — closest first
        let hits = system.hit_test([0.0, 0.0], 0.5);
        assert_eq!(hits, vec![0, 2, 1]); // 0 is at 0.0, 2 is at 0.05, 1 is at 0.1
    }

    #[test]
    fn test_hit_test_no_positions() {
        let system = MarkSelectionSystem::new(5);
        // No positions set — returns empty
        let hits = system.hit_test([0.0, 0.0], 0.1);
        assert!(hits.is_empty());
    }

    #[test]
    fn test_rect_hit_test_cpu() {
        let mut system = MarkSelectionSystem::new(5);
        system.set_positions(vec![
            [0.0, 0.0],
            [0.5, 0.5],
            [1.0, 1.0],
            [1.5, 1.5],
            [2.0, 2.0],
        ]);

        let rect = Rect::new(Vec2::new(0.4, 0.4), Vec2::new(1.6, 1.6));
        let hits = system.rect_hit_test(&rect);
        assert_eq!(hits, vec![1, 2, 3]);
    }

    #[test]
    fn test_lasso_hit_test_cpu() {
        let mut system = MarkSelectionSystem::new(4);
        system.set_positions(vec![
            [0.5, 0.5], // inside
            [1.5, 1.5], // outside
            [0.2, 0.2], // inside
            [5.0, 5.0], // outside
        ]);

        let lasso = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ];
        let hits = system.lasso_hit_test(&lasso);
        assert_eq!(hits, vec![0, 2]);
    }

    #[test]
    fn test_build_element_data() {
        let mut system = MarkSelectionSystem::new(3);
        system.set_positions_with_sizes(
            vec![[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]],
            vec![[0.1, 0.1], [0.2, 0.2], [0.3, 0.3]],
        );

        let elements = system.build_element_data();
        assert_eq!(elements.len(), 3);
        assert_eq!(elements[0].position, [1.0, 2.0]);
        assert_eq!(elements[0].size, [0.1, 0.1]);
        assert_eq!(elements[0].element_id, 0);
        assert_eq!(elements[2].position, [5.0, 6.0]);
        assert_eq!(elements[2].element_id, 2);
    }

    #[test]
    fn test_build_element_data_no_positions() {
        let system = MarkSelectionSystem::new(3);
        let elements = system.build_element_data();
        assert!(elements.is_empty());
    }

    #[test]
    fn test_element_data_renderable() {
        use crate::interaction::Renderable;

        let data = vec![
            ElementData {
                position: [1.0, 2.0],
                size: [0.1, 0.1],
                mark_type: 0,
                element_id: 0,
                selection_id: 0,
                _padding: 0,
            },
            ElementData {
                position: [3.0, 4.0],
                size: [0.2, 0.2],
                mark_type: 0,
                element_id: 1,
                selection_id: 0,
                _padding: 0,
            },
        ];

        let renderable = ElementDataRenderable::new(data);
        assert_eq!(renderable.selection_id(), 0);
        let elements = renderable.get_elements_for_interaction().unwrap();
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].position, [1.0, 2.0]);
        assert_eq!(elements[1].position, [3.0, 4.0]);
    }

    // -- Element data version tracking (GUP-194) --

    #[test]
    fn test_initial_version_is_zero() {
        let system = MarkSelectionSystem::new(100);
        assert_eq!(system.element_data_version(), 0);
    }

    #[test]
    fn test_set_positions_increments_version() {
        let mut system = MarkSelectionSystem::new(100);
        assert_eq!(system.element_data_version(), 0);

        system.set_positions(vec![[1.0, 2.0]; 10]);
        assert_eq!(system.element_data_version(), 1);

        system.set_positions(vec![[3.0, 4.0]; 10]);
        assert_eq!(system.element_data_version(), 2);
    }

    #[test]
    fn test_set_positions_with_sizes_increments_version() {
        let mut system = MarkSelectionSystem::new(100);

        system.set_positions_with_sizes(vec![[1.0, 2.0]; 10], vec![[0.5, 0.5]; 10]);
        assert_eq!(system.element_data_version(), 1);

        system.set_positions_with_sizes(vec![[3.0, 4.0]; 10], vec![[1.0, 1.0]; 10]);
        assert_eq!(system.element_data_version(), 2);
    }

    #[test]
    fn test_version_increments_monotonically() {
        let mut system = MarkSelectionSystem::new(100);

        for i in 1..=10 {
            system.set_positions(vec![[0.0, 0.0]; 10]);
            assert_eq!(system.element_data_version(), i);
        }
    }

    #[test]
    fn test_same_data_still_increments_version() {
        let mut system = MarkSelectionSystem::new(100);
        let positions = vec![[1.0, 2.0]; 10];

        system.set_positions(positions.clone());
        assert_eq!(system.element_data_version(), 1);

        // Setting the same positions again should still increment
        system.set_positions(positions);
        assert_eq!(system.element_data_version(), 2);
    }
}
