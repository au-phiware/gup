// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Procedural macros for the Gup GPU visualization library

use proc_macro::TokenStream;
use quote::ToTokens;
use syn::parse_macro_input;

mod wgsl_function;

use wgsl_function::WgslFunctionInfo;

/// Procedural macro for generating shader functions from WGSL syntax.
///
/// This macro allows you to write WGSL functions directly in Rust and automatically
/// generates the corresponding `ComposableShaderFunction` trait implementation.
///
/// # Example
///
/// ```rust,ignore
/// use gup::*;
///
/// #[wgsl_function]
/// fn linear_scale(value: f32, scale: f32) -> f32 {
///     return value * scale;
/// }
/// ```
///
/// This generates:
/// - A `LinearScale` struct with configuration fields
/// - A `LinearScaleUniforms` struct for GPU uniforms
/// - An implementation of `ComposableShaderFunction` for `LinearScale`
/// - WGSL code that can be compiled and run on the GPU
#[proc_macro_attribute]
pub fn wgsl_function(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as WgslFunctionInfo);
    let mut tokens = proc_macro2::TokenStream::new();
    input.to_tokens(&mut tokens);
    TokenStream::from(tokens)
}
