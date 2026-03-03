# Stories

> Stories are the unit of work. Dependencies, not phases, determine order.
> Status: ✅ Complete · 🚧 In Progress · 📋 Planned

## GPU Rendering Pipeline

Core GPU abstractions: buffer management, render context, blend state, surface
and WebGPU integration.

- [GUP-003](GUP-003_GPU_Buffer_Management.md) ✅ — GPU buffer management is
  fundamental to Gup's performance.
- [GUP-004](GUP-004_Basic_Render_Context.md) ✅ — The render context
  (GupContext) provides the foundation for all GPU operations in Gup. Deps:
  GUP-003 ✅.
- [GUP-013](GUP-013_Event_Handling_System.md) ✅ — Implements the high-level
  event handling API connecting GPU interaction results (hit tests, picks) with
  .on(event, handler) patterns on Selection<T, M>. Deps: GUP-002 ✅, GUP-012 ✅.
- [GUP-013](GUP-013_GPU_Shader_Position_Precision_Fix.md) ✅ — GPU Shader
  Position Precision Fix.
- [GUP-017](GUP-017_Error_Handling_Framework.md) ✅ — A robust error handling
  framework is essential for Gup's reliability and developer experience. Deps:
  GUP-001 ✅, GUP-003 ✅, GUP-004 ✅.
- [GUP-020](GUP-020_WebGPU_Integration_RenderContext.md) ✅ — The current
  RenderContext contains placeholder WebGPU resources (Option<wgpu::Device>)
  that…. Deps: GUP-001 ✅.
- [GUP-027](GUP-027_GPU_Blend_State_Integration.md) ✅ — GUP-021 introduced the
  BlendMode enum and placeholder methods on RenderContext, but these are…. Deps:
  GUP-020 ✅, GUP-021 ✅.
- [GUP-030](GUP-030_GPU_Buffer_Pool_Management.md) ✅ — During GUP-002, we
  discovered that creating new GPU buffers for every resize operation is….
- [GUP-035](GUP-035_Advanced_Buffer_Download_System.md) ✅ — During GUP-003, we
  implemented a comprehensive GPU buffer upload system but discovered that….
  Deps: GUP-003 ✅.
- [GUP-036](GUP-036_Buffer_Pool_Performance_Optimization.md) ✅ — GUP-003
  implemented a functional buffer pool with 100% efficiency in basic scenarios.
  Deps: GUP-003 ✅.
- [GUP-038](GUP-038_Texture_Pool_Enhancement.md) ✅ — The current TexturePool in
  GupContext is a placeholder implementation that only creates new textures.
- [GUP-039](GUP-039_Context_Window_Integration.md) ✅ — While GUP-004
  implemented basic surface support, real applications need more sophisticated….
- [GUP-040](GUP-040_Advanced_Blend_State_Optimization.md) ✅ — Following
  GUP-027, the blend state integration works well but there are opportunities
  for….
- [GUP-042](GUP-042_Cross_Platform_Surface_Features.md) ✅ — GUP-039 provides
  basic cross-platform surface management, but advanced applications need….
- [GUP-043](GUP-043_Visual_Blend_Mode_Demonstration.md) ✅ — Visual Blend Mode
  Demonstration ✅ COMPLETED.
- [GUP-044](GUP-044_GPU_Test_Resource_Management.md) ✅ — GPU Test Resource
  Management.
- [GUP-045](GUP-045_RAII_State_Management_System.md) ✅ — RAII State Management
  System.
- [GUP-047](GUP-047_Surface_Event_Integration.md) ✅ — While GUP-039 implemented
  basic multi-surface management, real applications need integrated….
- [GUP-048](GUP-048_Context_Error_Recovery.md) ✅ — GPU contexts can fail due to
  device loss, driver issues, or system resource exhaustion.
- [GUP-049](GUP-049_Surface_Performance_Optimization.md) ✅ — While GUP-039
  meets basic performance requirements (<16ms resize), advanced multi-window….
- [GUP-050](GUP-050_Visual_Blend_Mode_Validation.md) ✅ — GUP-027 implemented
  GPU blend state integration with functional tests, but lacks visual
  validation.
- [GUP-079](GUP-079_GPU_Memory_Pool_Optimization.md) ✅ — GPU Memory Pool
  Optimization.
- [GUP-085](GUP-085_GPU_Resource_Dependency_Graph.md) ✅ — During GUP-034
  implementation, identified the need to visualize relationships between GPU….
- [GUP-102](GUP-102_Demo_GPU_Resource_Management_Fixes.md) ✅ — Demo GPU
  Resource Management Fixes.
- [GUP-132](GUP-132_GPU_Path_Tessellation.md) ✅ — Current Path mark
  implementation tessellates paths on CPU and uploads triangles to GPU.
- [GUP-134](GUP-134_Storage_Buffer_ColorGradient.md) ✅ — GUP-033 implemented
  ColorGradient with a limitation of 8 color stops using uniform buffers.
- [GUP-135](GUP-135_Fix_Examples_Compilation.md) ✅ — Multiple examples have
  outdated ShaderFunction implementations that fail to compile:.
- [GUP-149](GUP-149_Automatic_Device_Loss_Detection.md) ✅ — Currently,
  applications must manually call mark_device_lost() when GPU operations fail.
- [GUP-149](GUP-149_Box_Plot_GPU_Rendering.md) ✅ — GUP-147 implemented the
  BoxPlot mark type and statistical computation layer, but deferred full GPU
  rendering integration.
- [GUP-207](GUP-207_Fix_Preexisting_Doctest_Failures.md) ✅ — Fix Pre-existing
  Doctest Failures.
- [GUP-238](GUP-238_Remaining_Send_Sync_Audit.md) ✅ — Audit and migrate
  remaining Send + Sync trait bounds throughout the codebase to use the….

## Composition System

The `Mixable` trait and all composition modes, error recovery, async streaming,
and data source merging.

- [GUP-001](GUP-001_Build_Mixable_Trait.md) ✅ — The Mixable trait is the
  foundation of Gup's universal composability system.
- [GUP-019](GUP-019_Mixable_Performance_Validation.md) ✅ — The current Mixable
  trait benchmarks (from GUP-001) test trivial operations (sub-100ns) where….
  Deps: GUP-001 ✅.
