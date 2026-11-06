// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

use gup_core::geom::Point;
use gup_core::{Button, Input, Key, Modifiers};

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

/// The `Input` for an egui key press (`pressed: true`, repeats included);
/// `None` for a release or a key the chart does not use.
///
/// Arrows, Home, End, Enter, Space and Escape carry the modifiers as held;
/// `+`/`=`, `-` and `0` become `ZoomIn`, `ZoomOut` and `ResetView` with
/// Shift dropped (`+` needs Shift on some layouts only).
pub fn key_input(event: &egui::Event) -> Option<Input> {
    let egui::Event::Key {
        key,
        pressed: true,
        modifiers: m,
        ..
    } = event
    else {
        return None;
    };
    let held = Modifiers {
        shift: m.shift,
        ctrl: m.ctrl,
        alt: m.alt,
        meta: m.mac_cmd,
    };
    let (key, held) = match key {
        egui::Key::ArrowLeft => (Key::Left, held),
        egui::Key::ArrowRight => (Key::Right, held),
        egui::Key::ArrowUp => (Key::Up, held),
        egui::Key::ArrowDown => (Key::Down, held),
        egui::Key::Home => (Key::Home, held),
        egui::Key::End => (Key::End, held),
        egui::Key::Enter | egui::Key::Space => (Key::Enter, held),
        egui::Key::Escape => (Key::Escape, held),
        egui::Key::Plus | egui::Key::Equals => (
            Key::ZoomIn,
            Modifiers {
                shift: false,
                ..held
            },
        ),
        egui::Key::Minus => (
            Key::ZoomOut,
            Modifiers {
                shift: false,
                ..held
            },
        ),
        egui::Key::Num0 => (
            Key::ResetView,
            Modifiers {
                shift: false,
                ..held
            },
        ),
        _ => return None,
    };
    Some(Input::key_with(key, held))
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

    fn key(k: egui::Key, pressed: bool, modifiers: Modifiers) -> Event {
        Event::Key {
            key: k,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn key_input_maps_every_row() {
        use gup_core::{Input, Key as K, Modifiers as M};
        let cases = [
            (egui::Key::ArrowLeft, K::Left),
            (egui::Key::ArrowRight, K::Right),
            (egui::Key::ArrowUp, K::Up),
            (egui::Key::ArrowDown, K::Down),
            (egui::Key::Home, K::Home),
            (egui::Key::End, K::End),
            (egui::Key::Enter, K::Enter),
            (egui::Key::Space, K::Enter),
            (egui::Key::Escape, K::Escape),
            (egui::Key::Plus, K::ZoomIn),
            (egui::Key::Equals, K::ZoomIn),
            (egui::Key::Minus, K::ZoomOut),
            (egui::Key::Num0, K::ResetView),
        ];
        for (k, want) in cases {
            assert_eq!(
                key_input(&key(k, true, Modifiers::NONE)),
                Some(Input::key_with(want, M::NONE)),
                "{k:?}"
            );
        }
    }

    #[test]
    fn key_input_modifiers_and_nones() {
        use gup_core::{Input, Key as K, Modifiers as M};
        let shift = Modifiers::SHIFT;
        assert_eq!(
            key_input(&key(egui::Key::ArrowRight, true, shift)),
            Some(Input::key_with(K::Right, M::SHIFT))
        );
        assert_eq!(
            key_input(&key(egui::Key::Plus, true, shift)),
            Some(Input::key_with(K::ZoomIn, M::NONE))
        );
        let ctrl = Modifiers::CTRL;
        let got = key_input(&key(egui::Key::Num0, true, ctrl)).unwrap();
        assert!(matches!(got, Input::KeyDown { modifiers, .. } if modifiers.ctrl));
        // A release, and a key the chart does not use.
        assert_eq!(
            key_input(&key(egui::Key::ArrowLeft, false, Modifiers::NONE)),
            None
        );
        assert_eq!(key_input(&key(egui::Key::A, true, Modifiers::NONE)), None);
        assert_eq!(key_input(&key(egui::Key::Tab, true, Modifiers::NONE)), None);
    }
}
