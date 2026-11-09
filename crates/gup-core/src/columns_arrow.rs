// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arrow record batches as columnar input (GUP-630, feature `arrow`).
//!
//! One inherent constructor, [`Columns::from_arrow`], and a `TryFrom`; no
//! Arrow type appears in any other public signature.
//!
//! Numeric columns are copied once, into an `Arc<[f64]>` or `Arc<[f32]>`
//! (the shape [`Column`] holds): an Arrow buffer is not an `Arc<[T]>`, and
//! `Arc<[T]>` has no way to adopt an allocation, since the `Arc` header
//! lives in front of the values. The copy is a single allocation and one
//! pass, small next to the f32 conversion the store makes anyway. Key
//! columns are built, not borrowed: Arrow's string and dictionary layouts
//! hold offsets and bytes, not `Arc<str>` labels.

use crate::column::NULL_CODE;
use crate::columns::{Column, Columns, KeyColumn};
use crate::error::{Error, Result};
use arrow_array::cast::AsArray;
use arrow_array::types::{
    ArrowDictionaryKeyType, Date32Type, Date64Type, Float32Type, Float64Type, Int8Type, Int16Type,
    Int32Type, Int64Type, TimestampMicrosecondType, TimestampMillisecondType,
    TimestampNanosecondType, TimestampSecondType, UInt8Type, UInt16Type, UInt32Type, UInt64Type,
};
use arrow_array::{Array, ArrowPrimitiveType, PrimitiveArray, RecordBatch};
use arrow_schema::{DataType, TimeUnit};
use std::collections::HashMap;
use std::sync::Arc;

/// `f` of each value of `array` (`null` where it is null), collected
/// straight into an `Arc` (one allocation: a mapped slice iterator has a
/// known length).
fn build<T: ArrowPrimitiveType, U: Copy>(
    array: &PrimitiveArray<T>,
    f: impl Fn(T::Native) -> U,
    null: U,
) -> Arc<[U]> {
    let mut out: Arc<[U]> = array.values().iter().map(|&v| f(v)).collect();
    if array.null_count() > 0
        && let (Some(nulls), Some(cells)) = (array.nulls(), Arc::get_mut(&mut out))
    {
        for (cell, valid) in cells.iter_mut().zip(nulls.iter()) {
            if !valid {
                *cell = null;
            }
        }
    }
    out
}

fn seconds(v: i64, per_second: i64) -> f64 {
    // Split first: nanoseconds since the epoch exceed 2^53.
    v.div_euclid(per_second) as f64 + v.rem_euclid(per_second) as f64 / per_second as f64
}

fn unsupported(name: &str, data_type: &DataType) -> Error {
    Error::config(
        "Arrow batch",
        format!("column {name:?}: Arrow type {data_type} is not supported"),
    )
}

macro_rules! numeric {
    ($array:expr, $t:ty) => {
        Column::F64(build($array.as_primitive::<$t>(), |v| v as f64, f64::NAN))
    };
}

macro_rules! timestamp {
    ($array:expr, $t:ty, $per_second:expr) => {
        Column::F64(build(
            $array.as_primitive::<$t>(),
            |v| seconds(v, $per_second),
            f64::NAN,
        ))
    };
}

fn dictionary<K: ArrowDictionaryKeyType>(name: &str, array: &dyn Array) -> Result<KeyColumn> {
    let dict = array.as_dictionary::<K>();
    let values: Vec<Option<&str>> = match dict.values().data_type() {
        DataType::Utf8 => dict.values().as_string::<i32>().iter().collect(),
        DataType::LargeUtf8 => dict.values().as_string::<i64>().iter().collect(),
        other => {
            return Err(Error::config(
                "Arrow batch",
                format!(
                    "column {name:?}: Arrow dictionary values of type {other} are not supported \
                     (Utf8 or LargeUtf8)"
                ),
            ));
        }
    };
    // Arrow allows a repeated or null dictionary value; a key column does
    // not, so map each value to the code of its first occurrence.
    let mut first: HashMap<&str, u32> = HashMap::with_capacity(values.len());
    let mut labels: Vec<Arc<str>> = Vec::new();
    let remap: Vec<u32> = values
        .iter()
        .map(|v| match v {
            None => NULL_CODE,
            Some(s) => *first.entry(s).or_insert_with(|| {
                labels.push(Arc::from(*s));
                (labels.len() - 1) as u32
            }),
        })
        .collect();
    let mut codes = Vec::with_capacity(dict.len());
    for (row, key) in dict.keys_iter().enumerate() {
        codes.push(match key {
            None => NULL_CODE,
            Some(k) => *remap.get(k).ok_or_else(|| {
                Error::config(
                    "Arrow batch",
                    format!(
                        "column {name:?}: row {row} has dictionary key {k}, but the dictionary \
                         has {} values",
                        remap.len()
                    ),
                )
            })?,
        });
    }
    KeyColumn::new(codes, labels)
}

