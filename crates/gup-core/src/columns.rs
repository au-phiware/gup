// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Columnar input (GUP-628): named columns a layer reads directly, with no
//! row type and no closure per row. See [`Plot::add_columns`](crate::Plot::add_columns).

use crate::column::{ColumnSlice, NULL_CODE};
use crate::error::{Error, Result};
use std::sync::Arc;

/// Named columns of equal length: the data of a columnar layer
/// ([`Plot::add_columns`](crate::Plot::add_columns)).
///
/// Columns are shared, not copied: cloning a `Columns` (or binding one
/// column to several channels or layers) clones an [`Arc`]. A `Vec` given
/// to [`with`](Self::with) is copied once into its `Arc`; for the largest
/// columns, collect straight into an `Arc<[f64]>` (one allocation from an
/// iterator of known length) and pass that.
///
/// ```
/// use gup_core::{Column, Columns};
/// # fn main() -> gup_core::Result<()> {
/// let cols = Columns::new()
///     .with("x", vec![0.0, 1.0, 2.0])?
///     .with("y", vec![1.0_f32, 3.0, 2.0])?;
/// assert_eq!(cols.len(), 3);
/// assert!(matches!(cols["y"], Column::F32(_)));
/// assert!(cols.get("z").is_none());
/// // Every column has the first one's length.
/// assert!(cols.clone().with("short", vec![1.0]).is_err());
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, Default)]
pub struct Columns {
    len: usize,
    columns: Vec<(Arc<str>, Column)>,
}

/// One column. Numbers: NaN (or ±∞) is null.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Column {
    /// 64-bit floats.
    F64(Arc<[f64]>),
    /// 32-bit floats.
    F32(Arc<[f32]>),
    /// Categories: dictionary codes and their labels (a [`KeyColumn`]),
    /// for a Band or PointScale axis or a `Categorical` colour.
    Keys(KeyColumn),
}

/// A column of categories, stored as dictionary codes plus the label of
/// each code: how columnar sources already hold them. A plot maps the
/// labels into its scale's dictionary once each and gathers the rows, so
/// no key closure and no string hash runs per row.
///
/// [`NULL_CODE`] marks a null row. A label no row uses stays out of the
/// scale's domain, as an unused key does on the row path.
///
/// ```
/// use gup_core::KeyColumn;
/// let k = KeyColumn::from_labels(["a", "b", "a"]);
/// assert_eq!(k.codes(), [0, 1, 0]);
/// assert_eq!(k.labels().len(), 2);
/// // A code past the labels is refused.
/// assert!(KeyColumn::new(vec![0, 2], vec![std::sync::Arc::<str>::from("a")]).is_err());
/// ```
#[derive(Clone, Debug)]
pub struct KeyColumn {
    codes: Arc<[u32]>,
    labels: Arc<[Arc<str>]>,
}

impl KeyColumn {
    /// Rows `codes`, each indexing `labels` (or [`NULL_CODE`]). `Err` for a
    /// code outside `labels` or a repeated label.
    pub fn new(codes: impl Into<Arc<[u32]>>, labels: impl Into<Arc<[Arc<str>]>>) -> Result<Self> {
        let (codes, labels) = (codes.into(), labels.into());
        let mut seen = std::collections::HashSet::with_capacity(labels.len());
        if let Some(l) = labels.iter().find(|l| !seen.insert(&***l)) {
            return Err(Error::config(
                "key column",
                format!("the label `{l}` is listed twice; labels must be distinct"),
            ));
        }
        if let Some((row, &code)) = codes
            .iter()
            .enumerate()
            .find(|&(_, &c)| c != NULL_CODE && c as usize >= labels.len())
        {
            return Err(Error::config(
                "key column",
                format!(
                    "row {row} has code {code}, but there are only {} labels",
                    labels.len()
                ),
            ));
        }
        Ok(Self { codes, labels })
    }

    /// The labels of `labels`, one per row, coded in first-seen order.
    pub fn from_labels<S: AsRef<str>>(labels: impl IntoIterator<Item = S>) -> Self {
        Self::from_optional_labels(labels.into_iter().map(Some))
    }

    /// Like [`from_labels`](Self::from_labels); `None` is a null row
    /// ([`NULL_CODE`]).
    pub fn from_optional_labels<S: AsRef<str>>(
        labels: impl IntoIterator<Item = Option<S>>,
    ) -> Self {
        let mut dictionary = crate::column::Dictionary::<Arc<str>>::new();
        let codes: Vec<u32> = labels
            .into_iter()
            .map(|l| match l {
                // More than u32::MAX - 1 distinct labels is not a column.
                Some(l) => dictionary
                    .encode_borrowed(l.as_ref())
                    .expect("fewer than u32::MAX distinct labels"),
                None => NULL_CODE,
            })
            .collect();
        Self {
            codes: codes.into(),
            labels: dictionary.labels().into(),
        }
    }

    /// Each row's code.
    pub fn codes(&self) -> &[u32] {
        &self.codes
    }

    /// The label of each code.
    pub fn labels(&self) -> &[Arc<str>] {
        &self.labels
    }

    /// The number of rows.
    pub fn len(&self) -> usize {
        self.codes.len()
    }

    /// Whether there are no rows.
    pub fn is_empty(&self) -> bool {
        self.codes.is_empty()
    }
}

