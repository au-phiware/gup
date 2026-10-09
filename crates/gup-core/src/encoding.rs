// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Encodings: shader functions with CPU mirrors (RFC-001 §5) and the
//! `IntoEncoding` conversion that type-checks `Selection::attr` (§4).

use crate::channel::{ConstValue, GpuType, Visual};
use crate::column::{ColumnData, ColumnFormat};
use crate::error::{Error, Result};
use crate::shader::WgslModule;
use std::marker::PhantomData;

/// A GPU function from a column value (`In`) to a visual value (`Out`),
/// implemented by a WGSL library module.
///
/// The module's entry point has the signature
/// `fn ENTRY(x: In, p: Params [, resources…]) -> Out`, or for relative
/// inputs ([`ColumnFormat::F32Relative`])
/// `fn ENTRY(v: f32, base: f32, p: Params [, resources…]) -> Out`, where
/// `base` comes from [`ShaderFn::chunk_base`]. A resource
/// ([`Resource::Lut`]) adds a `texture_2d<f32>` and a `sampler` argument.
pub trait ShaderFn: Clone + Send + Sync + 'static {
    /// The GPU input type.
    type In: GpuType;
    /// The visual output type.
    type Out: GpuType;
    /// The uniform parameters. Their std140 layout comes from `encase`, so
    /// the Rust and WGSL structs cannot drift silently (a test checks the
    /// sizes against naga's layout of the module). The WGSL struct must
    /// span a multiple of 16 bytes; the build fails otherwise (GUP-406).
    type Params: encase::ShaderType + encase::internal::WriteInto;

    /// The WGSL module implementing the function.
    const MODULE: &'static WgslModule;
    /// The entry function in [`ShaderFn::MODULE`].
    const ENTRY: &'static str;

    /// Current parameters (domain, range, …). Rescaling or zooming changes
    /// only these.
    fn params(&self) -> Self::Params;

    /// How the input column is stored.
    fn input_format(&self) -> ColumnFormat;

    /// The per-chunk `base` passed to a relative entry point for a chunk
    /// whose values are stored relative to `origin`. The default makes the
    /// shader see the absolute value; scales override it to fold their
    /// domain start in (in f64) and keep precision.
    fn chunk_base(&self, origin: f64) -> f32 {
        origin as f32
    }

    /// Extra GPU resources the entry point takes.
    fn resources(&self) -> Vec<Resource> {
        Vec::new()
    }

    /// Adopt the data extent of the encoded column if this function has an
    /// automatic (data-driven) domain. Functions without a domain accept
    /// any extent.
    fn fit_domain(&mut self, extent: (f64, f64)) -> Result<()> {
        let _ = extent;
        Ok(())
    }

    /// Encode a field of each row through (a clone of) this function:
    /// `scale.encode(|r: &Row| r.value)`. Taking `&self` keeps shared
    /// handles such as [`ScaleRef`](crate::ScaleRef) usable for later
    /// domain changes (zoom, pan).
    fn encode<T, D, A>(&self, accessor: A) -> Encoded<A, Self>
    where
        A: Fn(&T) -> D,
        D: ColumnValue,
    {
        Encoded {
            accessor,
            func: self.clone(),
        }
    }

    /// Encode a string key of each row through a dictionary (RFC-001 S4b):
    /// `categorical.encode_key(|r: &Row| r.continent.as_str())`. Each
    /// distinct key gets the next `u32` code in first-seen order, and that
    /// order is the domain.
    ///
    /// The accessor may return a borrow of its row: its bound is
    /// `for<'a> Fn(&'a T) -> &'a str`, which [`encode`](Self::encode)'s
    /// `Fn(&T) -> D` cannot express (RFC-001 §12 risk 4). Rows whose key
    /// can be missing use [`encode_nullable_key`](Self::encode_nullable_key).
    fn encode_key<T, A>(&self, accessor: A) -> KeyEncoded<Key<A>, Self>
    where
        Self: ShaderFn<In = u32>,
        A: for<'a> Fn(&'a T) -> &'a str,
    {
        KeyEncoded {
            key: Key(accessor),
            func: self.clone(),
        }
    }

    /// Like [`encode_key`](Self::encode_key), for keys that may be
    /// missing: `None` is a null, stored as
    /// [`NULL_CODE`](crate::column::NULL_CODE), which a colour function
    /// draws in its null colour.
    fn encode_nullable_key<T, A>(&self, accessor: A) -> KeyEncoded<NullableKey<A>, Self>
    where
        Self: ShaderFn<In = u32>,
        A: for<'a> Fn(&'a T) -> Option<&'a str>,
    {
        KeyEncoded {
            key: NullableKey(accessor),
            func: self.clone(),
        }
    }
}