fn convert(name: &str, array: &dyn Array) -> Result<Column> {
    Ok(match array.data_type() {
        DataType::Float64 => {
            Column::F64(build(array.as_primitive::<Float64Type>(), |v| v, f64::NAN))
        }
        DataType::Float32 => {
            Column::F32(build(array.as_primitive::<Float32Type>(), |v| v, f32::NAN))
        }
        DataType::Int8 => numeric!(array, Int8Type),
        DataType::Int16 => numeric!(array, Int16Type),
        DataType::Int32 => numeric!(array, Int32Type),
        DataType::Int64 => numeric!(array, Int64Type),
        DataType::UInt8 => numeric!(array, UInt8Type),
        DataType::UInt16 => numeric!(array, UInt16Type),
        DataType::UInt32 => numeric!(array, UInt32Type),
        DataType::UInt64 => numeric!(array, UInt64Type),
        DataType::Date32 => Column::F64(build(
            array.as_primitive::<Date32Type>(),
            |days| f64::from(days) * 86_400.0,
            f64::NAN,
        )),
        DataType::Date64 => timestamp!(array, Date64Type, 1_000),
        DataType::Timestamp(TimeUnit::Second, _) => timestamp!(array, TimestampSecondType, 1),
        DataType::Timestamp(TimeUnit::Millisecond, _) => {
            timestamp!(array, TimestampMillisecondType, 1_000)
        }
        DataType::Timestamp(TimeUnit::Microsecond, _) => {
            timestamp!(array, TimestampMicrosecondType, 1_000_000)
        }
        DataType::Timestamp(TimeUnit::Nanosecond, _) => {
            timestamp!(array, TimestampNanosecondType, 1_000_000_000)
        }
        DataType::Utf8 => KeyColumn::from_optional_labels(array.as_string::<i32>().iter()).into(),
        DataType::LargeUtf8 => {
            KeyColumn::from_optional_labels(array.as_string::<i64>().iter()).into()
        }
        DataType::Utf8View => KeyColumn::from_optional_labels(array.as_string_view().iter()).into(),
        DataType::Boolean => {
            let codes: Vec<u32> = array
                .as_boolean()
                .iter()
                .map(|b| b.map_or(NULL_CODE, u32::from))
                .collect();
            KeyColumn::new(codes, vec![Arc::<str>::from("false"), Arc::from("true")])?.into()
        }
        DataType::Dictionary(key, _) => match &**key {
            DataType::Int8 => dictionary::<Int8Type>(name, array)?,
            DataType::Int16 => dictionary::<Int16Type>(name, array)?,
            DataType::Int32 => dictionary::<Int32Type>(name, array)?,
            DataType::Int64 => dictionary::<Int64Type>(name, array)?,
            DataType::UInt8 => dictionary::<UInt8Type>(name, array)?,
            DataType::UInt16 => dictionary::<UInt16Type>(name, array)?,
            DataType::UInt32 => dictionary::<UInt32Type>(name, array)?,
            DataType::UInt64 => dictionary::<UInt64Type>(name, array)?,
            _ => return Err(unsupported(name, array.data_type())),
        }
        .into(),
        other => return Err(unsupported(name, other)),
    })
}

impl Columns {
    /// The columns of an Arrow record batch, named by the schema's fields.
    ///
    /// | Arrow type | Column |
    /// | --- | --- |
    /// | `Float64` | `F64` |
    /// | `Float32` | `F32` |
    /// | `Int8` to `Int64`, `UInt8` to `UInt64` | `F64` (values beyond 2^53 lose precision) |
    /// | `Date32` | `F64`, Unix seconds (days x 86,400) |
    /// | `Date64`, `Timestamp` (any unit and zone) | `F64`, Unix seconds (a zone does not shift the instant) |
    /// | `Utf8`, `LargeUtf8`, `Utf8View` | `Keys`, first-seen order |
    /// | `Dictionary` (any integer key; `Utf8` or `LargeUtf8` values) | `Keys`: the values are the labels, the keys the codes |
    /// | `Boolean` | `Keys`, labels `"false"` and `"true"` |
    ///
    /// Numeric nulls become NaN and key nulls become null rows. Any other
    /// type is an error naming the column. Numeric columns are copied once
    /// (an Arrow buffer is not an `Arc<[T]>`); a dictionary's repeated
    /// values share a code.
    pub fn from_arrow(batch: &RecordBatch) -> Result<Self> {
        let mut columns = Self::new();
        for (field, array) in batch.schema().fields().iter().zip(batch.columns()) {
            columns = columns.with(field.name().as_str(), convert(field.name(), array)?)?;
        }
        Ok(columns)
    }
}