- [GUP-021](GUP-021_Advanced_Composition_Mode_Implementation.md) ✅ — The
  current Mixable trait implementation treats all composition modes (Overlay,
  Merge…. Deps: GUP-001 ✅, GUP-020 ✅.
- [GUP-022](GUP-022_Deep_Composition_Chain_Optimization.md) ✅ — While the basic
  Mixable trait supports composition chaining (e.g., a.mix(b).mix(c).mix(d))….
  Deps: GUP-001 ✅, GUP-020 ✅.
- [GUP-023](GUP-023_Mixable_Trait_Ecosystem_Integration.md) ✅ — The current
  Mixable trait requires manual implementation for each visualization type,
  limiting…. Deps: GUP-001 ✅, GUP-020 ✅.
- [GUP-024](GUP-024_Composition_Error_Recovery_Diagnostics.md) ✅ — The current
  Mixable trait system provides basic error propagation but lacks
  sophisticated…. Deps: GUP-001 ✅, GUP-020 ✅.
- [GUP-025](GUP-025_Async_Streaming_Composition_Support.md) ✅ — The current
  Mixable trait system operates synchronously and assumes all data is available
  at composition time. Deps: GUP-001 ✅, GUP-020 ✅.
- [GUP-026](GUP-026_Data_Source_Merge_Implementation.md) ✅ — Currently, the
  Merge composition mode in GUP-021 uses a placeholder implementation that
  simply…. Deps: GUP-021 ✅.
- [GUP-028](GUP-028_Composition_Performance_Optimization.md) ✅ — Current
  composition implementation in GUP-021 recalculates viewport splits and blend
  states on every render. Deps: GUP-021 ✅, GUP-027 ✅.
- [GUP-136](GUP-136_Parallel_Composition_Implementation.md) ✅ — GUP-033 defined
  the ParallelComposition API pattern but deferred implementation due to
  complex….

## Selection API

The `Selection` type, attribute binding pipeline, pool integration, GPU-resident
data, and parallel output.

- [GUP-002](GUP-002_Core_Selection_Type.md) ✅ — The Selection<T, M> type is the
  heart of Gup's composability system, directly inspired by D3.js selections.
  Deps: GUP-001 ✅, GUP-003 ✅, GUP-004 ✅.
- [GUP-140](GUP-140_Selection_API_Parallel_Output.md) ✅ — GUP-136 implemented
  the core ParallelComposition functionality, enabling shader functions that….
- [GUP-140](GUP-140_Storage_Buffer_Keyframes.md) ✅ — GUP-138 implemented
  KeyframeAnimation with up to 16 keyframes using uniform buffers.
- [GUP-165](GUP-165_Selection_API_Render_Integration.md) ✅ — The Selection API
  (GUP-002) was built around data binding and event handling.
- [GUP-167](GUP-167_GpuBufferPool_Selection_Integration.md) ✅ — GUP-165
  (Selection API Render Integration) created instance buffers via
  device.create_buffer_init() directly.
- [GUP-168](GUP-168_Selection_Attribute_Binding_Pipeline.md) ✅ — GUP-165 added
  Selection::prepare_render(device, queue, mapper) which requires the caller
  to….
- [GUP-169](GUP-169_Shared_Pipeline_Cache_Selections.md) ✅ — GUP-165 has each
  Selection create its own render pipeline via
  MarkInfoImpl::create_render_pipeline().
- [GUP-183](GUP-183_Pooled_GPU_Instance_Filter_Buffers.md) ✅ — Pre-allocate and
  reuse GPU buffers for the ComputeInstanceFilter across frames, eliminating….
- [GUP-186](GUP-186_Dynamic_Attribute_GPU_Upload_Pipeline.md) ✅ — Build the
  complete GPU upload pipeline for DynamicAttributeMap, including automatic
  buffer….
- [GUP-192](GUP-192_Dynamic_Attribute_Readback_Pipeline.md) ✅ — Add async
  GPU-to-CPU readback for dynamic attribute buffers managed by
  DynamicAttributeBufferManager.
- [GUP-193](GUP-193_GPU_Resident_Candidate_Pipeline.md) ✅ — Eliminate the
  GPU→CPU→GPU readback in the Morton query pipeline by keeping candidate
  element….
- [GUP-194](GUP-194_GPU_Resident_Selection_Data_Cache.md) ✅ — Pre-upload and
  cache mark positions and sizes on the GPU so that hit testing queries avoid
  the….
- [GUP-195](GUP-195_Bind_Group_Caching_Pooled_Filter.md) ✅ — Cache the wgpu
  bind group in PooledComputeInstanceFilter when the input buffer does not
  change between dispatches.
- [GUP-197](GUP-197_Result_Buffer_Readback_Optimization.md) ✅ — The
  download_results() method in InteractionSystem creates a new staging buffer on
  every query….
- [GUP-198](GUP-198_Non_Blocking_Query_API.md) ✅ — The current hit test query
  API (query_point_cached, query_region_cached, etc.) blocks the….
- [GUP-221](GUP-221_Pool_Aware_SetData_Selection.md) ✅ — GUP-167 integrated
  BufferPool into Selection::prepare_render(), but Selection::set_data()….
- [GUP-276](GUP-276_D3_Style_Data_Transitions.md) ✅ — D3-style
  enter/update/exit transitions via Selection::data_keyed() with
  GPU-interpolated 2-keyframe animations for smooth data rebinding. Deps:
  GUP-002 ✅, GUP-138 ✅, GUP-141 ✅, GUP-142 ✅, GUP-168 ✅.
- [GUP-277](GUP-277_GPU_Render_Loop_Transition_Integration.md) 📋 — Wire
  CommittedTransition into the GPU render loop with KeyframeAnimation instances
  for automatic per-frame interpolation. Deps: GUP-276 ✅.
- [GUP-278](GUP-278_Staggered_Transition_Delays.md) 📋 — Per-element delay
  offsets via `.delay_fn()` for cascading/staggered animation effects. Deps:
  GUP-276 ✅.

## Shader Function System

The `ShaderFunction` trait, WGSL macro, pipeline builder, type safety, AST
integration, and Rust→WGSL transpilation.

- [GUP-005](GUP-005_Shader_Function_Trait.md) ✅ — The ShaderFunction trait is
  Gup's core innovation - it treats all data transformations as…. Deps: GUP-003
  ✅, GUP-004 ✅.
- [GUP-006](GUP-006_WGSL_Function_Macro.md) ✅ — The #[wgsl_function] macro is
  the key developer experience feature that makes Gup accessible. Deps: GUP-005
  ✅.
- [GUP-007](GUP-007_Shader_Pipeline_Builder.md) ✅ — The ShaderPipeline is
  responsible for taking composed shader functions and generating…. Deps:
  GUP-003 ✅, GUP-005 ✅, GUP-006 ✅.
- [GUP-008](GUP-008_Type_System_Integration.md) ✅ — Type system integration
  ensures that Rust's compile-time type checking validates shader function
  composition. Deps: GUP-005 ✅.
- [GUP-029](GUP-029_WGSL_Shader_Code_Generation.md) ✅ — During GUP-002, we
  implemented a shader function system with placeholder WGSL code generation.
- [GUP-033](GUP-033_Shader_Function_Composition_Engine.md) ✅ — GUP-002
  implemented basic PositionShaderFunction and ColorShaderFunction types.
- [GUP-051](GUP-051_WGSL_Code_Generation_Templates.md) ✅ — While GUP-005
  established the foundation for composable shader functions, the WGSL code….
- [GUP-052](GUP-052_Shader_Pipeline_Builder.md) ✅ — With the shader function
  composition system from GUP-005 and WGSL code generation from….
- [GUP-053](GUP-053_Advanced_Shader_Function_Library.md) ✅ — GUP-005
  implemented basic shader functions (LinearScale, ColorMap, PositionTransform).
- [GUP-252](GUP-252_LinearScale_GPU_Shader_Function.md) ✅ — LinearScaleUniforms
  and WGSL linear_scale/linear_scale_invert shader functions with clamping and
  inversion support. Deps: GUP-005 ✅, GUP-007 ✅, GUP-053 ✅.
- [GUP-253](GUP-253_LogScale_GPU_Shader_Function.md) ✅ — LogScale
  ShaderFunction with configurable base, zero/epsilon guard, and symmetric-log
  mode for data straddling zero. Deps: GUP-252 ✅.
- [GUP-254](GUP-254_OrdinalScale_GPU_Shader_Function.md) ✅ — OrdinalScale
  (BandScale + PointScale) GPU shader functions mapping integer category indices
  to positions. Deps: GUP-252 ✅, GUP-053 ✅.
- [GUP-255](GUP-255_ColorScale_GPU_Shader_Function.md) ✅ — ColorScale
  ShaderFunction (f32 → vec4) with built-in palettes (Viridis, Plasma, etc.)
  composable with LinearScale. Deps: GUP-134 ✅, GUP-252 ✅.
- [GUP-273](GUP-273_Geographic_Projection_Shader_System.md) ✅ — Mercator,
  Equirectangular, Stereographic, and Orthographic projections as composable
  WGSL ShaderFunctions with GeoPoint coordinate type and boundary clipping.
  Deps: GUP-005 ✅, GUP-007 ✅, GUP-052 ✅, GUP-053 ✅.
- [GUP-293](GUP-293_LAB_OKLab_Color_Space_Shader_Functions.md) 📋 — LAB/OKLab
  perceptual color space conversions as composable shader functions. Deps:
  GUP-053 ✅.
- [GUP-294](GUP-294_Shader_Function_Module_Reorganization.md) 📋 — Split
  shader_function.rs (7700+ lines) into category-based submodules. Deps: GUP-053
  ✅.
- [GUP-053](GUP-053_Shader_Pipeline_Performance_Optimization.md) ✅ — Enhance
  the ComposableShaderPipeline system with advanced performance optimizations
  based on….
- [GUP-054](GUP-054_Existing_Solutions_Analysis.md) ✅ — Before implementing our
  own Rust-to-WGSL transpilation system, we need to thoroughly….
- [GUP-054](GUP-054_Shader_Function_Performance_Optimization.md) ✅ — During
  GUP-005 implementation, we achieved the target of <100ms for 1000 compositions
  (~15ns average).
- [GUP-054](GUP-054_Shader_Function_Type_Safety_Enhancement.md) ✅ — Enhance the
  shader function system with improved type safety and automatic type inference
  to….
- [GUP-055](GUP-055_Rust_AST_Parsing_Research.md) ✅ — The current string-based
  WGSL template system provides functional dynamic code generation but….
- [GUP-056](GUP-056_Type_System_Mapping.md) ✅ — Building on the research from
  GUP-055, we need to implement a robust type system that can….
- [GUP-057](GUP-057_Expression_Transpilation.md) ✅ — With the type system
  mapping established in GUP-056, we need to implement comprehensive….
- [GUP-058](GUP-058_Control_Flow_Handling.md) ✅ — Building on expression
  transpilation from GUP-057, we need to implement control flow….
- [GUP-059](GUP-059_Built_in_Function_Library.md) ✅ — With expression
  transpilation and control flow established, we need to create a
  comprehensive….
- [GUP-060](GUP-060_Optimization_Error_Reporting.md) ✅ — As the final piece of
  the Rust-to-WGSL transpilation system, we need to implement optimization….
- [GUP-061](GUP-061_Integration_With_Shader_Function_System.md) ✅ — The
  Rust-to-WGSL transpilation system (GUP-054 through GUP-060) builds a new path
  for writing….
- [GUP-062](GUP-062_Community_Validation_Prototyping.md) ✅ — Building on the
  research from GUP-054 and technical architecture from GUP-055, we need to….
- [GUP-064](GUP-064-B_Custom_Struct_Code_Generation.md) ✅ — Custom Struct Code
  Generation for WGSL.
- [GUP-065](GUP-065_Documentation_Macro_First_API.md) ✅ — Following GUP-008's
  implementation of the macro-based type construction system, the…. Deps:
  GUP-008 ✅.
- [GUP-066](GUP-066_Advanced_Type_Conversion_Patterns.md) ✅ — During GUP-008
  implementation, the need for more sophisticated type conversion patterns
  became apparent. Deps: GUP-008 ✅.
- [GUP-131](GUP-131_Shader_Type_Constructors.md) ✅ — During GUP-032
  implementation, discovered that shader types like Vec2, Vec3, Vec4, Mat2,
  Mat3….
- [GUP-177](GUP-177_GPU_Shader_Function_Attribute_Binding.md) ✅ — GUP-168
  implemented CPU-side attribute binding where closures extract values from data
  items….
- [GUP-179](GUP-179_Shader_Function_Uniform_Live_Update.md) ✅ — GUP-177
  implemented GPU-side shader function attribute bindings where raw data values
  are….
- [GUP-180](GUP-180_FunctionChain_Binding_Support.md) ✅ — GUP-177's
  attr_shader() accepts any ComposableShaderFunction.
- [GUP-189](GUP-189_AST_Integration_ComposableShaderPipeline.md) ✅ — Wire the
  AST-based optimizer from shader_ast into the existing….
- [GUP-190](GUP-190_WGSL_Compute_Shader_AST_Support.md) ✅ — Extend the
  shader_ast parser and AST types to handle WGSL compute shader constructs
  including….
- [GUP-191](GUP-191_Enable_AST_Optimization_Default.md) ✅ — Once the WGSL
  parser in shader_ast can handle the full range of generated shader constructs
  —….
- [GUP-210](GUP-210_Switch_Statement_Transpilation.md) ✅ — GUP-058 implemented
  control flow transpilation but explicitly excluded match expression….
- [GUP-211](GUP-211_Fix_Preexisting_wgsl_function_Test.md) ✅ — The test
  wgsl_function::tests::test_is_uniform_compatible_type in
  gup-macros/src/wgsl_function.rs:1256 has been failing.
- [GUP-212](GUP-212_WGSL_Reserved_Keyword_Detection.md) ✅ — During GUP-062
  validation, it was discovered that the #[shader_fn] transpiler allows Rust….
- [GUP-213](GUP-213_Transpiler_Custom_Struct_Support.md) ✅ — The
  current #[shader_fn] transpiler supports primitive types, vectors, and
  matrices as function parameters.
- [GUP-218](GUP-218_Duplicate_Struct_Definition_Prevention.md) ✅ — When
  multiple shader function bindings share component types (e.g., two different….
- [GUP-219](GUP-219_Deep_Chain_Binding_Support.md) ✅ — GUP-180 validated
  two-level function chains (A.compose(B)) with attr_shader().
- [GUP-220](GUP-220_Mixed_Chain_Attribute_Deduplication.md) ✅ — GUP-218
  introduced name-based WGSL struct deduplication: when multiple attr_shader()
  bindings….

## Mark System

Mark trait, built-in mark types (circle, rect, line, path), instanced rendering,
derive macros, and custom mark kit.

- [GUP-009](GUP-009_Core_Mark_Trait.md) ✅ — The Mark trait defines the
  interface that all visual primitives implement. Deps: GUP-003 ✅, GUP-005 ✅,
  GUP-007 ✅.
- [GUP-010](GUP-010_Basic_Mark_Implementations.md) ✅ — The core visual marks
  (Circle, Rectangle, Line) are the fundamental building blocks for all data
  visualizations. Deps: GUP-003 ✅, GUP-004 ✅, GUP-009 ✅.
- [GUP-011](GUP-011_Mark_Shader_Integration.md) ✅ — The mark-shader integration
  bridges visual primitives with the shader function system…. Deps: GUP-005 ✅,
  GUP-007 ✅, GUP-009 ✅, GUP-010 ✅.
- [GUP-032](GUP-032_Advanced_Mark_System.md) ✅ — GUP-002 implemented a basic
  Circle mark for proof-of-concept.
- [GUP-067](GUP-067_Rectangle_Line_Mark_Implementations.md) ✅ — Following the
  successful implementation of the core Mark trait system in GUP-009, we need
  to…. Deps: GUP-009 ✅.
- [GUP-068](GUP-068_Mark_Pipeline_Integration.md) ✅ — While GUP-009 implemented
  the core Mark trait system, the create_render_pipeline method in…. Deps:
  GUP-003 ✅, GUP-007 ✅, GUP-009 ✅.
- [GUP-072](GUP-072_Mark_System_Documentation.md) ✅ — This guide walks through
  implementing custom marks, from simple geometric shapes to complex….
- [GUP-073](GUP-073_Advanced_Shader_Composition.md) ✅ — Advanced Shader
  Composition.
- [GUP-074](GUP-074_Mark_Performance_Optimization.md) ✅ — Mark Performance
  Optimization.
- [GUP-130](GUP-130_Mark_Type_ID_Proc_Macro.md) ✅ — GUP-128 fixed mark type ID
  generation by using type name string matching (e.g., checking if type name
  contains "Circle").
- [GUP-178](GUP-178_MarkInstanceBuilder_Line_BoxPlot.md) ✅ — GUP-168
  implemented MarkInstanceBuilder for Circle and Rectangle marks, enabling
  declarative….
- [GUP-185](GUP-185_Multi_Pass_Mark_Examples.md) ✅ — Create example marks that
  use multi-pass rendering to validate the multi-pass API with visual output.
- [GUP-208](GUP-208_Mark_Derive_Instance_Buffer_Generation.md) ✅ — Mark Derive
  Macro GPU Instance Buffer Generation.
- [GUP-274](GUP-274_Map_Mark_Rendering.md) ✅ — GeoPathMark that loads GeoJSON
  boundaries, tessellates polygons via GUP-132, and renders them with a
  geographic projection shader. Deps: GUP-009 ✅, GUP-132 ✅, GUP-273 ✅.
- [GUP-285](GUP-285_High_Resolution_GeoJSON_Streaming.md) 📋 — Background/async
  GeoJSON streaming for large (10–100 MB) boundary datasets without blocking the
  render thread. Deps: GUP-274 ✅.
- [GUP-286](GUP-286_Spherical_Polygon_Simplification.md) 📋 — Spherical-aware
  polygon simplification using great-circle distance for polar-region accuracy.
  Deps: GUP-274 ✅.

## Interaction & Spatial Index

GPU hit testing, spatial index, interaction events, touch support, lasso
selection, and radix sort.

- [GUP-012](GUP-012_GPU_Interaction_System.md) ✅ — The interaction system must
  handle hit testing, picking, and event handling for massive datasets using GPU
  acceleration. Deps: GUP-002 ✅, GUP-003 ✅, GUP-009 ✅, GUP-010 ✅.
- [GUP-014](GUP-014_Interaction_Performance_Optimization.md) ✅ — Interaction
  Performance Optimization.
- [GUP-014](GUP-014_Performance_Validation.md) 📋 — This story validates that
  Phase 1 achieves all performance targets and optimizes any…. Deps: GUP-001 ✅,
  GUP-013 ✅.
- [GUP-031](GUP-031_GPU_Interaction_Event_System.md) ✅ — GUP-002 implemented
  placeholder event handling with InteractionEvent types.
- [GUP-075](GUP-075_Interactive_Mark_Selection.md) ✅ — Interactive Mark
  Selection.
- [GUP-076](GUP-076_GPU_Occlusion_Culling.md) ✅ — Implement
  compute-shader-based occlusion culling using a hierarchical Z-buffer for dense
  point….
- [GUP-076](GUP-076_Spatial_Index_Bind_Group_Layout_Fix.md) ✅ — Spatial Index
  Bind Group Layout Fix.
- [GUP-077](GUP-077_Compute_Shader_Instance_Filtering.md) ✅ — Move instance
  culling, LOD classification, and Z-order sorting to GPU compute shaders for….
- [GUP-077](GUP-077_Performance_Benchmarking_Suite.md) ✅ — Performance
  Benchmarking Suite.
- [GUP-078](GUP-078_Spatial_Index_Algorithm_Optimization.md) ✅ — Spatial Index
  Algorithm Optimization.
- [GUP-128](GUP-128_Debug_GPU_Hit_Test_Detection.md) ✅ — During GUP-031
  implementation, 3 interaction system tests started failing with GPU hit
  tests….
- [GUP-175](GUP-175_GPU_Side_Morton_Range_Query.md) ✅ — Implement Morton-based
  spatial query entirely on GPU using sorted buffers and binary search in….
- [GUP-176](GUP-176_Spatial_Index_Adaptive_Grid_Size.md) ✅ — The basic grid
  spatial index uses a fixed 100×100 grid regardless of dataset size.
- [GUP-181](GUP-181_GPU_Selection_Hit_Testing.md) ✅ — Integrate the
  MarkSelectionSystem from GUP-075 with the GPU-based InteractionSystem from….
- [GUP-182](GUP-182_Touch_Selection_Support.md) ✅ — Add touch gesture support
  to the mark selection system, enabling interactive mark selection on….
- [GUP-184](GUP-184_GPU_Radix_Sort_Z_Order.md) ✅ — Implement a parallel GPU
  radix sort pass in the compute shader instance filtering pipeline to….
- [GUP-196](GUP-196_Hit_Test_Result_Buffer_Query_Count.md) ✅ — Pass the actual
  query count (not the buffer capacity) to the hit test compute shader via a….
- [GUP-222](GUP-222_Unified_Frustum_Occlusion_Pipeline.md) ✅ — Combine the
  existing ComputeInstanceFilter (frustum culling, LOD, prefix-sum, compaction)
  with….
- [GUP-223](GUP-223_Coarse_HiZ_Early_Reject.md) ✅ — The current occlusion test
  (GUP-076) always operates at Hi-Z level 0 (finest resolution) for correctness.
- [GUP-234](GUP-234_Adaptive_Build_Coverage_Budget.md) ✅ — The build_coverage
  pass in the occlusion culling shader has a fixed 4096-cell limit per….
- [GUP-234](GUP-234_Touch_Lasso_Selection.md) ✅ — Extend the
  TouchSelectionAdapter to support lasso (free-form) selection via touch
  gestures.
- [GUP-235](GUP-235_Radix_Sort_Scatter_Optimization.md) ✅ — Optimize the
  scatter pass in the GPU radix sort to replace the O(workgroup_size²) serial
  local….
- [GUP-236](GUP-236_Sort_Aware_Visual_Demo.md) ✅ — Create an example
  application demonstrating transparent overlapping marks rendered with and….
- [GUP-277](GUP-277_Zoom_Pan_Interactions.md) ✅ — ZoomBehavior with GPU
  ViewportTransform uniform, inertia panning, zoom-to-cursor, and configurable
  scale limits. Deps: GUP-012 ✅, GUP-013 ✅.
- [GUP-278](GUP-278_Brush_Mark_Rectangular_Selection.md) ✅ — BrushMark overlay
  with drag-to-select, GPU region query, visual feedback, and viewport-aware
  coordinate transform. Deps: GUP-012 ✅, GUP-067 ✅, GUP-075 ✅, GUP-013 ✅.
- [GUP-279](GUP-279_Linked_View_Coordination.md) ✅ — SharedSelectionState
  coordinating brush/click selections across multiple charts with opacity-based
  visual dimming. Deps: GUP-001 ✅, GUP-075 ✅, GUP-013 ✅, GUP-278 ✅.
- [GUP-283](GUP-283_Event_Coalescing.md) 📋 — Frame-rate-aware coalescing for
  high-frequency mousemove/touchmove events in EventManager. Deps: GUP-013 ✅.
- [GUP-284](GUP-284_Unified_Vec2_Type.md) 📋 — Unify interaction::Vec2,
  shader_function::Vec2, and [f32; 2] into a single ergonomic type with
  arithmetic ops.
- [GUP-285](GUP-285_BrushMark_GPU_Overlay_Rendering.md) 📋 — Wire BrushMark
  screen_rect into the chart render loop as a visible RectangleInstance overlay.
  Deps: GUP-278 ✅, GUP-067 ✅.
- [GUP-286](GUP-286_GPU_Accelerated_Brush_Region_Query.md) 📋 — Replace CPU
  filter_by_rect with GPU rect_hit_test_gpu for brush selection on 500K+ mark
  datasets. Deps: GUP-278 ✅, GUP-012 ✅, GUP-075 ✅.
- [GUP-287](GUP-287_LinkedSelection_Wrapper_Type.md) ✅ — Wrap Selection +
  SharedSelectionState + key_fn into a single LinkedSelection type with
  automatic generation-based rebuild. Deps: GUP-279 ✅.
- [GUP-288](GUP-288_GPU_Selection_Mask_Buffer.md) ✅ — GPU-side selection mask
  buffer + compute shader for alpha dimming at 100K+ scale without CPU rebuild.
  Deps: GUP-279 ✅, GUP-003 ✅.
- [GUP-289](GUP-289_LinkedSelection_GPU_Integration.md) ✅ — Wire
  SelectionMaskBuffer into LinkedSelection for automatic GPU dimming above a
  configurable threshold. Deps: GUP-288 ✅, GUP-279 ✅.
- [GUP-290](GUP-290_GPU_Mask_Buffer_Pool_Integration.md) ✅ — Integrate
  SelectionMaskBuffer with BufferPool for GPU buffer reuse. Deps: GUP-288 ✅,
  GUP-003 ✅.
- [GUP-291](GUP-291_Adaptive_GPU_Dimming_Threshold.md) ✅ — Auto-tune the
  CPU/GPU dimming threshold based on runtime profiling. Deps: GUP-289 ✅.
- [GUP-292](GUP-292_GPU_Timestamp_Query_Profiling.md) 📋 — Use GPU timestamp
  queries for more accurate auto-tune calibration. Deps: GUP-291 ✅.

## Axis & Grid System

Axis infrastructure, tick generation, grid rendering, scale integration, chart
builder axis/grid API.

- [GUP-089](GUP-089_Core_Axis_System_Infrastructure.md) ✅ — Core Axis System
  Infrastructure.
- [GUP-090](GUP-090_Automatic_Tick_Generation_Algorithm.md) ✅ — Automatic Tick
  Generation Algorithm.
- [GUP-091](GUP-091_Grid_Line_Rendering_System.md) ✅ — Grid Line Rendering
  System.
- [GUP-092](GUP-092_Label_Formatting_and_Positioning.md) ✅ — Label Formatting
  and Positioning.
- [GUP-093](GUP-093_Scale_Axis_Integration_System.md) ✅ — Scale-Axis
  Integration System.
- [GUP-094](GUP-094_Axis_Performance_Optimization.md) ✅ — Axis Performance
  Optimization.
- [GUP-095](GUP-095_Grid_Visual_Rendering_Integration.md) ✅ — Grid Visual
  Rendering Integration.
- [GUP-096](GUP-096_Grid_Performance_Benchmarking.md) ✅ — Grid Performance
  Benchmarking and Validation.
- [GUP-097](GUP-097_Chart_Builder_Grid_API_Enhancement.md) ✅ — Chart Builder
  Grid API Enhancement.
- [GUP-098](GUP-098_Grid_System_Documentation.md) ✅ — Grid System Comprehensive
  Documentation.
- [GUP-100](GUP-100_Visual_Chart_Axis_Integration.md) ✅ — Visual Chart Axis
  Integration.
- [GUP-204](GUP-204_GPU_Instance_Rendering_Axis_Ticks.md) ✅ — Replace the
  current per-tick vertex pair approach with GPU instancing for axis tick marks.
- [GUP-206](GUP-206_Cross_Platform_Axis_Performance_Validation.md) ✅ — Run the
  axis performance benchmarks introduced in GUP-094 on macOS, Windows, and
  WebAssembly….
- [GUP-216](GUP-216_Chart_Title_Layout_Configuration.md) ✅ — Add a dedicated
  TitleConfig struct to ChartConfig supporting title alignment, vertical
  offset….
- [GUP-217](GUP-217_Per_Axis_Label_Style_Override.md) ✅ — Allow individual axes
  to override the chart-level label_style from ChartConfig, enabling….
- [GUP-224](GUP-224_Chart_Builder_Instanced_Ticks.md) ✅ — Update
  ComposedChart::generate_axis_geometry() and generate_axis_geometry_resolved()
  to use….
- [GUP-225](GUP-225_Instanced_Grid_Line_Rendering.md) ✅ — Apply the GPU
  instancing pattern from GUP-204 (tick marks) to grid lines.
- [GUP-226](GUP-226_WebAssembly_Axis_Performance_Validation.md) ✅ — Run the
  cross-platform axis performance benchmarks (from GUP-206) in an actual….

## Text Rendering

GPU SDF text pipeline, font atlas, multi-font, text layout (wrapping, clipping,
ellipsis), tooltip background.

- [GUP-099](GUP-099_GPU_Text_Rendering_Pipeline.md) ✅ — GPU Text Rendering
  Pipeline Implementation.
- [GUP-101](GUP-101_Label_Collision_Detection_Enhancement.md) ✅ — Label
  Collision Detection Enhancement.
- [GUP-104](GUP-104_SDF_Glyph_Texture_Upload.md) ✅ — SDF Glyph Texture Upload
  Implementation.
- [GUP-105](GUP-105_Text_Clipping_Detection_and_Viewport_Bounds_Management.md)
  ✅ — Text Clipping Detection and Viewport Bounds Management.
- [GUP-106](GUP-106_System_Font_Loading.md) ✅ — Implement system font loading
  to support dynamic font selection by name instead of relying only on embedded
  fonts.
- [GUP-107](GUP-107_Text_Character_Positioning_Bug.md) ✅ — Text Character
  Positioning Bug.
- [GUP-199](GUP-199_Text_Wrapping_Multi_Line_Layout.md) ✅ — Text Wrapping and
  Multi-Line Layout.
- [GUP-200](GUP-200_Interactive_Clipping_Reveal.md) ✅ — Interactive Clipping
  Reveal.
- [GUP-201](GUP-201_Text_Clipping_Visual_Demo.md) ✅ — Text Clipping Visual
  Demo.
- [GUP-202](GUP-202_Font_Aware_Text_Rendering_Pipeline.md) ✅ — Connect the
  TextStyle.font_family field to actual font atlas creation in the text
  rendering….
- [GUP-203](GUP-203_Multi_Font_Atlas_Manager.md) ✅ — Implement a font atlas
  manager that maintains multiple FontAtlas instances for different fonts….
- [GUP-205](GUP-205_SDF_Text_Rendering_Performance_Tuning.md) ✅ — Tune the SDF
  (Signed Distance Field) shader parameters for optimal quality/performance
  balance….
- [GUP-214](GUP-214_Font_Atlas_Eviction.md) ✅ — Add LRU eviction and memory
  limits to FontAtlasManager to prevent unbounded GPU memory growth….
- [GUP-215](GUP-215_Chart_Builder_Multi_Font.md) ✅ — Update the chart builder
  layer (axes, titles, labels) to use FontAtlasManager so that….
- [GUP-228](GUP-228_Ellipsis_Last_Wrapped_Line.md) ✅ — Ellipsis on Last Wrapped
  Line.
- [GUP-229](GUP-229_Tooltip_Background_Rendering.md) ✅ — Tooltip Background
  Rendering.
- [GUP-230](GUP-230_Chart_Builder_Hover_Reveal_Integration.md) ✅ — Chart
  Builder Hover Reveal Integration.
- [GUP-241](GUP-241_Tooltip_Arrow_Pointer.md) ✅ — Tooltip Arrow/Pointer.
- [GUP-242](GUP-242_Shared_UI_Chrome_Renderer.md) ✅ — Shared UI Chrome
  Renderer.

## Accessibility

ARIA generation, platform accessibility APIs (macOS, Windows, Linux, Web),
screen reader testing, focus elements.

- [GUP-016](GUP-016_Core_Accessibility_System.md) ✅ — Core Accessibility
  System. Deps: GUP-002 ✅, GUP-012 ✅, GUP-013 ✅.
- [GUP-111](GUP-111_Automatic_ARIA_Generation.md) ✅ — GUP-016 implemented the
  core accessibility infrastructure including ARIA tree structures. Deps:
  GUP-002 ✅, GUP-016 ✅.
- [GUP-112](GUP-112_Platform_Accessibility_Integration.md) ✅ — GUP-016
  implemented platform-agnostic accessibility infrastructure (ARIA trees,
  keyboard navigation, contrast modes). Deps: GUP-016 ✅.
- [GUP-114](GUP-114_macOS_NSAccessibility_Integration.md) ✅ — GUP-112
  implemented the architecture for platform-specific accessibility bridges with
  a stub…. Deps: GUP-016 ✅, GUP-112 ✅.
- [GUP-115](GUP-115_Windows_UI_Automation_Integration.md) ✅ — GUP-112
  implemented the architecture for platform-specific accessibility bridges with
  a stub…. Deps: GUP-016 ✅, GUP-112 ✅.
- [GUP-116](GUP-116_Linux_AT-SPI2_Integration.md) ✅ — GUP-112 implemented the
  architecture for platform-specific accessibility bridges with a stub…. Deps:
  GUP-016 ✅, GUP-112 ✅.
- [GUP-117](GUP-117_Web_Accessibility_DOM_Overlay.md) ✅ — GUP-112 implemented
  basic Web ARIA support by creating hidden DOM elements with accessibility
  attributes. Deps: GUP-016 ✅, GUP-112 ✅.
- [GUP-118](GUP-118_Visualization_Position_Synchronization.md) ✅ — GUP-117
  created the DOM overlay structure with placeholder element positioning. Deps:
  GUP-117 ✅.
- [GUP-119](GUP-119_Interactive_Event_Forwarding.md) ✅ — GUP-117 created
  pointer event handlers that log events but don't forward them to the
  visualization system. Deps: GUP-012 ✅, GUP-117 ✅.
- [GUP-121](GUP-121_Screen_Reader_Manual_Testing.md) ✅ — GUP-117 implemented
  Web DOM overlay with ARIA support, but production deployment requires…. Deps:
  GUP-117 ✅.
- [GUP-122](GUP-122_Manual_Screen_Reader_Testing_Execution.md) ✅ — GUP-121
  created comprehensive screen reader testing infrastructure and documentation.
  Deps: GUP-117 ✅, GUP-121 ✅.
- [GUP-124](GUP-124_Enhanced_Color_Description.md) ✅ — GUP-111 implemented
  basic color description for accessible mark descriptions using simple RGB….
  Deps: GUP-111 ✅.
- [GUP-125](GUP-125_Automatic_ARIA_Registration.md) ✅ — GUP-111 implemented
  generate_aria_tree() method for Selections but requires manual registration
  with AccessibilitySystem. Deps: GUP-016 ✅, GUP-111 ✅.
- [GUP-126](GUP-126_Reactive_ARIA_Updates.md) ✅ — Currently, ARIA trees are
  generated once when generate_aria_tree() is called. Deps: GUP-111 ✅, GUP-125
  ✅.
- [GUP-127](GUP-127_Focus_Elements_for_Data_Points.md) ✅ — GUP-111 implemented
  ARIA tree generation, but screen reader users still cannot navigate…. Deps:
  GUP-016 ✅, GUP-111 ✅.
- [GUP-272](GUP-272_WCAG_2_1_AA_Compliance_Validation.md) 📋 — Systematic WCAG
  2.1 AA audit of all 50 success criteria, gap fixes, conformance statement, and
  automated accessibility checks in CI. Deps: GUP-016 ✅, GUP-111 ✅, GUP-112
  ✅, GUP-122 ✅, GUP-124 ✅, GUP-127 ✅.

## Animation & Streaming

Temporal animation, spline curves, keyframe storage buffers, animation events,
async streaming composition.

- [GUP-138](GUP-138_Advanced_Temporal_Animation.md) ✅ — GUP-033 implemented
  basic temporal interpolation and easing functions.
- [GUP-141](GUP-141_Spline_Animation_Curves.md) ✅ — GUP-138 implemented linear
  interpolation between keyframes.
- [GUP-142](GUP-142_Animation_Event_System.md) ✅ — GUP-138 implemented
  AnimationTimeline and keyframe animations.

## Statistical Visualization

Histogram, KDE, box plots, statistical marks, GPU statistics, streaming
aggregation.

- [GUP-139](GUP-139_Statistical_Shader_Functions.md) ✅ — GUP-033 implemented
  transformation and filtering functions but deferred statistical….
- [GUP-143](GUP-143_Histogram_Generation.md) ✅ — GUP-139 implemented basic
  statistical aggregations but deferred histogram generation.
- [GUP-144](GUP-144_Kernel_Density_Estimation.md) ✅ — GUP-139 provided basic
  statistical aggregations, but kernel density estimation (KDE) is needed….
- [GUP-145](GUP-145_GPU_Statistics_Integration_Tests.md) ✅ — GUP-139
  implemented GPU statistical compute infrastructure but included primarily
  CPU-side tests.
- [GUP-146](GUP-146_Streaming_Statistical_Aggregation.md) ✅ — GUP-139
  statistical functions are limited by GPU memory size.
- [GUP-147](GUP-147_Box_Plot_Visualization.md) ✅ — GUP-139 provides the
  statistical foundation (min, max, quartiles) needed for box plots.
- [GUP-147](GUP-147_GPU_Memory_Bandwidth_Profiling.md) 📋 — Adds actual GPU
  memory bandwidth measurement (upload/download throughput, texture access
  patterns, memory pressure detection) to the profiling system. Deps: GUP-046
  ✅, GUP-080 ✅.
- [GUP-148](GUP-148_Fix_Statistics_Shader_Bug.md) ✅ — GUP-145 discovered a
  critical bug in the statistics compute shader from GUP-139.
- [GUP-148](GUP-148_Profiling_Data_Export_Visualization.md) 📋 — Exports
  profiling data to JSON/CSV/Chrome trace, generates SVG flame graphs, and
  provides a live web dashboard. Deps: GUP-046 ✅, GUP-080 ✅, GUP-147 📋.
- [GUP-150](GUP-150_Recovery_Metrics_and_Analytics.md) ✅ — The error recovery
  system currently tracks individual recovery attempts but doesn't aggregate
  metrics over time.
- [GUP-150](GUP-150_Statistical_Mark_Builder_API.md) ✅ — Per the implementation
  strategy, high-level APIs are Phase 2 work.
- [GUP-151](GUP-151_Multi_Category_Box_Plots.md) ✅ — Box plots are often used
  to compare distributions across multiple categories (e.g., sales by….
- [GUP-151](GUP-151_Surface_Configuration_Caching.md) ✅ — After device
  recovery, surfaces must be recreated by the application.
- [GUP-166](GUP-166_Unified_BoxPlot_Mark_Renderer.md) ✅ — GUP-149 established
  the statistical computation layer and component-generation helpers for box
  plots.
- [GUP-170](GUP-170_BoxPlot_Notch_Rendering.md) ✅ — GUP-166 implemented a
  unified BoxPlot mark with SDF-based rendering.
- [GUP-171](GUP-171_BoxPlot_Pixel_Space_Strokes.md) ✅ — GUP-166's SDF box plot
  shader specifies stroke widths and outlier radii in clip-space units.

## Pattern Rendering

Procedural and texture-based patterns, mark pipeline integration, multi-mark
pattern support.

- [GUP-113](GUP-113_Pattern_Rendering.md) ✅ — GUP-016 implemented the pattern
  library infrastructure and ContrastMode::Pattern, but the…. Deps: GUP-016 ✅.
- [GUP-155](GUP-155_Mark_Pipeline_Pattern_Integration.md) ✅ — GUP-113
  implemented the pattern rendering infrastructure (PatternUniforms,
  PatternRenderer…. Deps: GUP-113 ✅.
- [GUP-156](GUP-156_Pattern_Performance_Benchmarking.md) ✅ — GUP-113
  implemented pattern rendering with a target of <5ms overhead. Deps: GUP-113
  ✅, GUP-119 ✅.
- [GUP-157](GUP-157_Multi_Mark_Pattern_Support.md) ✅ — GUP-113 created pattern
  infrastructure and GUP-119 integrated it with circles. Deps: GUP-113 ✅,
  GUP-119 ✅.
- [GUP-158](GUP-158_Path_Mark_Pattern_Support.md) ✅ — Path Mark Pattern
  Support. Deps: GUP-113 ✅, GUP-157 ✅.
- [GUP-159](GUP-159_Multi_Mark_Pattern_Visual_Example.md) ✅ — Pattern rendering
  has been implemented across all major mark types (Circle, Rectangle, Line….
  Deps: GUP-113 ✅, GUP-157 ✅.
- [GUP-160](GUP-160_Pattern_Visual_Regression_Tests.md) ✅ — Pattern rendering
  has functional tests but no visual validation. Deps: GUP-113 ✅, GUP-157 ✅.
- [GUP-163](GUP-163_Texture_Based_Pattern_Rendering.md) ✅ — GUP-113 chose
  procedural pattern generation in fragment shaders. Deps: GUP-113 ✅, GUP-156
  ✅.
- [GUP-164](GUP-164_Pattern_Rendering_Optimization.md) ✅ — GUP-156 created
  benchmarks to validate the <5ms overhead target for pattern rendering at 100K
  points. Deps: GUP-156 ✅, GUP-157 ✅.

## Chart Builders

Observable Plot-style chart builder API, migration guide, external library
integration, pipeline caching.

- [GUP-018](GUP-018_Observable_Plot_Chart_Builders.md) ✅ — This is Gup's
  primary developer-facing API that must achieve Observable Plot's legendary….
  Deps: GUP-001 ✅, GUP-002 ✅, GUP-005 ✅, GUP-009 ✅, GUP-010 ✅.
- [GUP-086](GUP-086_Observable_Plot_Migration_Guide.md) ✅ — GUP-018 implemented
  Observable Plot-style chart builders with excellent API alignment.
- [GUP-086](GUP-086_Web_Profiling_Dashboard.md) ✅ — GUP-034 implemented
  text-based visualization for GPU profiling.
- [GUP-087](GUP-087_Chart_Builder_Performance_Optimization.md) ✅ — Optimize the
  chart builder system (GUP-018) to eliminate remaining performance overhead….
  Deps: GUP-018 ✅.
- [GUP-088](GUP-088_External_Library_Integration_System.md) ✅ — External
  visualization libraries and custom data types need seamless integration with
  Gup's Mixable trait ecosystem. Deps: GUP-001 ✅, GUP-020 ✅.
- [GUP-103](GUP-103_Comprehensive_Chart_Examples_Suite.md) ✅ — Comprehensive
  Chart Examples Suite.
- [GUP-239](GUP-239_Pipeline_Caching_Chart_Builder.md) ✅ — The
  multi_font_chart_demo (and likely other chart rendering code) recreates the
  axis-line….
- [GUP-245](GUP-245_Bar_Chart_Builder.md) 📋 — BarChartBuilder with
  vertical/horizontal, grouped, and stacked variants using instanced Rectangle
  marks and OrdinalScale. Deps: GUP-018 ✅, GUP-067 ✅, GUP-093 ✅, GUP-254 ✅.
- [GUP-246](GUP-246_Line_Chart_Builder.md) 📋 — LineChartBuilder with
  multi-series support, automatic x-sorting, point markers, and four curve
  interpolation modes. Deps: GUP-018 ✅, GUP-067 ✅, GUP-093 ✅, GUP-168 ✅.
- [GUP-247](GUP-247_Area_Chart_Builder.md) 📋 — AreaChartBuilder with stacked,
  normalized-stacked, gradient-fill, and band/ribbon area variants via
  tessellated path polygons. Deps: GUP-018 ✅, GUP-246 📋, GUP-132 ✅.
- [GUP-248](GUP-248_Heatmap_Chart_Builder.md) 📋 — HeatmapChartBuilder with
  automatic 2D binning, ColorScale value→color mapping, and GPU-instanced
  Rectangle rendering for 1M+ cells at 60 FPS. Deps: GUP-018 ✅, GUP-067 ✅,
  GUP-093 ✅, GUP-255 ✅.
- [GUP-249](GUP-249_Violin_Plot_Builder.md) 📋 — ViolinPlotBuilder using KDE
  (GUP-144) for smooth mirrored density curves with optional embedded box plots
  and half-violin split mode. Deps: GUP-018 ✅, GUP-132 ✅, GUP-144 ✅, GUP-166
  ✅.
- [GUP-250](GUP-250_Density_Plot_Builder.md) 📋 — DensityPlotBuilder with 2D GPU
  KDE compute shader and marching-squares contour extraction, rendered as filled
  contours or line isolevels. Deps: GUP-018 ✅, GUP-144 ✅, GUP-132 ✅, GUP-248
  📋.
- [GUP-251](GUP-251_Custom_Composite_Chart_Support.md) 📋 —
  CompositeChartBuilder composing multiple chart layers with shared axes,
  unified scale domains, and optional dual-y-axis support. Deps: GUP-001 ✅,
  GUP-018 ✅, GUP-093 ✅, GUP-245 📋, GUP-246 📋.
- [GUP-275](GUP-275_Choropleth_Chart_Builder.md) ✅ — ChoroplethChartBuilder
  mapping GeoJSON region values to colors with projection selection, colorbar
  legend, and zoom/pan. Deps: GUP-018 ✅, GUP-273 ✅, GUP-274 ✅, GUP-255 ✅.

## Performance & Profiling

Benchmarking suites, CI performance alerts, GPU timestamp queries, memory
profiling, trend visualisation.

- [GUP-046](GUP-046_Context_Performance_Profiling.md) ✅ — The current
  FrameStats in GupContext provides basic timing information.
- [GUP-080](GUP-080_WebGPU_Timestamp_Query_Integration.md) ✅ — WebGPU Timestamp
  Query Integration.
- [GUP-084](GUP-084_Error_Handling_Performance_Optimization.md) ✅ — Error
  Handling Performance Optimization.
- [GUP-137](GUP-137_Shader_Performance_Benchmarking.md) ✅ — GUP-033 claimed
  "performance within 15% of hand-optimized shaders" but this was not
  empirically validated.
- [GUP-152](GUP-152_Performance_Trend_Visualization.md) ✅ — Performance Trend
  Visualization.
- [GUP-153](GUP-153_Automated_Baseline_Recommendation.md) ✅ — Automated
  Baseline Recommendation.
- [GUP-161](GUP-161_GPU_Timestamp_Query_Integration.md) ✅ — GUP-156 implemented
  pattern performance benchmarks, but they only measure CPU-side overhead….
  Deps: GUP-156 ✅.
- [GUP-172](GUP-172_WebAssembly_Performance_Benchmarks.md) ✅ — Create headless
  browser benchmarking infrastructure to measure and compare WebGPU/WASM….
- [GUP-188](GUP-188_Automatic_Draw_Call_Metrics.md) ✅ — Add a
  render_marks_tracked(&mut self, ...) variant to MarkRenderer that
  automatically….

## Debug & Development Tools

GPU debug visualisation, memory profiling, resource dependency graph, buffer
validation.

- [GUP-015](GUP-015_GPU_Debugging_Tools.md) ✅ — GPU Debugging and Profiling
  Tools.
- [GUP-015](GUP-015_Real_Time_Data_Streaming.md) 📋 — Implements
  StreamingBuffer<T> with keyed insert/update/remove, dirty-region tracking,
  double-buffering swap, and GPU-flush of only mutated byte ranges. Deps:
  GUP-002 ✅, GUP-003 ✅, GUP-004 ✅.
- [GUP-244](GUP-244_Streaming_Data_Builder_API.md) 📋 — Ergonomic DataStream<T>
  builder API (capacity, mode, backpressure) with Selection::stream()
  integration on top of GUP-015's low-level primitives. Deps: GUP-002 ✅,
  GUP-015 📋.
- [GUP-034](GUP-034_GPU_Memory_Profiling_Tools.md) ✅ — During GUP-002
  development, debugging GPU resource issues was challenging.
- [GUP-037](GUP-037_Buffer_Validation_and_Debugging_Tools.md) ✅ — During
  GUP-003 development, debugging buffer operations required manual inspection
  and custom test code. Deps: GUP-003 ✅, GUP-035 ✅.
- [GUP-081](GUP-081_Advanced_Debug_Data_Visualization.md) ✅ — Advanced Debug
  Data Visualization.
- [GUP-082](GUP-082_Debug_Tool_Integration_CI_CD.md) ✅ — Debug Tool Integration
  with CI/CD Pipeline.
- [GUP-083](GUP-083_Debug_Tool_Type_Complexity_Refactor.md) ✅ — Debug Tool Type
  Complexity Refactor.
- [GUP-129](GUP-129_GPU_Debug_Visualization_Tool.md) ✅ — During GUP-128
  debugging, it was challenging to visualize what data was being uploaded to
  GPU….

## CI & Build Infrastructure

CI test workflows, WASM builds, platform gating, ChromeDriver, Puppeteer, flaky
test fixes.

- [GUP-154](GUP-154_Multi-Platform_CI_Testing.md) ✅ — Multi-Platform CI
  Testing.
- [GUP-162](GUP-162_Pattern_Benchmark_CI_Integration.md) ✅ — GUP-156 created
  comprehensive pattern performance benchmarks with Criterion baseline
  management. Deps: GUP-154 ✅, GUP-156 ✅.
- [GUP-173](GUP-173_CI_Performance_Alert_System.md) ✅ — Implement an automated
  alerting system that detects performance regressions in CI/CD pipelines.
- [GUP-174](GUP-174_Flaky_Performance_Test_Stabilization.md) ✅ — Review and
  stabilize timing-sensitive performance tests across the codebase that fail….
- [GUP-187](GUP-187_Flaky_Label_Performance_Test_Fix.md) ✅ — The
  label::positioner::tests::test_performance_500_labels test has an overly tight
  10ms target….
- [GUP-209](GUP-209_Mark_Validation_CI_Integration.md) ✅ — Mark Validation CI
  Integration.
- [GUP-231](GUP-231_WASM_Build_Platform_Gating.md) ✅ — Gate platform-specific
  accessibility backends and DOM integration code behind cfg attributes….
- [GUP-232](GUP-232_Fix_Mark_Renderer_Metric_Tests.md) ✅ — Three tests in
  mark::renderer::tests consistently fail:.
- [GUP-233](GUP-233_Fix_Flaky_Registry_Scalability_Test.md) ✅ — The
  test_registry_scalability test in mark_pipeline_performance_tests.rs
  consistently fails….
- [GUP-233](GUP-233_Winit_Touch_Event_Integration.md) ✅ — Add a convenience
  From<winit::event::Touch> conversion for TouchEvent/TouchPhase and update….
- [GUP-237](GUP-237_WASM_Integration_Test_Suite.md) ✅ — Create a browser-based
  integration test suite that loads the wasm-pack output and verifies….
- [GUP-240](GUP-240_ChromeDriver_Puppeteer_CI_Integration.md) ✅ — Add matching
  ChromeDriver (or Puppeteer) to the nix development environment and CI
  workflows….
- [GUP-243](GUP-243_Puppeteer_HTML_Benchmark_CI_Runner.md) ✅ — Add Puppeteer
  (or Playwright) to the development environment and CI workflow to capture
  JSON….

## Advanced Scale

Billion-point LOD rendering, adaptive viewport, streaming LOD, GPU-accelerated
layouts, and 3D visualization.

- [GUP-256](GUP-256_Level_of_Detail_Pyramid.md) 📋 — LodPyramid struct with GPU
  compute shader pyramid builder for up to 1B points using grid-based point
  aggregation across 5+ LOD levels. Deps: GUP-003 ✅, GUP-004 ✅, GUP-030 ✅,
  GUP-077 ✅.
- [GUP-257](GUP-257_Adaptive_Viewport_Renderer.md) 📋 — AdaptiveRenderer that
  selects the coarsest LOD tier by pixels-per-data-point heuristic and issues a
  frustum-culled indirect draw with no CPU readback. Deps: GUP-256 📋, GUP-076
  ✅, GUP-077 ✅.
- [GUP-258](GUP-258_Streaming_Data_Manager_LOD.md) 📋 — StreamingLodManager
  combining DataStream<T> (GUP-015) with LodPyramid for incremental cell-level
  LOD updates and memory-budget eviction. Deps: GUP-015 📋, GUP-244 📋, GUP-256
  📋.
- [GUP-259](GUP-259_GPU_Force_Directed_Graph_Layout.md) 📋 — GPU compute shader
  force-directed layout (repulsion, spring, gravity, convergence detection)
  targeting 100K nodes in ≤5 seconds. Deps: GUP-003 ✅, GUP-004 ✅, GUP-077 ✅.
- [GUP-260](GUP-260_GPU_Treemap_Layout.md) 📋 — GPU compute shader treemap
  layout with four algorithm variants (Squarified, Binary, Strip, SliceDice)
  outputting Rectangle-compatible cells. Deps: GUP-003 ✅, GUP-004 ✅, GUP-067
  ✅.
- [GUP-261](GUP-261_3D_Visualization_Support.md) 📋 — Depth buffer,
  Camera/projection uniforms, Phong lighting, and Sphere3D/Box3D/Line3D marks
  enabling 3D scatter plots with materials. Deps: GUP-004 ✅, GUP-009 ✅,
  GUP-010 ✅, GUP-131 ✅.

## Ecosystem Integration

Framework integrations (Bevy, egui, Tauri, winit), export formats (SVG, PDF,
PNG, HTML), and platform targets.

- [GUP-262](GUP-262_Bevy_Integration.md) 📋 — gup-bevy crate with GupChart Bevy
  Component and GupPlugin sharing the wgpu device/queue with Bevy's renderer.
  Deps: GUP-004 ✅, GUP-018 ✅, GUP-039 ✅.
- [GUP-263](GUP-263_egui_Integration.md) 📋 — GupWidget implementing
  egui::Widget via render-to-texture, with dirty-tracking and interaction bridge
  forwarding egui pointer events. Deps: GUP-004 ✅, GUP-018 ✅, GUP-268 📋.
- [GUP-264](GUP-264_Tauri_Integration.md) 📋 — gup-tauri example running Gup
  WASM in a Tauri WebView with a Rust IPC bridge feeding data to the chart.
  Deps: GUP-004 ✅, GUP-018 ✅, GUP-172 ✅, GUP-237 ✅.
- [GUP-265](GUP-265_winit_Application_Shell.md) 📋 — GupApp application shell
  wrapping the full winit event loop (surface, resize, device-loss recovery)
  into a 5-line builder entry point. Deps: GUP-039 ✅, GUP-047 ✅, GUP-049 ✅,
  GUP-013 📋.
- [GUP-266](GUP-266_SVG_Export.md) 📋 — SvgRenderer extracting vector paths from
  marks and generating a valid SVG document with correct clip-space→viewport
  coordinate mapping. Deps: GUP-009 ✅, GUP-018 ✅, GUP-099 ✅.
- [GUP-267](GUP-267_PDF_Export.md) 📋 — PdfRenderer converting the GUP-266 SVG
  intermediate to a PDF with embedded font subsets, configurable page sizes, and
  multi-page support. Deps: GUP-018 ✅, GUP-266 📋.
- [GUP-268](GUP-268_PNG_Export.md) 📋 — Off-screen GPU render-to-texture with
  staging-buffer readback and PNG encoding via the image crate, supporting HiDPI
  scale factors. Deps: GUP-004 ✅, GUP-035 ✅, GUP-018 ✅.
- [GUP-269](GUP-269_HTML_Export.md) 📋 — HtmlExporter generating a single-file
  interactive HTML page with embedded WASM, data JSON, SVG fallback, and OG
  thumbnail meta tags. Deps: GUP-266 📋, GUP-268 📋, GUP-172 ✅.

## Mobile

iOS and Android platform support for GPU-accelerated Gup charts.

- [GUP-270](GUP-270_iOS_Platform_Support.md) 📋 — CAMetalLayer Metal surface,
  Swift/Obj-C UIKit/SwiftUI integration shim, and UITouch→InteractionEvent
  translation for iOS/iPadOS. Deps: GUP-004 ✅, GUP-039 ✅, GUP-182 ✅, GUP-013
  📋.
- [GUP-271](GUP-271_Android_Platform_Support.md) 📋 — Android SurfaceView
  lifecycle, JNI NDK wrapper, MotionEvent→InteractionEvent bridge, and APK
  example for Kotlin/Java embedding. Deps: GUP-004 ✅, GUP-039 ✅, GUP-182 ✅,
  GUP-013 📋, GUP-270 📋.

## Documentation

API reference generation, tutorials, and example gallery.

- [GUP-280](GUP-280_API_Reference_Generation.md) 📋 — Comprehensive rustdoc
  coverage of all public APIs with runnable doc examples, docs.rs configuration,
  and a CI gate enforcing zero doc warnings. Deps: GUP-002 ✅, GUP-005 ✅,
  GUP-009 ✅, GUP-018 ✅.
- [GUP-281](GUP-281_Tutorial_and_Guide_Suite.md) 📋 — Six step-by-step tutorials
  in docs/tutorials/ covering Getting Started, Data Binding, Custom Shaders,
  Interactions, Streaming, and Custom Marks. Deps: GUP-002 ✅, GUP-018 ✅,
  GUP-103 ✅, GUP-280 📋.
- [GUP-282](GUP-282_Example_Gallery.md) 📋 — Automated thumbnail generation (via
  GUP-268 PNG Export) and GitHub Pages gallery grouped by category with CI
  deployment. Deps: GUP-103 ✅, GUP-268 📋.