impl From<KeyColumn> for Column {
    fn from(k: KeyColumn) -> Self {
        Self::Keys(k)
    }
}

impl Columns {
    /// No columns.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a column named `name` (replacing one of that name); `Err` if its
    /// length differs from the first column's.
    pub fn with(mut self, name: impl Into<Arc<str>>, column: impl Into<Column>) -> Result<Self> {
        let (name, column) = (name.into(), column.into());
        if self.columns.is_empty() {
            self.len = column.len();
        } else if column.len() != self.len {
            return Err(Error::config(
                "columns",
                format!(
                    "column `{name}` has {} rows, but the columns before it have {}",
                    column.len(),
                    self.len
                ),
            ));
        }
        match self.columns.iter_mut().find(|(n, _)| *n == name) {
            Some((_, c)) => *c = column,
            None => self.columns.push((name, column)),
        }
        Ok(self)
    }

    /// The number of rows (0 without columns).
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there are no rows.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The column named `name`.
    pub fn get(&self, name: &str) -> Option<&Column> {
        self.columns
            .iter()
            .find(|(n, _)| &**n == name)
            .map(|(_, c)| c)
    }

    /// The column names, in the order added.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.columns.iter().map(|(n, _)| &**n)
    }
}

impl std::ops::Index<&str> for Columns {
    type Output = Column;

    /// The column named `name`.
    ///
    /// # Panics
    ///
    /// If there is no such column (the message names it).
    fn index(&self, name: &str) -> &Column {
        self.get(name).unwrap_or_else(|| {
            panic!(
                "no column named `{name}` (columns: {})",
                self.names().collect::<Vec<_>>().join(", ")
            )
        })
    }
}

impl Column {
    /// The number of rows.
    pub fn len(&self) -> usize {
        match self {
            Self::F64(v) => v.len(),
            Self::F32(v) => v.len(),
            Self::Keys(k) => k.len(),
        }
    }

    /// What the column holds, for error messages.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::F64(_) => "an f64 column",
            Self::F32(_) => "an f32 column",
            Self::Keys(_) => "a key column",
        }
    }

    /// Whether there are no rows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Row `i` as an `f64` (NaN for a null), or `None` past the end.
    pub(crate) fn value(&self, i: usize) -> Option<f64> {
        match self {
            Self::F64(v) => v.get(i).copied(),
            Self::F32(v) => v.get(i).map(|&x| f64::from(x)),
            // The source code, which is the scale's only before a remap.
            Self::Keys(k) => k.codes.get(i).map(|&c| match c {
                NULL_CODE => f64::NAN,
                c => f64::from(c),
            }),
        }
    }

    /// The rows, borrowed, as the column store writes them.
    pub(crate) fn slice(&self) -> ColumnSlice<'_> {
        match self {
            Self::F64(v) => ColumnSlice::F64(v),
            Self::F32(v) => ColumnSlice::F32(v),
            Self::Keys(k) => ColumnSlice::Codes(&k.codes),
        }
    }

    /// Every row as an `f64` (a copy).
    pub(crate) fn to_f64(&self) -> Vec<f64> {
        match self {
            Self::F64(v) => v.to_vec(),
            Self::F32(v) => v.iter().map(|&x| f64::from(x)).collect(),
            Self::Keys(k) => (0..k.len())
                .map(|i| self.value(i).unwrap_or(f64::NAN))
                .collect(),
        }
    }
}

impl From<Vec<f64>> for Column {
    fn from(v: Vec<f64>) -> Self {
        Self::F64(v.into())
    }
}

impl From<Vec<f32>> for Column {
    fn from(v: Vec<f32>) -> Self {
        Self::F32(v.into())
    }
}

impl From<Arc<[f64]>> for Column {
    fn from(v: Arc<[f64]>) -> Self {
        Self::F64(v)
    }
}

impl From<Arc<[f32]>> for Column {
    fn from(v: Arc<[f32]>) -> Self {
        Self::F32(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unequal_lengths_are_refused_naming_the_column() {
        let err = Columns::new()
            .with("x", vec![1.0, 2.0])
            .unwrap()
            .with("y", vec![1.0_f32])
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("`y` has 1 rows") && err.contains("have 2"),
            "{err}"
        );
    }

    #[test]
    fn get_index_and_names() {
        let shared: Arc<[f64]> = vec![1.0, 2.0].into();
        let cols = Columns::new()
            .with("a", Arc::clone(&shared))
            .unwrap()
            .with("b", vec![3.0_f32, 4.0])
            .unwrap();
        assert_eq!(cols.names().collect::<Vec<_>>(), ["a", "b"]);
        assert!(cols.get("c").is_none());
        assert_eq!(cols["b"].value(1), Some(4.0));
        assert_eq!(cols["a"].len(), 2);
        // Shared, not copied.
        assert!(matches!(&cols["a"], Column::F64(a) if Arc::ptr_eq(a, &shared)));
        // A same-named column replaces the old one.
        let cols = cols.with("a", vec![9.0, 8.0]).unwrap();
        assert_eq!(cols.names().count(), 2);
        assert_eq!(cols["a"].value(0), Some(9.0));
        assert!(Columns::new().is_empty());
    }

    #[test]
    #[should_panic(expected = "no column named `z` (columns: a)")]
    fn index_panics_with_the_name() {
        let cols = Columns::new().with("a", vec![1.0]).unwrap();
        let _ = &cols["z"];
    }
}
