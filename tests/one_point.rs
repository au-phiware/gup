// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-700: `Point` is one type, the geometry point, wherever it is named.

use gup::prelude::*;

fn same<T>(_: &T, _: &T) {}

#[test]
fn point_is_the_geometry_point_from_every_path() {
    let from_prelude: Point = Point::new(1.0, 2.0);
    let from_root = gup::Point::new(1.0, 2.0);
    let from_geom = gup::geom::Point::new(1.0, 2.0);
    same(&from_prelude, &from_root);
    same(&from_root, &from_geom);
    assert_eq!(from_prelude, from_geom);
    // What `Input` takes, with only the prelude imported.
    let _ = Input::pointer_moved(from_prelude);
}

#[test]
fn the_point_scale_is_point_scale() {
    let scale = PointScale::new().padding(0.5);
    let _: &gup::scale::PointScale = &scale;
}
