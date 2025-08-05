// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GPU spatial indexing compute shader for hierarchical hit testing
// Implements a grid-based spatial index for efficient region queries

struct SpatialCell {
    element_count: u32,
    element_start_index: u32,
    bounds_min: vec2<f32>,
    bounds_max: vec2<f32>,
}

struct SpatialIndex {
    grid_size: vec2<u32>,      // Number of cells in X and Y
    cell_size: vec2<f32>,      // Size of each cell in world units
    world_bounds_min: vec2<f32>, // Minimum world coordinates
    world_bounds_max: vec2<f32>, // Maximum world coordinates
}

struct ElementData {
    position: vec2<f32>,
    size: vec2<f32>,
    mark_type: u32,
    element_id: u32,
    selection_id: u32,
    _padding: u32,
}

@group(0) @binding(0) var<storage, read> elements: array<ElementData>;
@group(0) @binding(1) var<storage, read_write> spatial_cells: array<SpatialCell>;
@group(0) @binding(2) var<storage, read_write> element_indices: array<u32>;
@group(0) @binding(3) var<uniform> spatial_index: SpatialIndex;

// Build spatial index from element data
@compute @workgroup_size(256)
fn build_spatial_index(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let element_index = global_id.x;
    
    if (element_index >= arrayLength(&elements)) {
        return;
    }
    
    let element = elements[element_index];
    
    // Calculate which cell this element belongs to
    let cell_pos = world_to_cell(element.position);
    let cell_index = cell_pos.y * spatial_index.grid_size.x + cell_pos.x;
    
    if (cell_index >= arrayLength(&spatial_cells)) {
        return;
    }
    
    // Increment element count for this cell (simplified approach without atomics)
    spatial_cells[cell_index].element_count = spatial_cells[cell_index].element_count + 1u;
}

// Compute prefix sum for element start indices
@compute @workgroup_size(256)
fn compute_cell_offsets(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let cell_index = global_id.x;
    
    if (cell_index >= arrayLength(&spatial_cells)) {
        return;
    }
    
    if (cell_index == 0u) {
        spatial_cells[0].element_start_index = 0u;
    } else {
        spatial_cells[cell_index].element_start_index = 
            spatial_cells[cell_index - 1].element_start_index + 
            spatial_cells[cell_index - 1].element_count;
    }
}

// Populate element indices in sorted order
@compute @workgroup_size(256)
fn populate_element_indices(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let element_index = global_id.x;
    
    if (element_index >= arrayLength(&elements)) {
        return;
    }
    
    let element = elements[element_index];
    let cell_pos = world_to_cell(element.position);
    let cell_index = cell_pos.y * spatial_index.grid_size.x + cell_pos.x;
    
    if (cell_index >= arrayLength(&spatial_cells)) {
        return;
    }
    
    // Find insertion position (simplified approach without atomics)
    let insertion_index = spatial_cells[cell_index].element_start_index;
    spatial_cells[cell_index].element_start_index = spatial_cells[cell_index].element_start_index + 1u;
    
    if (insertion_index < arrayLength(&element_indices)) {
        element_indices[insertion_index] = element_index;
    }
}

// Spatial query using the index
@compute @workgroup_size(256)
fn spatial_query(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // Implementation for spatial queries using the built index
    // This would be called instead of the brute-force hit test
}

// Helper function: convert world position to cell coordinates
fn world_to_cell(world_pos: vec2<f32>) -> vec2<u32> {
    let normalized_pos = (world_pos - spatial_index.world_bounds_min) / 
                        (spatial_index.world_bounds_max - spatial_index.world_bounds_min);
    let cell_pos = normalized_pos * vec2<f32>(spatial_index.grid_size);
    return vec2<u32>(
        clamp(u32(cell_pos.x), 0u, spatial_index.grid_size.x - 1u),
        clamp(u32(cell_pos.y), 0u, spatial_index.grid_size.y - 1u)
    );
}

// Helper function: get cells overlapping with a query region
fn get_overlapping_cells(region_min: vec2<f32>, region_max: vec2<f32>) -> vec4<u32> {
    let min_cell = world_to_cell(region_min);
    let max_cell = world_to_cell(region_max);
    return vec4<u32>(min_cell.x, min_cell.y, max_cell.x, max_cell.y);
}