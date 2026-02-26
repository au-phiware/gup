// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Font specification and system font loading.
//!
//! Provides types for specifying desired fonts by family name, weight, and style,
//! and a font database for resolving these specifications to actual font data
//! from the operating system's installed fonts.

use crate::error::{GupError, GupResult};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Font weight specification.
///
/// Maps to standard CSS/OpenType font weight values (100–900).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontWeight {
    /// Thin (100)
    Thin,
    /// Extra-light (200)
    ExtraLight,
    /// Light (300)
    Light,
    /// Regular/Normal (400)
    #[default]
    Regular,
    /// Medium (500)
    Medium,
    /// Semi-bold (600)
    SemiBold,
    /// Bold (700)
    Bold,
    /// Extra-bold (800)
    ExtraBold,
    /// Black (900)
    Black,
}

impl FontWeight {
    /// Get the numeric weight value (100–900).
    pub fn value(&self) -> u16 {
        match self {
            FontWeight::Thin => 100,
            FontWeight::ExtraLight => 200,
            FontWeight::Light => 300,
            FontWeight::Regular => 400,
            FontWeight::Medium => 500,
            FontWeight::SemiBold => 600,
            FontWeight::Bold => 700,
            FontWeight::ExtraBold => 800,
            FontWeight::Black => 900,
        }
    }

    /// Create from a numeric value, snapping to the nearest standard weight.
    pub fn from_value(value: u16) -> Self {
        match value {
            0..=150 => FontWeight::Thin,
            151..=250 => FontWeight::ExtraLight,
            251..=350 => FontWeight::Light,
            351..=450 => FontWeight::Regular,
            451..=550 => FontWeight::Medium,
            551..=650 => FontWeight::SemiBold,
            651..=750 => FontWeight::Bold,
            751..=850 => FontWeight::ExtraBold,
            _ => FontWeight::Black,
        }
    }
}

impl From<FontWeight> for fontdb::Weight {
    fn from(w: FontWeight) -> Self {
        fontdb::Weight(w.value())
    }
}

/// Font style specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontStyle {
    /// Normal (upright) style
    #[default]
    Normal,
    /// Italic style
    Italic,
    /// Oblique style
    Oblique,
}

impl From<FontStyle> for fontdb::Style {
    fn from(s: FontStyle) -> Self {
        match s {
            FontStyle::Normal => fontdb::Style::Normal,
            FontStyle::Italic => fontdb::Style::Italic,
            FontStyle::Oblique => fontdb::Style::Oblique,
        }
    }
}

/// Font specification for selecting a font by family, weight, and style.
///
/// # Examples
///
/// ```
/// use gup::text::FontSpec;
/// use gup::text::font::{FontWeight, FontStyle};
///
/// // Simple family name
/// let spec = FontSpec::new("Arial");
///
/// // With weight and style
/// let spec = FontSpec::new("Times New Roman")
///     .with_weight(FontWeight::Bold)
///     .with_style(FontStyle::Italic);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FontSpec {
    /// Font family name (e.g., "Arial", "Times New Roman", "Courier New")
    pub family: String,
    /// Font weight
    pub weight: FontWeight,
    /// Font style (normal, italic, oblique)
    pub style: FontStyle,
}

impl FontSpec {
    /// Create a new font specification with the given family name.
    ///
    /// Uses regular weight and normal style by default.
    pub fn new(family: &str) -> Self {
        Self {
            family: family.to_string(),
            weight: FontWeight::Regular,
            style: FontStyle::Normal,
        }
    }

    /// Set the font weight.
    pub fn with_weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    /// Set the font style.
    pub fn with_style(mut self, style: FontStyle) -> Self {
        self.style = style;
        self
    }
}

impl Default for FontSpec {
    fn default() -> Self {
        Self {
            family: "sans-serif".to_string(),
            weight: FontWeight::Regular,
            style: FontStyle::Normal,
        }
    }
}

/// Result of a font resolution, containing the font data and metadata.
#[derive(Debug, Clone)]
pub struct ResolvedFont {
    /// Raw font data bytes (TTF/OTF)
    pub data: Arc<Vec<u8>>,
    /// The family name as reported by the font file
    pub family: String,
    /// Whether this is the embedded fallback font
    pub is_fallback: bool,
}