/// The exact f64 CPU twin of a [`ShaderFn`], used for axes, ticks, legends,
/// picking and vector output.
pub trait CpuMirror: ShaderFn {
    /// Evaluate the function on the CPU.
    fn eval(&self, x: f64) -> <Self::Out as GpuType>::Cpu;
}

/// A GPU resource argument of a shader function.
#[derive(Clone, Debug, PartialEq)]
pub enum Resource {
    /// A one-row palette lookup table of sRGB-encoded RGBA8 colours,
    /// sampled with linear filtering between texel centres.
    Lut(Vec<[u8; 4]>),
}

/// A value an accessor may return for a column. Non-finite values (NaN,
/// ±∞) are nulls: a null position or size is not drawn, and a null
/// colour input draws in the null colour (RFC-001 S4b).
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a column value; `encode` takes accessors returning numbers",
    label = "`encode`'s accessor returns `{Self}`",
    note = "for string keys (categories), use `encode_key(|row| row.field.as_str())` on a dictionary function such as `Categorical`: its accessor may return a borrow of the row"
)]
pub trait ColumnValue: Copy + 'static {
    /// The GPU type the column holds.
    type Gpu: GpuType;
    /// The value in f64, from which the column (and its stats) is built.
    fn to_f64(self) -> f64;
}

impl ColumnValue for f64 {
    type Gpu = f32;
    fn to_f64(self) -> f64 {
        self
    }
}

impl ColumnValue for f32 {
    type Gpu = f32;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
}

/// An accessor composed with a shader function, made by
/// [`ShaderFn::encode`].
#[derive(Clone)]
pub struct Encoded<A, S> {
    accessor: A,
    func: S,
}

/// A key accessor composed with a shader function over dictionary codes,
/// made by [`ShaderFn::encode_key`] or [`ShaderFn::encode_nullable_key`].
#[derive(Clone)]
pub struct KeyEncoded<K, S> {
    key: K,
    func: S,
}

/// Reads a dictionary key from a row (see [`ShaderFn::encode_key`]).
/// Implemented by [`Key`] and [`NullableKey`], which those methods make.
pub trait KeyAccessor<T>: Send + Sync + 'static {
    /// The key of `row`, borrowed from it; `None` is a null.
    fn key<'a>(&self, row: &'a T) -> Option<&'a str>;
}

/// A key accessor whose rows always have a key (made by
/// [`ShaderFn::encode_key`]).
#[derive(Clone)]
pub struct Key<A>(A);

/// A key accessor whose rows may lack a key (made by
/// [`ShaderFn::encode_nullable_key`]).
#[derive(Clone)]
pub struct NullableKey<A>(A);

impl<T, A> KeyAccessor<T> for Key<A>
where
    A: for<'a> Fn(&'a T) -> &'a str + Send + Sync + 'static,
{
    fn key<'a>(&self, row: &'a T) -> Option<&'a str> {
        Some((self.0)(row))
    }
}

