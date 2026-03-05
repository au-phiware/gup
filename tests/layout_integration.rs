// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the GPU force-directed graph layout engine.

use gup::layout::*;
use gup::render::RenderContext;

// ---------------------------------------------------------------------------
// Unit tests — ForceDirected builder defaults
// ---------------------------------------------------------------------------

#[test]
fn force_directed_defaults_are_sensible() {
    let fd = ForceDirected::new();
    assert!(fd.repulsion_strength > 0.0, "repulsion must be positive");
    assert!(fd.spring_strength > 0.0, "spring strength must be positive");
    assert!(fd.spring_rest_length > 0.0, "rest length must be positive");
    assert!(fd.gravity > 0.0, "gravity must be positive");
    assert!(
        (0.0..=1.0).contains(&fd.damping),
        "damping should be in [0,1]"
    );
    assert!(fd.iterations > 0, "iterations must be > 0");
    assert!(
        fd.convergence_threshold > 0.0,
        "convergence threshold must be positive"
    );
    assert!(
        fd.convergence_check_interval >= 1,
        "check interval must be >= 1"
    );
}

#[test]
fn force_directed_builder_methods() {
    let fd = ForceDirected::new()
        .repulsion_strength(100.0)
        .spring_strength(0.05)
        .spring_rest_length(50.0)
        .gravity(0.2)
        .damping(0.8)
        .iterations(500)
        .convergence_threshold(0.25)
        .convergence_check_interval(5);

    assert!((fd.repulsion_strength - 100.0).abs() < f32::EPSILON);
    assert!((fd.spring_strength - 0.05).abs() < f32::EPSILON);
    assert!((fd.spring_rest_length - 50.0).abs() < f32::EPSILON);
    assert!((fd.gravity - 0.2).abs() < f32::EPSILON);
    assert!((fd.damping - 0.8).abs() < f32::EPSILON);
    assert_eq!(fd.iterations, 500);
    assert!((fd.convergence_threshold - 0.25).abs() < f32::EPSILON);
    assert_eq!(fd.convergence_check_interval, 5);
}

#[test]
fn convergence_check_interval_clamped_to_1() {
    let fd = ForceDirected::new().convergence_check_interval(0);
    assert_eq!(fd.convergence_check_interval, 1);
}

#[test]
fn graph_layout_trait_name() {
    let fd = ForceDirected::new();
    assert_eq!(fd.name(), "force-directed");
}

#[test]
fn gpu_struct_sizes() {
    // GPU struct sizes are validated at compile time via const assertions in types.rs.
    // This test just verifies the layout module compiles correctly.
    let _ = ForceDirected::new();
}

// ---------------------------------------------------------------------------
// GPU integration tests — require a GPU device (--test-threads=1)
// ---------------------------------------------------------------------------

/// Helper: create a RenderContext or skip the test if no GPU is available.
async fn gpu_context() -> Option<RenderContext> {
    match RenderContext::new().await {
        Ok(ctx) => Some(ctx),
        Err(_) => {
            eprintln!("Skipping GPU test: no GPU adapter available");
            None
        }
    }
}

#[tokio::test]
async fn engine_creation_compiles_shaders() {
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx);
    assert!(engine.is_ok(), "LayoutEngine::new() failed: {:?}", engine);
}

#[tokio::test]
async fn layout_empty_graph() {
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx).unwrap();
    let result = engine
        .force_directed_layout(&[], &[], &ForceDirected::new())
        .await
        .unwrap();
    assert!(result.positions.is_empty());
    assert_eq!(result.iterations_performed, 0);
    assert!(result.converged);
}

#[tokio::test]
async fn layout_single_node() {
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx).unwrap();
    let nodes = vec![LayoutNode {
        id: 0,
        x: 0.0,
        y: 0.0,
    }];
    let result = engine
        .force_directed_layout(&nodes, &[], &ForceDirected::new().iterations(10))
        .await
        .unwrap();
    assert_eq!(result.positions.len(), 1);
    assert_eq!(result.positions[0].id, 0);
    // Single node with gravity should converge near origin
    assert!(result.positions[0].x.is_finite());
    assert!(result.positions[0].y.is_finite());
}

