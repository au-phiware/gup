// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

use gup_core::geom::Point;
use gup_core::{Button, Input};

/// The `Input` for an egui event, in coordinates relative to `rect`.
///
/// - `PointerMoved` becomes `PointerMoved` (outside `rect` too, so a drag
///   can leave it); `PointerGone` becomes `PointerLeft`.
/// - A primary, secondary or middle press **inside** `rect` becomes
///   `PointerDown`; a release of one of those buttons anywhere becomes
///   `PointerUp`. Other buttons give `None`.
/// - `MouseWheel` becomes `Wheel` with `lines` of `delta.y` for `Line`
///   units, `delta.y / 40` for `Point` and `delta.y × 10` for `Page`. egui's
///   wheel event carries no position, so `at` is the centre of `rect`;
///   [`GupWidget::show`](crate::GupWidget::show) replaces it with the
///   hovered position and drops the event when the pointer is elsewhere.
///
/// Positions are in points, which are Gup logical pixels.
pub fn to_input(event: &egui::Event, rect: egui::Rect) -> Option<Input> {
    let local = |p: egui::Pos2| Point::new(p.x - rect.min.x, p.y - rect.min.y);
    let button = |b: egui::PointerButton| match b {
        egui::PointerButton::Primary => Some(Button::Primary),
        egui::PointerButton::Secondary => Some(Button::Secondary),
        egui::PointerButton::Middle => Some(Button::Middle),
        _ => None,
    };
    match *event {
        egui::Event::PointerMoved(p) => Some(Input::PointerMoved { at: local(p) }),
        egui::Event::PointerGone => Some(Input::PointerLeft),
        egui::Event::PointerButton {
            pos,
            button: b,
            pressed,
            ..
        } => {
            let button = button(b)?;
            if pressed {
                rect.contains(pos).then(|| Input::PointerDown {
                    at: local(pos),
                    button,
                })
            } else {
                Some(Input::PointerUp {
                    at: local(pos),
                    button,
                })
            }
        }
        egui::Event::MouseWheel { unit, delta, .. } => {
            let lines = match unit {
                egui::MouseWheelUnit::Line => delta.y,
                egui::MouseWheelUnit::Point => delta.y / 40.0,
                egui::MouseWheelUnit::Page => delta.y * 10.0,
            };
            Some(Input::Wheel {
                at: local(rect.center()),
                lines,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers, MouseWheelUnit, PointerButton, Pos2, Rect, Vec2, pos2};

    fn rect() -> Rect {
        Rect::from_min_size(pos2(100.0, 50.0), egui::vec2(200.0, 100.0))
    }

    fn button(pos: Pos2, b: PointerButton, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: b,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    fn wheel(unit: MouseWheelUnit, y: f32) -> Event {
        Event::MouseWheel {
            unit,
            delta: Vec2::new(0.0, y),
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn move_is_relative_to_the_rect() {
        let got = to_input(&Event::PointerMoved(pos2(110.0, 80.0)), rect());
        assert_eq!(
            got,
            Some(Input::PointerMoved {
                at: Point::new(10.0, 30.0)
            })
        );
    }

    #[test]
    fn move_outside_is_still_forwarded() {
        let got = to_input(&Event::PointerMoved(pos2(0.0, 0.0)), rect());
        assert_eq!(
            got,
            Some(Input::PointerMoved {
                at: Point::new(-100.0, -50.0)
            })
        );
    }

    #[test]
    fn press_inside_only() {
        let at = pos2(150.0, 60.0);
        assert_eq!(
            to_input(&button(at, PointerButton::Primary, true), rect()),
            Some(Input::PointerDown {
                at: Point::new(50.0, 10.0),
                button: Button::Primary
            })
        );
        assert_eq!(
            to_input(
                &button(pos2(5.0, 5.0), PointerButton::Primary, true),
                rect()
            ),
            None
        );
        assert!(matches!(
            to_input(&button(at, PointerButton::Secondary, true), rect()),
            Some(Input::PointerDown {
                button: Button::Secondary,
                ..
            })
        ));
        assert!(matches!(
            to_input(&button(at, PointerButton::Middle, true), rect()),
            Some(Input::PointerDown {
                button: Button::Middle,
                ..
            })
        ));
        assert_eq!(
            to_input(&button(at, PointerButton::Extra1, true), rect()),
            None
        );
    }

    #[test]
    fn release_inside_and_outside() {
        for (at, rel) in [
            (pos2(150.0, 60.0), Point::new(50.0, 10.0)),
            (pos2(5.0, 5.0), Point::new(-95.0, -45.0)),
        ] {
            assert_eq!(
                to_input(&button(at, PointerButton::Primary, false), rect()),
                Some(Input::PointerUp {
                    at: rel,
                    button: Button::Primary
                })
            );
        }
    }

    #[test]
    fn wheel_units() {
        let lines = |unit, y| match to_input(&wheel(unit, y), rect()) {
            Some(Input::Wheel { lines, at }) => {
                assert_eq!(at, Point::new(100.0, 50.0));
                lines
            }
            other => panic!("{other:?}"),
        };
        assert_eq!(lines(MouseWheelUnit::Line, 2.0), 2.0);
        assert_eq!(lines(MouseWheelUnit::Point, 80.0), 2.0);
        assert_eq!(lines(MouseWheelUnit::Page, 0.5), 5.0);
    }

    #[test]
    fn pointer_gone_and_other_events() {
        assert_eq!(
            to_input(&Event::PointerGone, rect()),
            Some(Input::PointerLeft)
        );
        assert_eq!(to_input(&Event::Copy, rect()), None);
    }
}
