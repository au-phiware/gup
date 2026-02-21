// Performance validation tests for advanced mark system

use gup::mark::*;
use std::time::Instant;

#[test]
fn test_path_mark_vertex_generation_performance() {
    let start = Instant::now();
    for _ in 0..10000 {
        let _vertices = Path::generate_vertices();
    }
    let duration = start.elapsed();
    
    // Should complete 10K generations in under 10ms
    assert!(
        duration.as_millis() < 10,
        "Path vertex generation too slow: {:?}",
        duration
    );
}

#[test]
fn test_composite_mark_vertex_generation_performance() {
    let start = Instant::now();
    for _ in 0..10000 {
        let _vertices = CompositeMark::generate_vertices();
    }
    let duration = start.elapsed();
    
    assert!(
        duration.as_millis() < 10,
        "CompositeMark vertex generation too slow: {:?}",
        duration
    );
}

#[test]
fn test_text_mark_vertex_generation_performance() {
    let start = Instant::now();
    for _ in 0..10000 {
        let _vertices = Text::generate_vertices();
    }
    let duration = start.elapsed();
    
    assert!(
        duration.as_millis() < 10,
        "Text vertex generation too slow: {:?}",
        duration
    );
}

#[test]
fn test_mark_vertex_memory_efficiency() {
    use std::mem::size_of;
    
    // Verify vertex types are memory-efficient
    assert_eq!(size_of::<PathVertex>(), 16);  // 2 vec2<f32>
    assert_eq!(size_of::<CompositeMarkVertex>(), 8);  // 1 vec2<f32>
    assert_eq!(size_of::<TextVertex>(), 16);  // 2 vec2<f32>
}

#[test]
fn test_transform_to_matrix_performance() {
    let transform = Transform::identity();
    
    let start = Instant::now();
    for _ in 0..100000 {
        let _matrix = transform.to_matrix();
    }
    let duration = start.elapsed();
    
    // 100K transforms should complete in under 5ms
    assert!(
        duration.as_millis() < 5,
        "Transform to matrix too slow: {:?}",
        duration
    );
}