impl TryFrom<&RecordBatch> for Columns {
    type Error = Error;

    fn try_from(batch: &RecordBatch) -> Result<Self> {
        Self::from_arrow(batch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::types::{Int8Type, UInt16Type};
    use arrow_array::{
        ArrayRef, BooleanArray, Date32Array, Date64Array, DictionaryArray, Float32Array,
        Float64Array, Int8Array, Int16Array, Int32Array, Int64Array, IntervalDayTimeArray,
        LargeStringArray, StringArray, StringViewArray, TimestampNanosecondArray,
        TimestampSecondArray, UInt8Array, UInt16Array, UInt32Array, UInt64Array,
    };
    use arrow_schema::{Field, Schema};

    fn batch(columns: Vec<(&str, ArrayRef)>) -> RecordBatch {
        RecordBatch::try_from_iter(columns).unwrap()
    }

    fn f64s(c: &Column) -> Vec<f64> {
        match c {
            Column::F64(v) => v.to_vec(),
            other => panic!("not an F64 column: {other:?}"),
        }
    }

    fn keys(c: &Column) -> (Vec<u32>, Vec<String>) {
        match c {
            Column::Keys(k) => (
                k.codes().to_vec(),
                k.labels().iter().map(|l| l.to_string()).collect(),
            ),
            other => panic!("not a key column: {other:?}"),
        }
    }

    fn same(a: &[f64], b: &[f64]) -> bool {
        a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(a, b)| a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()))
    }

    #[test]
    fn every_supported_type_converts_with_its_null() {
        let nan = f64::NAN;
        let mut cols: Vec<(&str, ArrayRef)> = vec![
            (
                "i8",
                Arc::new(Int8Array::from(vec![Some(-1), None, Some(3)])),
            ),
            (
                "i16",
                Arc::new(Int16Array::from(vec![Some(-1), None, Some(3)])),
            ),
            (
                "i32",
                Arc::new(Int32Array::from(vec![Some(-1), None, Some(3)])),
            ),
            (
                "i64",
                Arc::new(Int64Array::from(vec![Some(-1), None, Some(3)])),
            ),
            (
                "u8",
                Arc::new(UInt8Array::from(vec![Some(1), None, Some(3)])),
            ),
            (
                "u16",
                Arc::new(UInt16Array::from(vec![Some(1), None, Some(3)])),
            ),
            (
                "u32",
                Arc::new(UInt32Array::from(vec![Some(1), None, Some(3)])),
            ),
            (
                "u64",
                Arc::new(UInt64Array::from(vec![Some(1), None, Some(3)])),
            ),
        ];
        cols.extend([
            (
                "f64",
                Arc::new(Float64Array::from(vec![Some(0.5), None, Some(2.5)])) as ArrayRef,
            ),
            (
                "f32",
                Arc::new(Float32Array::from(vec![Some(0.5), None, Some(2.5)])),
            ),
            (
                "d32",
                Arc::new(Date32Array::from(vec![Some(1), None, Some(-1)])),
            ),
            (
                "d64",
                Arc::new(Date64Array::from(vec![Some(1_500), None, Some(-500)])),
            ),
            (
                "ts_s",
                Arc::new(TimestampSecondArray::from(vec![Some(10), None, Some(-10)])),
            ),
            (
                "ts_ns",
                Arc::new(
                    TimestampNanosecondArray::from(vec![
                        Some(1_700_000_000_500_000_000),
                        None,
                        Some(-250_000_000),
                    ])
                    .with_timezone("Australia/Sydney"),
                ),
            ),
            (
                "utf8",
                Arc::new(StringArray::from(vec![Some("b"), None, Some("a")])),
            ),
            (
                "large",
                Arc::new(LargeStringArray::from(vec![Some("b"), None, Some("b")])),
            ),
            (
                "view",
                Arc::new(StringViewArray::from(vec![Some("x"), None, Some("y")])),
            ),
            (
                "bool",
                Arc::new(BooleanArray::from(vec![Some(true), None, Some(false)])),
            ),
            (
                "dict",
                Arc::new(
                    DictionaryArray::<Int8Type>::try_new(
                        Int8Array::from(vec![Some(2), None, Some(0)]),
                        Arc::new(StringArray::from(vec!["p", "q", "r"])),
                    )
                    .unwrap(),
                ),
            ),
        ]);
        let cols = Columns::from_arrow(&batch(cols)).unwrap();
        assert_eq!(cols.len(), 3);
        for name in ["i8", "i16", "i32", "i64"] {
            assert!(same(&f64s(&cols[name]), &[-1.0, nan, 3.0]), "{name}");
        }
        for name in ["u8", "u16", "u32", "u64"] {
            assert!(same(&f64s(&cols[name]), &[1.0, nan, 3.0]), "{name}");
        }
        assert!(same(&f64s(&cols["f64"]), &[0.5, nan, 2.5]));
        assert!(
            matches!(&cols["f32"], Column::F32(v) if v[0] == 0.5 && v[1].is_nan() && v[2] == 2.5)
        );
        assert!(same(&f64s(&cols["d32"]), &[86_400.0, nan, -86_400.0]));
        assert!(same(&f64s(&cols["d64"]), &[1.5, nan, -0.5]));
        assert!(same(&f64s(&cols["ts_s"]), &[10.0, nan, -10.0]));
        // The zone does not shift the instant; nanoseconds keep their digits.
        assert!(same(&f64s(&cols["ts_ns"]), &[1_700_000_000.5, nan, -0.25]));
        let n = NULL_CODE;
        assert_eq!(
            keys(&cols["utf8"]),
            (vec![0, n, 1], vec!["b".into(), "a".into()])
        );
        assert_eq!(keys(&cols["large"]), (vec![0, n, 0], vec!["b".into()]));
        assert_eq!(
            keys(&cols["view"]),
            (vec![0, n, 1], vec!["x".into(), "y".into()])
        );
        assert_eq!(
            keys(&cols["bool"]),
            (vec![1, n, 0], vec!["false".into(), "true".into()])
        );
        assert_eq!(
            keys(&cols["dict"]),
            (vec![2, n, 0], vec!["p".into(), "q".into(), "r".into()])
        );
        assert_eq!(cols.names().next(), Some("i8"));
    }

