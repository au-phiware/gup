// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Translates egui pointer events into Gup [`InteractionEvent`] types.
//!
//! The bridge handles coordinate mapping from egui's panel-relative
//! coordinate system to Gup's chart coordinate space, accounting for the
//! widget's position within the egui layout.

use gup::event::ModifierFlags;
use gup::interaction::InteractionEvent;

/// Map an egui screen position to chart-local coordinates.
///
/// `pos` is in egui logical points (absolute screen position).
/// `chart_rect` is the widget's allocated rectangle in the same coordinate
/// space. The returned coordinates are relative to the top-left corner of the
/// chart, in physical pixels.
pub fn map_to_chart_coords(
    pos: egui::Pos2,
    chart_rect: egui::Rect,
    pixels_per_point: f32,
) -> [f32; 2] {
    let local_x = (pos.x - chart_rect.min.x) * pixels_per_point;
    let local_y = (pos.y - chart_rect.min.y) * pixels_per_point;
    [local_x, local_y]
}

/// Translate egui modifier state into Gup's [`ModifierFlags`].
fn translate_modifiers(ui_modifiers: &egui::Modifiers) -> ModifierFlags {
    ModifierFlags {
        shift: ui_modifiers.shift,
        ctrl: ui_modifiers.ctrl,
        alt: ui_modifiers.alt,
        meta: ui_modifiers.mac_cmd || ui_modifiers.command,
    }
}