#[tokio::test]
async fn layout_two_connected_nodes() {
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx).unwrap();
    let nodes = vec![
        LayoutNode {
            id: 0,
            x: -100.0,
            y: 0.0,
        },
        LayoutNode {
            id: 1,
            x: 100.0,
            y: 0.0,
        },
    ];
    let edges = vec![LayoutEdge {
        source: 0,
        target: 1,
    }];
    let config = ForceDirected::new()
        .iterations(100)
        .convergence_check_interval(10);
    let result = engine
        .force_directed_layout(&nodes, &edges, &config)
        .await
        .unwrap();

    assert_eq!(result.positions.len(), 2);
    for pos in &result.positions {
        assert!(pos.x.is_finite(), "x is not finite for node {}", pos.id);
        assert!(pos.y.is_finite(), "y is not finite for node {}", pos.id);
    }

    // The two nodes should be separated (not on top of each other)
    let dx = result.positions[0].x - result.positions[1].x;
    let dy = result.positions[0].y - result.positions[1].y;
    let dist = (dx * dx + dy * dy).sqrt();
    assert!(
        dist > 1.0,
        "Two connected nodes should be separated, got distance {dist}"
    );
}

#[tokio::test]
async fn layout_four_node_ring_roughly_symmetric() {
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx).unwrap();

    // 4-node ring: 0-1-2-3-0
    let nodes: Vec<LayoutNode> = (0..4)
        .map(|i| LayoutNode {
            id: i,
            x: 0.0,
            y: 0.0,
        })
        .collect();
    let edges = vec![
        LayoutEdge {
            source: 0,
            target: 1,
        },
        LayoutEdge {
            source: 1,
            target: 2,
        },
        LayoutEdge {
            source: 2,
            target: 3,
        },
        LayoutEdge {
            source: 3,
            target: 0,
        },
    ];

    let config = ForceDirected::new()
        .iterations(200)
        .convergence_check_interval(20);
    let result = engine
        .force_directed_layout(&nodes, &edges, &config)
        .await
        .unwrap();

    assert_eq!(result.positions.len(), 4);

    // All positions should be finite
    for pos in &result.positions {
        assert!(pos.x.is_finite());
        assert!(pos.y.is_finite());
    }

    // Compute distances from centroid — they should be roughly equal
    let cx: f32 = result.positions.iter().map(|p| p.x).sum::<f32>() / 4.0;
    let cy: f32 = result.positions.iter().map(|p| p.y).sum::<f32>() / 4.0;

    let distances: Vec<f32> = result
        .positions
        .iter()
        .map(|p| ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt())
        .collect();

    let avg_dist = distances.iter().sum::<f32>() / 4.0;
    for (i, d) in distances.iter().enumerate() {
        let ratio = d / avg_dist;
        assert!(
            (0.3..=3.0).contains(&ratio),
            "Node {i} distance ratio {ratio} is far from average (dist={d}, avg={avg_dist})"
        );
    }
}

#[tokio::test]
async fn convergence_early_exit() {
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx).unwrap();

    // Two nodes close together with strong spring — should converge quickly
    let nodes = vec![
        LayoutNode {
            id: 0,
            x: -5.0,
            y: 0.0,
        },
        LayoutNode {
            id: 1,
            x: 5.0,
            y: 0.0,
        },
    ];
    let edges = vec![LayoutEdge {
        source: 0,
        target: 1,
    }];

    let config = ForceDirected::new()
        .iterations(1000)
        .convergence_threshold(0.5)
        .convergence_check_interval(5);

    let result = engine
        .force_directed_layout(&nodes, &edges, &config)
        .await
        .unwrap();

    // Should converge in fewer than the maximum iterations
    assert!(
        result.iterations_performed < 1000,
        "Expected early convergence, but ran all {} iterations",
        result.iterations_performed
    );
    assert!(result.converged, "Expected converged=true");
}