impl<T, A> KeyAccessor<T> for NullableKey<A>
where
    A: for<'a> Fn(&'a T) -> Option<&'a str> + Send + Sync + 'static,
{
    fn key<'a>(&self, row: &'a T) -> Option<&'a str> {
        (self.0)(row)
    }
}

/// Marker for constant visual values (see [`IntoEncoding`]).
pub enum ConstMarker {}

/// Marker for column encodings producing `D` (see [`IntoEncoding`]).
pub struct ColumnMarker<D>(PhantomData<fn() -> D>);

/// Marker for dictionary-key encodings (see [`IntoEncoding`]).
pub enum KeyMarker {}

/// Anything that can drive a channel of visual type `V` for rows of type
/// `T`: a constant `V`, or `shader_fn.encode(accessor)` whose function
/// outputs `V`. `K` is a marker that keeps the blanket impls apart; it is
/// always inferred.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot drive a channel of visual type `{V}`",
    label = "this encoding does not produce `{V}` for rows of type `{T}`",
    note = "a `{V}` channel accepts a constant of type `{V}` (such as `Px(3.0)`) or `f.encode(|row: &{T}| …)` (or `f.encode_key(..)` for string keys) where `f: ShaderFn<Out = {V}>`"
)]
pub trait IntoEncoding<T, V: Visual, K> {
    /// Convert into a type-erased encoding.
    fn into_encoding(self) -> Encoding<T>;
}

impl<T, V: Visual> IntoEncoding<T, V, ConstMarker> for V {
    fn into_encoding(self) -> Encoding<T> {
        Encoding::Const(self.to_const())
    }
}

impl<T, V, A, D, S> IntoEncoding<T, V, ColumnMarker<D>> for Encoded<A, S>
where
    T: 'static,
    V: Visual,
    A: Fn(&T) -> D + Send + Sync + 'static,
    D: ColumnValue,
    S: ShaderFn<In = D::Gpu, Out = V>,
{
    fn into_encoding(self) -> Encoding<T> {
        Encoding::Column(Box::new(ColumnEncoding {
            accessor: self.accessor,
            func: self.func,
            _d: PhantomData,
        }))
    }
}

impl<T, V, K, S> IntoEncoding<T, V, KeyMarker> for KeyEncoded<K, S>
where
    T: 'static,
    V: Visual,
    K: KeyAccessor<T>,
    S: ShaderFn<In = u32, Out = V>,
{
    fn into_encoding(self) -> Encoding<T> {
        Encoding::Column(Box::new(self))
    }
}

/// A type-erased channel encoding.
pub enum Encoding<T> {
    /// A constant (a uniform field, no column).
    Const(ConstValue),
    /// An accessor plus a shader function (a column plus a GPU call).
    Column(Box<dyn DynColumnEncoding<T>>),
}

impl<T> std::fmt::Debug for Encoding<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Encoding::Const(c) => write!(f, "Const({c:?})"),
            Encoding::Column(c) => write!(f, "Column({})", c.func().signature()),
        }
    }
}

/// Object-safe view of an accessor + shader function.
pub trait DynColumnEncoding<T>: Send + Sync {
    /// Run the accessor over every row: numbers, or keys borrowed from
    /// the rows.
    fn evaluate<'r>(&self, rows: &'r [T]) -> ColumnData<'r>;
    /// The shader function.
    fn func(&self) -> &dyn DynShaderFn;
    /// The shader function, mutably.
    fn func_mut(&mut self) -> &mut dyn DynShaderFn;
}

struct ColumnEncoding<A, S, D> {
    accessor: A,
    func: S,
    _d: PhantomData<fn() -> D>,
}

impl<T, A, D, S> DynColumnEncoding<T> for ColumnEncoding<A, S, D>
where
    A: Fn(&T) -> D + Send + Sync,
    D: ColumnValue,
    S: ShaderFn,
{
    fn evaluate<'r>(&self, rows: &'r [T]) -> ColumnData<'r> {
        ColumnData::Values(rows.iter().map(|r| (self.accessor)(r).to_f64()).collect())
    }

    fn func(&self) -> &dyn DynShaderFn {
        &self.func
    }

    fn func_mut(&mut self) -> &mut dyn DynShaderFn {
        &mut self.func
    }
}