    #[test]
    fn an_unsupported_type_names_the_column_and_the_type() {
        let err = Columns::from_arrow(&batch(vec![
            ("x", Arc::new(Float64Array::from(vec![1.0])) as ArrayRef),
            (
                "when",
                Arc::new(IntervalDayTimeArray::from(vec![
                    arrow_array::types::IntervalDayTimeType::make_value(1, 2),
                ])),
            ),
        ]))
        .unwrap_err()
        .to_string();
        assert!(
            err.contains(r#"column "when": Arrow type Interval(DayTime) is not supported"#),
            "{err}"
        );
    }

    #[test]
    fn dictionaries_with_repeated_or_null_values_map_to_distinct_labels() {
        let dict = DictionaryArray::<UInt16Type>::try_new(
            UInt16Array::from(vec![Some(0), Some(1), Some(2), Some(3), None]),
            Arc::new(StringArray::from(vec![
                Some("a"),
                Some("b"),
                Some("a"),
                None,
            ])),
        )
        .unwrap();
        let cols = Columns::from_arrow(&batch(vec![("k", Arc::new(dict) as ArrayRef)])).unwrap();
        assert_eq!(
            keys(&cols["k"]),
            (
                vec![0, 1, 0, NULL_CODE, NULL_CODE],
                vec!["a".into(), "b".into()]
            )
        );
    }

    #[test]
    fn a_dictionary_of_numbers_is_refused() {
        let dict = DictionaryArray::<Int8Type>::try_new(
            Int8Array::from(vec![0]),
            Arc::new(Float64Array::from(vec![1.0])),
        )
        .unwrap();
        let err = Columns::from_arrow(&batch(vec![("k", Arc::new(dict) as ArrayRef)]))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains(r#"column "k""#) && err.contains("Float64"),
            "{err}"
        );
    }

    #[test]
    fn a_sliced_array_converts_only_its_rows() {
        let a = Float64Array::from(vec![Some(1.0), None, Some(3.0), Some(4.0)]).slice(1, 3);
        let s = StringArray::from(vec!["a", "b", "c", "d"]).slice(1, 3);
        let cols = Columns::from_arrow(&batch(vec![
            ("a", Arc::new(a) as ArrayRef),
            ("s", Arc::new(s)),
        ]))
        .unwrap();
        assert!(same(&f64s(&cols["a"]), &[f64::NAN, 3.0, 4.0]));
        assert_eq!(
            keys(&cols["s"]),
            (vec![0, 1, 2], vec!["b".into(), "c".into(), "d".into()])
        );
    }

    #[test]
    fn try_from_and_the_empty_batch() {
        let empty = RecordBatch::new_empty(Arc::new(Schema::new(vec![Field::new(
            "x",
            DataType::Float64,
            true,
        )])));
        let cols = Columns::try_from(&empty).unwrap();
        assert_eq!(cols.len(), 0);
        assert_eq!(cols["x"].len(), 0);
    }
}