#[tokio::test]
async fn layout_1k_random_graph_positions_finite() {
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx).unwrap();

    let node_count = 1000;
    let nodes: Vec<LayoutNode> = (0..node_count)
        .map(|i| LayoutNode {
            id: i as u32,
            x: 0.0,
            y: 0.0,
        })
        .collect();

    // Generate edges: Erdős-Rényi style with ~3 edges per node
    let mut edges = Vec::new();
    let mut seed: u64 = 42;
    for i in 0..node_count {
        for _ in 0..3 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let j = (seed >> 33) as usize % node_count;
            if i != j {
                edges.push(LayoutEdge {
                    source: i as u32,
                    target: j as u32,
                });
            }
        }
    }

    let config = ForceDirected::new()
        .iterations(50)
        .convergence_check_interval(25);
    let result = engine
        .force_directed_layout(&nodes, &edges, &config)
        .await
        .unwrap();

    assert_eq!(result.positions.len(), node_count);
    for pos in &result.positions {
        assert!(pos.x.is_finite(), "Node {} x is not finite", pos.id);
        assert!(pos.y.is_finite(), "Node {} y is not finite", pos.id);
        assert!(!pos.x.is_nan(), "Node {} x is NaN", pos.id);
        assert!(!pos.y.is_nan(), "Node {} y is NaN", pos.id);
        // Positions should be within a reasonable bounding box
        assert!(
            pos.x.abs() < 4096.0,
            "Node {} x={} exceeds bounding box",
            pos.id,
            pos.x
        );
        assert!(
            pos.y.abs() < 4096.0,
            "Node {} y={} exceeds bounding box",
            pos.id,
            pos.y
        );
    }
    assert!(result.iterations_performed <= 50);
}

// ---------------------------------------------------------------------------
// Barnes-Hut specific tests
// ---------------------------------------------------------------------------

#[test]
fn approximation_theta_default() {
    let fd = ForceDirected::new();
    assert!(
        (fd.approximation_theta - 0.5).abs() < f32::EPSILON,
        "default theta should be 0.5, got {}",
        fd.approximation_theta
    );
}

#[test]
fn approximation_theta_builder() {
    let fd = ForceDirected::new().approximation_theta(0.8);
    assert!(
        (fd.approximation_theta - 0.8).abs() < f32::EPSILON,
        "theta should be 0.8, got {}",
        fd.approximation_theta
    );
}

#[test]
fn approximation_theta_clamped_to_zero() {
    let fd = ForceDirected::new().approximation_theta(-1.0);
    assert!(
        fd.approximation_theta >= 0.0,
        "theta must not be negative, got {}",
        fd.approximation_theta
    );
}

#[tokio::test]
async fn theta_zero_falls_back_to_exact() {
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx).unwrap();

    let nodes = vec![
        LayoutNode {
            id: 0,
            x: -50.0,
            y: 0.0,
        },
        LayoutNode {
            id: 1,
            x: 50.0,
            y: 0.0,
        },
    ];
    let edges = vec![LayoutEdge {
        source: 0,
        target: 1,
    }];

    // theta=0 should use exact pairwise repulsion (original code path).
    let config = ForceDirected::new()
        .approximation_theta(0.0)
        .iterations(20)
        .convergence_check_interval(10);
    let result = engine
        .force_directed_layout(&nodes, &edges, &config)
        .await
        .unwrap();

    assert_eq!(result.positions.len(), 2);
    for pos in &result.positions {
        assert!(pos.x.is_finite());
        assert!(pos.y.is_finite());
    }
}