/// Translate an [`egui::Response`] into a vector of Gup
/// [`InteractionEvent`] types.
///
/// This inspects the response for hover, click, drag, and scroll events and
/// creates the corresponding Gup events with coordinates mapped to the
/// chart's physical pixel space.
///
/// # Arguments
///
/// * `response` — The egui response from the chart widget.
/// * `chart_rect` — The widget's allocated rectangle in egui screen
///   coordinates.
/// * `pixels_per_point` — The current display scale factor (from
///   `ui.ctx().pixels_per_point()`).
pub fn translate_response(
    response: &egui::Response,
    chart_rect: egui::Rect,
    pixels_per_point: f32,
) -> Vec<InteractionEvent> {
    let mut events = Vec::new();
    let ctx = response.ctx.clone();
    let modifiers = ctx.input(|i| i.modifiers);
    let gup_modifiers = translate_modifiers(&modifiers);

    // Hover / mouse move
    if response.hovered() {
        if let Some(pos) = response.hover_pos() {
            let [x, y] = map_to_chart_coords(pos, chart_rect, pixels_per_point);
            let mut event = InteractionEvent::new("mousemove", gup::interaction::Vec2::new(x, y));
            event.modifiers = gup_modifiers;
            events.push(event);
        }
    }

    // Click (primary button)
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let [x, y] = map_to_chart_coords(pos, chart_rect, pixels_per_point);
            let mut event = InteractionEvent::new("click", gup::interaction::Vec2::new(x, y));
            event.modifiers = gup_modifiers;
            events.push(event);
        }
    }

    // Secondary click (right-click)
    if response.secondary_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let [x, y] = map_to_chart_coords(pos, chart_rect, pixels_per_point);
            let mut event = InteractionEvent::new("contextmenu", gup::interaction::Vec2::new(x, y));
            event.modifiers = gup_modifiers;
            events.push(event);
        }
    }

    // Drag (mouse down + move)
    if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let [x, y] = map_to_chart_coords(pos, chart_rect, pixels_per_point);
            let mut event = InteractionEvent::new("drag", gup::interaction::Vec2::new(x, y));
            event.modifiers = gup_modifiers;
            events.push(event);
        }
    }

    // Drag started
    if response.drag_started() {
        if let Some(pos) = response.interact_pointer_pos() {
            let [x, y] = map_to_chart_coords(pos, chart_rect, pixels_per_point);
            let mut event = InteractionEvent::new("mousedown", gup::interaction::Vec2::new(x, y));
            event.modifiers = gup_modifiers;
            events.push(event);
        }
    }

    // Drag stopped (mouse up after drag)
    if response.drag_stopped() {
        if let Some(pos) = response.interact_pointer_pos() {
            let [x, y] = map_to_chart_coords(pos, chart_rect, pixels_per_point);
            let mut event = InteractionEvent::new("mouseup", gup::interaction::Vec2::new(x, y));
            event.modifiers = gup_modifiers;
            events.push(event);
        }
    }

    // Scroll events — forward rather than silently drop (AC3).
    let scroll_delta = ctx.input(|i| {
        i.events
            .iter()
            .filter_map(|e| {
                if let egui::Event::MouseWheel { delta, .. } = e {
                    Some(*delta)
                } else {
                    None
                }
            })
            .fold(egui::Vec2::ZERO, |acc, d| acc + d)
    });

    if scroll_delta != egui::Vec2::ZERO && response.hovered() {
        if let Some(pos) = response.hover_pos() {
            let [x, y] = map_to_chart_coords(pos, chart_rect, pixels_per_point);
            let mut event = InteractionEvent::new("scroll", gup::interaction::Vec2::new(x, y));
            event
                .metadata
                .insert("scroll_x".to_string(), scroll_delta.x.to_string());
            event
                .metadata
                .insert("scroll_y".to_string(), scroll_delta.y.to_string());
            event.modifiers = gup_modifiers;
            events.push(event);
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_to_chart_coords_origin() {
        let pos = egui::Pos2::new(100.0, 200.0);
        let rect = egui::Rect::from_min_size(egui::pos2(100.0, 200.0), egui::vec2(400.0, 300.0));
        let [x, y] = map_to_chart_coords(pos, rect, 1.0);
        assert!((x - 0.0).abs() < f32::EPSILON);
        assert!((y - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_map_to_chart_coords_offset() {
        let pos = egui::Pos2::new(150.0, 250.0);
        let rect = egui::Rect::from_min_size(egui::pos2(100.0, 200.0), egui::vec2(400.0, 300.0));
        let [x, y] = map_to_chart_coords(pos, rect, 1.0);
        assert!((x - 50.0).abs() < f32::EPSILON);
        assert!((y - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_map_to_chart_coords_with_scale() {
        let pos = egui::Pos2::new(150.0, 250.0);
        let rect = egui::Rect::from_min_size(egui::pos2(100.0, 200.0), egui::vec2(400.0, 300.0));
        let [x, y] = map_to_chart_coords(pos, rect, 2.0);
        // 50 logical points * 2.0 ppp = 100 physical pixels.
        assert!((x - 100.0).abs() < f32::EPSILON);
        assert!((y - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_map_to_chart_coords_fractional_scale() {
        let pos = egui::Pos2::new(110.0, 210.0);
        let rect = egui::Rect::from_min_size(egui::pos2(100.0, 200.0), egui::vec2(400.0, 300.0));
        let [x, y] = map_to_chart_coords(pos, rect, 1.5);
        assert!((x - 15.0).abs() < f32::EPSILON);
        assert!((y - 15.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_translate_modifiers_none() {
        let mods = egui::Modifiers::NONE;
        let flags = translate_modifiers(&mods);
        assert!(!flags.shift);
        assert!(!flags.ctrl);
        assert!(!flags.alt);
        assert!(!flags.meta);
    }

    #[test]
    fn test_translate_modifiers_shift() {
        let mods = egui::Modifiers::SHIFT;
        let flags = translate_modifiers(&mods);
        assert!(flags.shift);
        assert!(!flags.ctrl);
    }

    #[test]
    fn test_translate_modifiers_ctrl() {
        let mods = egui::Modifiers::CTRL;
        let flags = translate_modifiers(&mods);
        assert!(flags.ctrl);
    }

    #[test]
    fn test_translate_modifiers_command_maps_to_meta() {
        let mods = egui::Modifiers::COMMAND;
        let flags = translate_modifiers(&mods);
        assert!(flags.meta);
    }
}