/// Database for resolving font specifications to font data.
///
/// Wraps fontdb to provide system font discovery with caching and
/// fallback to the embedded default font.
///
/// # Thread Safety
///
/// `FontDatabase` is thread-safe and can be shared across threads. Font
/// resolution results are cached to avoid repeated filesystem access.
pub struct FontDatabase {
    db: fontdb::Database,
    /// Cache of resolved font data keyed by FontSpec
    cache: Mutex<HashMap<FontSpec, ResolvedFont>>,
}

impl FontDatabase {
    /// Create a new font database populated with system fonts.
    ///
    /// This scans the system font directories and loads all available
    /// font metadata. On Linux, this reads from fontconfig directories.
    /// On macOS, from `/Library/Fonts` and `~/Library/Fonts`.
    /// On Windows, from `C:\Windows\Fonts`.
    pub fn new() -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        Self {
            db,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Create a font database without loading system fonts.
    ///
    /// Useful for testing or WASM environments where system fonts
    /// are not available. Font resolution will always fall back to
    /// the embedded font.
    pub fn empty() -> Self {
        Self {
            db: fontdb::Database::new(),
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Resolve a font specification to font data.
    ///
    /// Returns the matching system font if found, otherwise falls back
    /// to the embedded default font. Results are cached for subsequent
    /// lookups with the same specification.
    pub fn resolve(&self, spec: &FontSpec) -> GupResult<ResolvedFont> {
        // Check cache first
        {
            let cache = self
                .cache
                .lock()
                .map_err(|e| GupError::resource_error(format!("Font cache lock poisoned: {e}")))?;
            if let Some(resolved) = cache.get(spec) {
                return Ok(resolved.clone());
            }
        }

        // Try to find a matching system font
        let resolved = self.resolve_uncached(spec);

        // Cache the result
        {
            let mut cache = self
                .cache
                .lock()
                .map_err(|e| GupError::resource_error(format!("Font cache lock poisoned: {e}")))?;
            cache.insert(spec.clone(), resolved.clone());
        }

        Ok(resolved)
    }

    /// Resolve a font specification without caching.
    fn resolve_uncached(&self, spec: &FontSpec) -> ResolvedFont {
        // Build the fontdb query
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(&spec.family)],
            weight: spec.weight.into(),
            style: spec.style.into(),
            ..fontdb::Query::default()
        };

        // Try to find a matching font
        if let Some(id) = self.db.query(&query)
            && let Some((data, _index)) = self.db.face_source(id)
        {
            if let fontdb::Source::Binary(arc_data) = data {
                let bytes: &[u8] = (*arc_data).as_ref();
                let family = self
                    .db
                    .face(id)
                    .map(|f| {
                        f.families
                            .first()
                            .map(|(name, _)| name.clone())
                            .unwrap_or_else(|| spec.family.clone())
                    })
                    .unwrap_or_else(|| spec.family.clone());

                return ResolvedFont {
                    data: Arc::new(bytes.to_vec()),
                    family,
                    is_fallback: false,
                };
            }
            // Font is file-based, read it
            if let fontdb::Source::File(path) = data
                && let Ok(file_data) = std::fs::read(&path)
            {
                let family = self
                    .db
                    .face(id)
                    .map(|f| {
                        f.families
                            .first()
                            .map(|(name, _)| name.clone())
                            .unwrap_or_else(|| spec.family.clone())
                    })
                    .unwrap_or_else(|| spec.family.clone());

                return ResolvedFont {
                    data: Arc::new(file_data),
                    family,
                    is_fallback: false,
                };
            }
        }

        // Fall back to embedded font
        Self::embedded_fallback()
    }

    /// Resolve font data from raw bytes.
    ///
    /// This skips system font lookup entirely and uses the provided
    /// font data directly. Falls back to the embedded font if the
    /// provided data cannot be parsed.
    pub fn resolve_from_data(data: Vec<u8>) -> GupResult<ResolvedFont> {
        // Validate the font data can be parsed
        let family = ttf_parser::Face::parse(&data, 0)
            .map(|face| {
                face.names()
                    .into_iter()
                    .find(|name| name.name_id == ttf_parser::name_id::FAMILY)
                    .and_then(|name| name.to_string())
                    .unwrap_or_else(|| "Unknown".to_string())
            })
            .map_err(|e| GupError::resource_error(format!("Failed to parse font data: {e:?}")))?;

        Ok(ResolvedFont {
            data: Arc::new(data),
            family,
            is_fallback: false,
        })
    }

    /// Get the embedded fallback font.
    pub fn embedded_fallback() -> ResolvedFont {
        let data = include_bytes!("../../assets/fonts/default.ttf");
        ResolvedFont {
            data: Arc::new(data.to_vec()),
            family: "Squada One".to_string(),
            is_fallback: true,
        }
    }

    /// List available font families on the system.
    ///
    /// Returns a sorted, deduplicated list of font family names.
    pub fn list_families(&self) -> Vec<String> {
        let mut families: Vec<String> = self
            .db
            .faces()
            .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
            .collect();
        families.sort();
        families.dedup();
        families
    }

    /// Check if a specific font family is available on the system.
    pub fn has_family(&self, family: &str) -> bool {
        self.db.faces().any(|face| {
            face.families
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(family))
        })
    }