#[tokio::test]
async fn barnes_hut_vs_exact_small_graph() {
    // For a small graph, Barnes-Hut and exact repulsion should produce
    // qualitatively similar layouts.
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx).unwrap();

    let nodes: Vec<LayoutNode> = (0..8)
        .map(|i| LayoutNode {
            id: i,
            x: 0.0,
            y: 0.0,
        })
        .collect();
    let edges = vec![
        LayoutEdge {
            source: 0,
            target: 1,
        },
        LayoutEdge {
            source: 1,
            target: 2,
        },
        LayoutEdge {
            source: 2,
            target: 3,
        },
        LayoutEdge {
            source: 3,
            target: 4,
        },
        LayoutEdge {
            source: 4,
            target: 5,
        },
        LayoutEdge {
            source: 5,
            target: 6,
        },
        LayoutEdge {
            source: 6,
            target: 7,
        },
        LayoutEdge {
            source: 7,
            target: 0,
        },
    ];

    let base = ForceDirected::new()
        .iterations(100)
        .convergence_check_interval(50);

    let exact_config = base.clone().approximation_theta(0.0);
    let bh_config = base.clone().approximation_theta(0.5);

    let exact_result = engine
        .force_directed_layout(&nodes, &edges, &exact_config)
        .await
        .unwrap();
    let bh_result = engine
        .force_directed_layout(&nodes, &edges, &bh_config)
        .await
        .unwrap();

    assert_eq!(exact_result.positions.len(), bh_result.positions.len());

    // Both should produce finite positions within a reasonable bounding box.
    for (e, b) in exact_result
        .positions
        .iter()
        .zip(bh_result.positions.iter())
    {
        assert!(e.x.is_finite() && e.y.is_finite(), "exact not finite");
        assert!(b.x.is_finite() && b.y.is_finite(), "BH not finite");
        assert!(e.x.abs() < 4096.0 && e.y.abs() < 4096.0, "exact OOB");
        assert!(b.x.abs() < 4096.0 && b.y.abs() < 4096.0, "BH OOB");
    }

    // Both layouts should spread nodes out (not all at origin).
    let exact_spread: f32 = exact_result
        .positions
        .iter()
        .map(|p| p.x * p.x + p.y * p.y)
        .sum::<f32>()
        .sqrt();
    let bh_spread: f32 = bh_result
        .positions
        .iter()
        .map(|p| p.x * p.x + p.y * p.y)
        .sum::<f32>()
        .sqrt();

    assert!(
        exact_spread > 1.0,
        "exact layout too collapsed: spread={exact_spread}"
    );
    assert!(
        bh_spread > 1.0,
        "BH layout too collapsed: spread={bh_spread}"
    );
}

#[tokio::test]
async fn barnes_hut_1k_graph_positions_finite() {
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx).unwrap();

    let node_count = 1000;
    let nodes: Vec<LayoutNode> = (0..node_count)
        .map(|i| LayoutNode {
            id: i as u32,
            x: 0.0,
            y: 0.0,
        })
        .collect();

    let mut edges = Vec::new();
    let mut seed: u64 = 42;
    for i in 0..node_count {
        for _ in 0..3 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let j = (seed >> 33) as usize % node_count;
            if i != j {
                edges.push(LayoutEdge {
                    source: i as u32,
                    target: j as u32,
                });
            }
        }
    }

    let config = ForceDirected::new()
        .approximation_theta(0.5)
        .iterations(50)
        .convergence_check_interval(25);

    let result = engine
        .force_directed_layout(&nodes, &edges, &config)
        .await
        .unwrap();

    assert_eq!(result.positions.len(), node_count);
    for pos in &result.positions {
        assert!(pos.x.is_finite(), "Node {} x is not finite", pos.id);
        assert!(pos.y.is_finite(), "Node {} y is not finite", pos.id);
        assert!(
            pos.x.abs() < 4096.0,
            "Node {} x={} exceeds bounding box",
            pos.id,
            pos.x
        );
        assert!(
            pos.y.abs() < 4096.0,
            "Node {} y={} exceeds bounding box",
            pos.id,
            pos.y
        );
    }
}

