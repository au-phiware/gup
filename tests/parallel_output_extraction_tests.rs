// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for ParallelOutput buffer extraction utilities (GUP-140 AC1).

use gup::prelude::*;

#[test]
fn test_extract_first() {
    let parallel_buffer = vec![
        ParallelOutput {
            first: [0.0_f32, 1.0],
            second: [1.0_f32, 0.0, 0.0, 1.0],
        },
        ParallelOutput {
            first: [1.0, 0.0],
            second: [0.0, 1.0, 0.0, 1.0],
        },
        ParallelOutput {
            first: [0.5, 0.5],
            second: [0.5, 0.5, 0.5, 1.0],
        },
    ];

    let first_buffer = parallel_output_extraction::extract_first(&parallel_buffer);

    assert_eq!(first_buffer.len(), 3);
    assert_eq!(first_buffer[0], [0.0, 1.0]);
    assert_eq!(first_buffer[1], [1.0, 0.0]);
    assert_eq!(first_buffer[2], [0.5, 0.5]);
}

#[test]
fn test_extract_second() {
    let parallel_buffer = vec![
        ParallelOutput {
            first: [0.0_f32, 1.0],
            second: [1.0_f32, 0.0, 0.0, 1.0],
        },
        ParallelOutput {
            first: [1.0, 0.0],
            second: [0.0, 1.0, 0.0, 1.0],
        },
        ParallelOutput {
            first: [0.5, 0.5],
            second: [0.5, 0.5, 0.5, 1.0],
        },
    ];

    let second_buffer = parallel_output_extraction::extract_second(&parallel_buffer);

    assert_eq!(second_buffer.len(), 3);
    assert_eq!(second_buffer[0], [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(second_buffer[1], [0.0, 1.0, 0.0, 1.0]);
    assert_eq!(second_buffer[2], [0.5, 0.5, 0.5, 1.0]);
}

#[test]
fn test_split_parallel_buffer() {
    let parallel_buffer = vec![
        ParallelOutput {
            first: [0.0_f32, 1.0],
            second: [1.0_f32, 0.0, 0.0, 1.0],
        },
        ParallelOutput {
            first: [1.0, 0.0],
            second: [0.0, 1.0, 0.0, 1.0],
        },
        ParallelOutput {
            first: [0.5, 0.5],
            second: [0.5, 0.5, 0.5, 1.0],
        },
    ];

    let (first_buffer, second_buffer) =
        parallel_output_extraction::split_parallel_buffer(&parallel_buffer);

    assert_eq!(first_buffer.len(), 3);
    assert_eq!(second_buffer.len(), 3);

    assert_eq!(first_buffer[0], [0.0, 1.0]);
    assert_eq!(first_buffer[1], [1.0, 0.0]);
    assert_eq!(first_buffer[2], [0.5, 0.5]);

    assert_eq!(second_buffer[0], [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(second_buffer[1], [0.0, 1.0, 0.0, 1.0]);
    assert_eq!(second_buffer[2], [0.5, 0.5, 0.5, 1.0]);
}

#[test]
fn test_split_parallel_buffer_empty() {
    let parallel_buffer: Vec<ParallelOutput<[f32; 2], [f32; 4]>> = vec![];
    let (first_buffer, second_buffer) =
        parallel_output_extraction::split_parallel_buffer(&parallel_buffer);

    assert_eq!(first_buffer.len(), 0);
    assert_eq!(second_buffer.len(), 0);
}

#[test]
fn test_memory_alignment() {
    // Test that extraction correctly handles memory alignment
    // Vec2 needs 8-byte alignment, Vec4 needs 16-byte alignment
    use std::mem::{align_of, size_of};

    type TestOutput = ParallelOutput<[f32; 2], [f32; 4]>;

    // Verify the ParallelOutput struct has correct size and alignment
    assert!(size_of::<TestOutput>() >= size_of::<[f32; 2]>() + size_of::<[f32; 4]>());
    assert!(align_of::<TestOutput>() >= align_of::<[f32; 2]>().max(align_of::<[f32; 4]>()));

    // Create test data
    let parallel_buffer = vec![
        ParallelOutput {
            first: [1.0, 2.0],
            second: [3.0, 4.0, 5.0, 6.0],
        },
        ParallelOutput {
            first: [7.0, 8.0],
            second: [9.0, 10.0, 11.0, 12.0],
        },
    ];

    // Extract and verify values are preserved correctly
    let (first_buffer, second_buffer) =
        parallel_output_extraction::split_parallel_buffer(&parallel_buffer);

    assert_eq!(first_buffer[0], [1.0, 2.0]);
    assert_eq!(first_buffer[1], [7.0, 8.0]);
    assert_eq!(second_buffer[0], [3.0, 4.0, 5.0, 6.0]);
    assert_eq!(second_buffer[1], [9.0, 10.0, 11.0, 12.0]);
}

#[test]
fn test_nested_parallel_output() {
    // Test with nested ParallelOutput (for 3-way and 4-way composition)
    type NestedOutput = ParallelOutput<ParallelOutput<[f32; 2], [f32; 4]>, f32>;

    let nested_buffer = vec![
        NestedOutput {
            first: ParallelOutput {
                first: [0.0, 1.0],
                second: [1.0, 0.0, 0.0, 1.0],
            },
            second: 5.0,
        },
        NestedOutput {
            first: ParallelOutput {
                first: [1.0, 0.0],
                second: [0.0, 1.0, 0.0, 1.0],
            },
            second: 10.0,
        },
    ];

    // Extract first component (which is itself a ParallelOutput)
    let first_buffer = parallel_output_extraction::extract_first(&nested_buffer);
    assert_eq!(first_buffer.len(), 2);
    assert_eq!(first_buffer[0].first, [0.0, 1.0]);
    assert_eq!(first_buffer[0].second, [1.0, 0.0, 0.0, 1.0]);

    // Extract second component (size values)
    let second_buffer = parallel_output_extraction::extract_second(&nested_buffer);
    assert_eq!(second_buffer.len(), 2);
    assert_eq!(second_buffer[0], 5.0);
    assert_eq!(second_buffer[1], 10.0);
}