    /// Get the number of fonts in the database.
    pub fn font_count(&self) -> usize {
        self.db.faces().count()
    }

    /// Clear the resolution cache.
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }
}

impl Default for FontDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for FontDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cache_size = self.cache.lock().map(|c| c.len()).unwrap_or(0);
        f.debug_struct("FontDatabase")
            .field("font_count", &self.font_count())
            .field("cache_entries", &cache_size)
            .finish()
    }
}

/// Manager for multiple font atlases, keyed by font family name.
///
/// Provides lazy creation of [`FontAtlas`](super::FontAtlas) instances
/// on first use, driven by `TextStyle.font_family`. When no font family
/// is specified, the embedded default font is used.
///
/// # Examples
///
/// ```ignore
/// use gup::text::{FontAtlasManager, FontDatabase, TextStyle};
///
/// let font_db = FontDatabase::new();
/// let mut manager = FontAtlasManager::new(font_db, 16.0);
///
/// // Get atlas for a specific font family (created lazily)
/// let atlas = manager.get_or_create(device, queue, Some("Arial"))?;
///
/// // Get atlas based on a TextStyle
/// let style = TextStyle::new(24.0).with_font_family("Times New Roman");
/// let atlas = manager.get_atlas_for_style(device, queue, &style)?;
/// ```
pub struct FontAtlasManager {
    /// Font database for resolving font specs.
    font_db: FontDatabase,
    /// Font atlases keyed by resolved family name.
    atlases: HashMap<String, super::FontAtlas>,
    /// Default font size for MSDF atlas generation.
    default_font_size: f32,
}

/// Key used for the default (embedded) font atlas.
const DEFAULT_ATLAS_KEY: &str = "__default__";

impl FontAtlasManager {
    /// Create a new font atlas manager.
    ///
    /// The `font_db` is used to resolve font family names to font data.
    /// The `default_font_size` controls the MSDF rasterisation quality
    /// for all lazily-created atlases.
    pub fn new(font_db: FontDatabase, default_font_size: f32) -> Self {
        Self {
            font_db,
            atlases: HashMap::new(),
            default_font_size,
        }
    }

    /// Get or lazily create a font atlas for the given family name.
    ///
    /// When `family` is `None`, returns the default (embedded) font atlas.
    /// When the requested font is not found on the system, falls back to
    /// the embedded default font but stores it under the requested key.
    pub fn get_or_create(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        family: Option<&str>,
    ) -> GupResult<&mut super::FontAtlas> {
        let key = family.unwrap_or(DEFAULT_ATLAS_KEY).to_string();

        if !self.atlases.contains_key(&key) {
            let atlas = match family {
                Some(name) => {
                    let spec = FontSpec::new(name);
                    super::FontAtlas::with_font(
                        device,
                        queue,
                        self.default_font_size,
                        &spec,
                        &self.font_db,
                    )?
                }
                None => super::FontAtlas::new(device, queue, self.default_font_size)?,
            };
            self.atlases.insert(key.clone(), atlas);
        }

        Ok(self.atlases.get_mut(&key).unwrap())
    }

    /// Get or lazily create the atlas appropriate for a [`TextStyle`](super::TextStyle).
    ///
    /// Uses `TextStyle.font_family` to select the atlas, falling back to
    /// the default atlas when `font_family` is `None`.
    pub fn get_atlas_for_style(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        style: &super::TextStyle,
    ) -> GupResult<&mut super::FontAtlas> {
        self.get_or_create(device, queue, style.font_family.as_deref())
    }

    /// Return the atlas key for a given font family.
    ///
    /// This is the key used internally to store and look up atlases.
    pub fn atlas_key(family: Option<&str>) -> String {
        family.unwrap_or(DEFAULT_ATLAS_KEY).to_string()
    }

    /// Get an existing atlas by key without creating one.
    pub fn get_atlas(&self, family: Option<&str>) -> Option<&super::FontAtlas> {
        let key = family.unwrap_or(DEFAULT_ATLAS_KEY);
        self.atlases.get(key)
    }