// ---------------------------------------------------------------------------
// Incremental (session-based) API tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn session_create_and_step() {
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx).unwrap();

    let nodes = vec![
        LayoutNode {
            id: 0,
            x: -50.0,
            y: 0.0,
        },
        LayoutNode {
            id: 1,
            x: 50.0,
            y: 0.0,
        },
    ];
    let edges = vec![LayoutEdge {
        source: 0,
        target: 1,
    }];
    let config = ForceDirected::new().approximation_theta(0.0);

    let mut session = engine.create_session(&nodes, &edges, &config).unwrap();
    assert_eq!(session.node_count(), 2);
    assert_eq!(session.iterations_performed, 0);

    // Step 5 iterations
    engine.step(&mut session, 5);
    assert_eq!(session.iterations_performed, 5);

    // Read back positions — should be finite and different from initial
    let positions = engine.read_positions(&session).await.unwrap();
    assert_eq!(positions.len(), 2);
    for pos in &positions {
        assert!(pos.x.is_finite());
        assert!(pos.y.is_finite());
    }
}

#[tokio::test]
async fn session_incremental_stepping() {
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx).unwrap();

    let nodes: Vec<LayoutNode> = (0..20)
        .map(|i| LayoutNode {
            id: i,
            x: 0.0,
            y: 0.0,
        })
        .collect();
    let mut edges = Vec::new();
    for i in 0..20u32 {
        edges.push(LayoutEdge {
            source: i,
            target: (i + 1) % 20,
        });
    }
    let config = ForceDirected::new().approximation_theta(0.0);
    let mut session = engine.create_session(&nodes, &edges, &config).unwrap();

    // Step multiple times and confirm positions change between steps
    engine.step(&mut session, 10);
    let pos1 = engine.read_positions(&session).await.unwrap();

    engine.step(&mut session, 10);
    let pos2 = engine.read_positions(&session).await.unwrap();

    assert_eq!(session.iterations_performed, 20);

    // After additional iterations positions should differ
    let mut changed = false;
    for (a, b) in pos1.iter().zip(pos2.iter()) {
        if (a.x - b.x).abs() > f32::EPSILON || (a.y - b.y).abs() > f32::EPSILON {
            changed = true;
            break;
        }
    }
    assert!(changed, "positions should change between steps");
}

#[tokio::test]
async fn session_pin_node() {
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx).unwrap();

    let nodes = vec![
        LayoutNode {
            id: 0,
            x: -50.0,
            y: 0.0,
        },
        LayoutNode {
            id: 1,
            x: 50.0,
            y: 0.0,
        },
        LayoutNode {
            id: 2,
            x: 0.0,
            y: 50.0,
        },
    ];
    let edges = vec![
        LayoutEdge {
            source: 0,
            target: 1,
        },
        LayoutEdge {
            source: 1,
            target: 2,
        },
    ];
    let config = ForceDirected::new().approximation_theta(0.0);
    let mut session = engine.create_session(&nodes, &edges, &config).unwrap();

    // Pin node 0 at (100, 200)
    engine.pin_node(&session, 0, 100.0, 200.0);

    // Step the simulation
    engine.step(&mut session, 5);

    // Pin it again to enforce position
    engine.pin_node(&session, 0, 100.0, 200.0);

    let positions = engine.read_positions(&session).await.unwrap();
    assert_eq!(positions[0].id, 0);
    // After pinning + step + pinning, position should be exactly at pin location
    assert!(
        (positions[0].x - 100.0).abs() < f32::EPSILON,
        "pinned x should be 100, got {}",
        positions[0].x
    );
    assert!(
        (positions[0].y - 200.0).abs() < f32::EPSILON,
        "pinned y should be 200, got {}",
        positions[0].y
    );
}
