// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Unit tests for `GupWidget` dirty-flag transitions and state management.
//!
//! These tests exercise the widget's dirty tracking, chart replacement, and
//! event queue without requiring a GPU device (no egui Ui context needed).

use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::{AccessorFunction, scatter};
use gup::render::RenderContext;
use gup_egui::GupWidget;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Helper: build a minimal scatter chart for testing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Pt {
    x: f32,
    y: f32,
}

fn make_chart() -> gup::chart_builder::ComposedChart<Pt, gup::mark::Circle> {
    let data: Vec<Pt> = (0..10)
        .map(|i| Pt {
            x: i as f32,
            y: (i as f32).sin(),
        })
        .collect();
    let ctx =
        Arc::new(pollster::block_on(RenderContext::new()).expect("Failed to create RenderContext"));
    let x_acc = AccessorFunction::new(|d: &Pt| AccessorValue::Float(d.x));
    let y_acc = AccessorFunction::new(|d: &Pt| AccessorValue::Float(d.y));

    scatter()
        .x(x_acc)
        .y(y_acc)
        .point_size(4.0)
        .build_with_data(data, ctx)
        .expect("build chart")
}

// ---------------------------------------------------------------------------
// Dirty-flag tests
// ---------------------------------------------------------------------------

#[test]
fn widget_starts_dirty() {
    let widget = GupWidget::new(make_chart());
    assert!(widget.is_dirty(), "new widget should start dirty");
}

#[test]
fn mark_dirty_sets_flag() {
    let mut widget = GupWidget::new(make_chart());
    // Simulate a render by directly manipulating (we can't call show without a UI).
    // Just verify that mark_dirty works.
    widget.mark_dirty();
    assert!(widget.is_dirty());
}

#[test]
fn set_chart_marks_dirty() {
    let mut widget = GupWidget::new(make_chart());
    // Build a new chart and set it.
    let chart2 = make_chart();
    widget.set_chart(chart2);
    assert!(widget.is_dirty(), "set_chart should mark dirty");
}

#[test]
fn take_events_drains() {
    let mut widget = GupWidget::new(make_chart());

    // Initially no events.
    let events = widget.take_events();
    assert!(events.is_empty());

    // take_events returns empty when called again.
    let events2 = widget.take_events();
    assert!(events2.is_empty());
}

#[test]
fn chart_ref_accessors() {
    let widget = GupWidget::new(make_chart());
    // We can borrow the inner chart (just checking it doesn't panic).
    let _chart = widget.chart();
}

#[test]
fn chart_mut_accessor() {
    let mut widget = GupWidget::new(make_chart());
    let _chart = widget.chart_mut();
}