    /// Get a mutable reference to an existing atlas by key.
    pub fn get_atlas_mut(&mut self, family: Option<&str>) -> Option<&mut super::FontAtlas> {
        let key = family.unwrap_or(DEFAULT_ATLAS_KEY);
        self.atlases.get_mut(key)
    }

    /// Get the number of font atlases currently loaded.
    pub fn atlas_count(&self) -> usize {
        self.atlases.len()
    }

    /// Get the font database.
    pub fn font_db(&self) -> &FontDatabase {
        &self.font_db
    }

    /// Iterate over all loaded atlases and their keys.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &super::FontAtlas)> {
        self.atlases.iter().map(|(k, v)| (k.as_str(), v))
    }
}

impl std::fmt::Debug for FontAtlasManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontAtlasManager")
            .field("atlas_count", &self.atlases.len())
            .field(
                "loaded_families",
                &self.atlases.keys().collect::<Vec<_>>(),
            )
            .field("default_font_size", &self.default_font_size)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_weight_values() {
        assert_eq!(FontWeight::Thin.value(), 100);
        assert_eq!(FontWeight::Regular.value(), 400);
        assert_eq!(FontWeight::Bold.value(), 700);
        assert_eq!(FontWeight::Black.value(), 900);
    }

    #[test]
    fn test_font_weight_from_value() {
        assert_eq!(FontWeight::from_value(100), FontWeight::Thin);
        assert_eq!(FontWeight::from_value(400), FontWeight::Regular);
        assert_eq!(FontWeight::from_value(700), FontWeight::Bold);
        assert_eq!(FontWeight::from_value(900), FontWeight::Black);
        // Edge cases
        assert_eq!(FontWeight::from_value(0), FontWeight::Thin);
        assert_eq!(FontWeight::from_value(450), FontWeight::Regular);
        assert_eq!(FontWeight::from_value(451), FontWeight::Medium);
        assert_eq!(FontWeight::from_value(1000), FontWeight::Black);
    }

    #[test]
    fn test_font_weight_default() {
        assert_eq!(FontWeight::default(), FontWeight::Regular);
    }

    #[test]
    fn test_font_style_default() {
        assert_eq!(FontStyle::default(), FontStyle::Normal);
    }

    #[test]
    fn test_font_spec_new() {
        let spec = FontSpec::new("Arial");
        assert_eq!(spec.family, "Arial");
        assert_eq!(spec.weight, FontWeight::Regular);
        assert_eq!(spec.style, FontStyle::Normal);
    }

    #[test]
    fn test_font_spec_builder() {
        let spec = FontSpec::new("Times New Roman")
            .with_weight(FontWeight::Bold)
            .with_style(FontStyle::Italic);
        assert_eq!(spec.family, "Times New Roman");
        assert_eq!(spec.weight, FontWeight::Bold);
        assert_eq!(spec.style, FontStyle::Italic);
    }

    #[test]
    fn test_font_spec_default() {
        let spec = FontSpec::default();
        assert_eq!(spec.family, "sans-serif");
        assert_eq!(spec.weight, FontWeight::Regular);
        assert_eq!(spec.style, FontStyle::Normal);
    }

    #[test]
    fn test_font_spec_equality() {
        let spec1 = FontSpec::new("Arial");
        let spec2 = FontSpec::new("Arial");
        let spec3 = FontSpec::new("Helvetica");
        assert_eq!(spec1, spec2);
        assert_ne!(spec1, spec3);
    }

    #[test]
    fn test_font_spec_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(FontSpec::new("Arial"));
        set.insert(FontSpec::new("Arial"));
        set.insert(FontSpec::new("Helvetica"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_font_database_empty() {
        let db = FontDatabase::empty();
        assert_eq!(db.font_count(), 0);
    }

    #[test]
    fn test_font_database_system() {
        let db = FontDatabase::new();
        // System should have at least some fonts (unless running in a very
        // minimal environment)
        // We don't assert count > 0 because CI might have no fonts
        let _count = db.font_count();
    }

    #[test]
    fn test_font_database_fallback() {
        let db = FontDatabase::empty();
        // With empty database, any query should fall back to embedded font
        let result = db.resolve(&FontSpec::new("NonExistentFont")).unwrap();
        assert!(result.is_fallback);
        assert_eq!(result.family, "Squada One");
        assert!(!result.data.is_empty());
    }

    #[test]
    fn test_font_database_caching() {
        let db = FontDatabase::empty();
        let spec = FontSpec::new("TestFont");

        // First resolution
        let result1 = db.resolve(&spec).unwrap();
        assert!(result1.is_fallback);

        // Second resolution should hit cache
        let result2 = db.resolve(&spec).unwrap();
        assert!(result2.is_fallback);

        // Data should be identical (same Arc)
        assert_eq!(result1.data.len(), result2.data.len());
    }

    #[test]
    fn test_font_database_clear_cache() {
        let db = FontDatabase::empty();
        let spec = FontSpec::new("TestFont");

        // Resolve to populate cache
        let _ = db.resolve(&spec).unwrap();

        // Clear and verify it still works
        db.clear_cache();
        let result = db.resolve(&spec).unwrap();
        assert!(result.is_fallback);
    }

    #[test]
    fn test_font_database_list_families() {
        let db = FontDatabase::empty();
        let families = db.list_families();
        assert!(families.is_empty());

        // System database may have families
        let db = FontDatabase::new();
        let families = db.list_families();
        // Just verify it doesn't panic
        let _ = families.len();
    }

    #[test]
    fn test_font_database_has_family() {
        let db = FontDatabase::empty();
        assert!(!db.has_family("Arial"));
        assert!(!db.has_family("NonExistent"));
    }

    #[test]
    fn test_embedded_fallback() {
        let resolved = FontDatabase::embedded_fallback();
        assert!(resolved.is_fallback);
        assert_eq!(resolved.family, "Squada One");
        // Verify the data is valid TTF
        let face = ttf_parser::Face::parse(&resolved.data, 0);
        assert!(face.is_ok());
    }

    #[test]
    fn test_resolve_from_data() {
        // Use the embedded font data as test data
        let data = include_bytes!("../../assets/fonts/default.ttf").to_vec();
        let result = FontDatabase::resolve_from_data(data).unwrap();
        assert!(!result.is_fallback);
        assert!(!result.data.is_empty());
    }

    #[test]
    fn test_resolve_from_invalid_data() {
        let data = vec![0u8; 100]; // Invalid font data
        let result = FontDatabase::resolve_from_data(data);
        assert!(result.is_err());
    }

    #[test]
    fn test_font_database_debug() {
        let db = FontDatabase::empty();
        let debug_str = format!("{db:?}");
        assert!(debug_str.contains("FontDatabase"));
        assert!(debug_str.contains("font_count"));
        assert!(debug_str.contains("cache_entries"));
    }

    #[test]
    fn test_resolved_font_clone() {
        let resolved = FontDatabase::embedded_fallback();
        let cloned = resolved.clone();
        assert_eq!(resolved.family, cloned.family);
        assert_eq!(resolved.is_fallback, cloned.is_fallback);
        assert_eq!(resolved.data.len(), cloned.data.len());
    }

    #[test]
    fn test_font_weight_into_fontdb() {
        let w: fontdb::Weight = FontWeight::Bold.into();
        assert_eq!(w, fontdb::Weight(700));
    }

    #[test]
    fn test_font_style_into_fontdb() {
        let s: fontdb::Style = FontStyle::Italic.into();
        assert_eq!(s, fontdb::Style::Italic);
    }

    // --- FontAtlasManager tests ---

    #[test]
    fn test_font_atlas_manager_new() {
        let db = FontDatabase::empty();
        let manager = FontAtlasManager::new(db, 16.0);
        assert_eq!(manager.atlas_count(), 0);
    }

    #[test]
    fn test_font_atlas_manager_atlas_key() {
        assert_eq!(FontAtlasManager::atlas_key(None), DEFAULT_ATLAS_KEY);
        assert_eq!(FontAtlasManager::atlas_key(Some("Arial")), "Arial");
    }

    #[test]
    fn test_font_atlas_manager_get_atlas_empty() {
        let db = FontDatabase::empty();
        let manager = FontAtlasManager::new(db, 16.0);
        assert!(manager.get_atlas(None).is_none());
        assert!(manager.get_atlas(Some("Arial")).is_none());
    }

    #[test]
    fn test_font_atlas_manager_debug() {
        let db = FontDatabase::empty();
        let manager = FontAtlasManager::new(db, 16.0);
        let debug_str = format!("{manager:?}");
        assert!(debug_str.contains("FontAtlasManager"));
        assert!(debug_str.contains("atlas_count"));
        assert!(debug_str.contains("default_font_size"));
    }
}