impl<T, K, S> DynColumnEncoding<T> for KeyEncoded<K, S>
where
    K: KeyAccessor<T>,
    S: ShaderFn,
{
    fn evaluate<'r>(&self, rows: &'r [T]) -> ColumnData<'r> {
        ColumnData::Keys(rows.iter().map(|r| self.key.key(r)).collect())
    }

    fn func(&self) -> &dyn DynShaderFn {
        &self.func
    }

    fn func_mut(&mut self) -> &mut dyn DynShaderFn {
        &mut self.func
    }
}

/// Object-safe view of a [`ShaderFn`], used by the glue emitter and the
/// layer's uniform writer.
pub trait DynShaderFn: Send + Sync {
    /// The WGSL module.
    fn module(&self) -> &'static WgslModule;
    /// The entry function.
    fn entry(&self) -> &'static str;
    /// The WGSL output type.
    fn out_wgsl(&self) -> &'static str;
    /// How the input column is stored.
    fn input_format(&self) -> ColumnFormat;
    /// The std140 bytes of the current parameters.
    fn params_bytes(&self) -> Result<Vec<u8>>;
    /// Size in bytes of the `Params` uniform struct (as WGSL lays it out;
    /// `rust_params_match_wgsl_params` checks the two agree).
    fn params_size(&self) -> u64;
    /// See [`ShaderFn::chunk_base`].
    fn chunk_base(&self, origin: f64) -> f32;
    /// See [`ShaderFn::resources`].
    fn resources(&self) -> Vec<Resource>;
    /// See [`ShaderFn::fit_domain`].
    fn fit_domain(&mut self, extent: (f64, f64)) -> Result<()>;
    /// A stable description used in the pipeline cache key: everything
    /// that changes the generated WGSL, nothing that only changes uniforms.
    fn signature(&self) -> String;
}

impl<S: ShaderFn> DynShaderFn for S {
    fn module(&self) -> &'static WgslModule {
        S::MODULE
    }

    fn entry(&self) -> &'static str {
        S::ENTRY
    }

    fn out_wgsl(&self) -> &'static str {
        <S::Out as GpuType>::WGSL
    }

    fn input_format(&self) -> ColumnFormat {
        ShaderFn::input_format(self)
    }

    fn params_size(&self) -> u64 {
        <S::Params as encase::ShaderType>::min_size().get()
    }

    fn params_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = encase::UniformBuffer::new(Vec::<u8>::new());
        buf.write(&self.params())
            .map_err(|e| Error::Configuration {
                what: "shader function parameters",
                detail: format!("{}::{}: {e}", S::MODULE.import_path, S::ENTRY),
            })?;
        Ok(buf.into_inner())
    }

    fn chunk_base(&self, origin: f64) -> f32 {
        ShaderFn::chunk_base(self, origin)
    }

    fn resources(&self) -> Vec<Resource> {
        ShaderFn::resources(self)
    }

    fn fit_domain(&mut self, extent: (f64, f64)) -> Result<()> {
        ShaderFn::fit_domain(self, extent)
    }

    fn signature(&self) -> String {
        let format = match ShaderFn::input_format(self) {
            ColumnFormat::F32 => "f32",
            ColumnFormat::F32Relative => "f32rel",
            ColumnFormat::U32 => "u32",
        };
        let resources = ShaderFn::resources(self)
            .iter()
            .map(|r| match r {
                Resource::Lut(_) => "(lut)",
            })
            .collect::<String>();
        format!(
            "{format}→{}::{}{resources}",
            S::MODULE.import_path,
            S::ENTRY
        )
    }
}
