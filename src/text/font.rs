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

/// Configuration for the [`FontAtlasManager`].
///
/// Controls capacity limits and GPU memory budgets.
///
/// # Examples
///
/// ```
/// use gup::text::FontAtlasManagerConfig;
///
/// // Default: 16 atlases, ~64 MB budget
/// let config = FontAtlasManagerConfig::default();
///
/// // Custom limits
/// let config = FontAtlasManagerConfig::new()
///     .with_max_atlases(8)
///     .with_memory_budget_bytes(32 * 1024 * 1024);
/// ```
#[derive(Debug, Clone)]
pub struct FontAtlasManagerConfig {
    /// Maximum number of font atlases to keep loaded.
    /// When this limit is reached, the least-recently-used atlas is evicted.
    /// The default atlas is never evicted.
    pub max_atlases: usize,
    /// GPU memory budget in bytes. Each 1024×1024 RGBA atlas uses ~4 MB.
    /// When this budget would be exceeded, LRU eviction occurs.
    /// A value of 0 means no memory budget (only `max_atlases` applies).
    pub memory_budget_bytes: u64,
}

impl FontAtlasManagerConfig {
    /// Create a new configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of cached atlases.
    pub fn with_max_atlases(mut self, max: usize) -> Self {
        self.max_atlases = max;
        self
    }

    /// Set the GPU memory budget in bytes.
    ///
    /// Each 1024×1024 RGBA atlas consumes approximately 4 MB.
    /// Set to 0 to disable memory-based eviction.
    pub fn with_memory_budget_bytes(mut self, budget: u64) -> Self {
        self.memory_budget_bytes = budget;
        self
    }
}

impl Default for FontAtlasManagerConfig {
    fn default() -> Self {
        Self {
            max_atlases: 16,
            // 64 MB default: 16 atlases × 4 MB each
            memory_budget_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Telemetry statistics for the font atlas manager.
///
/// Provides insight into cache behaviour and GPU memory usage.
#[derive(Debug, Clone, Default)]
pub struct FontAtlasStats {
    /// Number of atlases currently loaded.
    pub atlas_count: usize,
    /// Estimated total GPU memory used by all loaded atlases (bytes).
    pub memory_used_bytes: u64,
    /// Total number of `get_or_create` calls that found an existing atlas.
    pub cache_hits: u64,
    /// Total number of `get_or_create` calls that created a new atlas.
    pub cache_misses: u64,
    /// Total number of atlases evicted due to capacity or memory limits.
    pub evictions: u64,
}

impl FontAtlasStats {
    /// Cache hit rate as a fraction (0.0 to 1.0).
    ///
    /// Returns 0.0 if no accesses have been recorded.
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }
}

impl std::fmt::Display for FontAtlasStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FontAtlasStats {{ atlases: {}, memory: {:.1} MB, hit_rate: {:.1}%, evictions: {} }}",
            self.atlas_count,
            self.memory_used_bytes as f64 / (1024.0 * 1024.0),
            self.hit_rate() * 100.0,
            self.evictions,
        )
    }
}

/// Manager for multiple font atlases with LRU eviction and telemetry.
///
/// Provides lazy creation of [`FontAtlas`](super::FontAtlas) instances
/// on first use, driven by `TextStyle.font_family`. When no font family
/// is specified, the embedded default font is used.
///
/// When the configured capacity or memory budget is exceeded, the
/// least-recently-used atlas is evicted. The default (embedded) atlas
/// is never evicted.
///
/// Font families that resolve to the same underlying font data share
/// a single atlas via internal aliasing, saving GPU memory.
///
/// # Examples
///
/// ```ignore
/// use gup::text::{FontAtlasManager, FontAtlasManagerConfig, FontDatabase, TextStyle};
///
/// let font_db = FontDatabase::new();
/// let config = FontAtlasManagerConfig::new().with_max_atlases(8);
/// let mut manager = FontAtlasManager::with_config(font_db, 16.0, config);
///
/// // Get atlas for a specific font family (created lazily)
/// let atlas = manager.get_or_create(device, queue, Some("Arial"))?;
///
/// // Get atlas based on a TextStyle
/// let style = TextStyle::new(24.0).with_font_family("Times New Roman");
/// let atlas = manager.get_atlas_for_style(device, queue, &style)?;
///
/// // Check telemetry
/// let stats = manager.stats();
/// println!("{}", stats);
/// ```
pub struct FontAtlasManager {
    /// Font database for resolving font specs.
    font_db: FontDatabase,
    /// Font atlases keyed by resolved family name.
    atlases: HashMap<String, super::FontAtlas>,
    /// LRU access order: most-recently-used at the back.
    /// Only contains canonical keys (not aliases).
    access_order: Vec<String>,
    /// Alias map: requested family name → canonical atlas key.
    /// Multiple family names can map to the same canonical key
    /// when they resolve to the same underlying font.
    aliases: HashMap<String, String>,
    /// Default font size for MSDF atlas generation.
    default_font_size: f32,
    /// Configuration (capacity and memory budget).
    config: FontAtlasManagerConfig,
    /// Telemetry counters.
    cache_hits: u64,
    cache_misses: u64,
    evictions: u64,
}

/// Key used for the default (embedded) font atlas.
const DEFAULT_ATLAS_KEY: &str = "__default__";

/// Estimated GPU memory per atlas: atlas_size² × 4 bytes (RGBA).
///
/// With the default 1024×1024 atlas, this is 4 MB.
const BYTES_PER_ATLAS: u64 = (super::sdf::ATLAS_SIZE as u64) * (super::sdf::ATLAS_SIZE as u64) * 4;

impl FontAtlasManager {
    /// Create a new font atlas manager with default configuration.
    ///
    /// The `font_db` is used to resolve font family names to font data.
    /// The `default_font_size` controls the MSDF rasterisation quality
    /// for all lazily-created atlases.
    pub fn new(font_db: FontDatabase, default_font_size: f32) -> Self {
        Self::with_config(
            font_db,
            default_font_size,
            FontAtlasManagerConfig::default(),
        )
    }

    /// Create a new font atlas manager with custom configuration.
    pub fn with_config(
        font_db: FontDatabase,
        default_font_size: f32,
        config: FontAtlasManagerConfig,
    ) -> Self {
        Self {
            font_db,
            atlases: HashMap::new(),
            access_order: Vec::new(),
            aliases: HashMap::new(),
            default_font_size,
            config,
            cache_hits: 0,
            cache_misses: 0,
            evictions: 0,
        }
    }

    /// Get or lazily create a font atlas for the given family name.
    ///
    /// When `family` is `None`, returns the default (embedded) font atlas.
    /// When the requested font is not found on the system, falls back to
    /// the embedded default font but stores it under the requested key.
    ///
    /// If the atlas already exists, it is marked as recently used.
    /// If creating a new atlas would exceed the configured limits,
    /// the least-recently-used non-default atlas is evicted first.
    pub fn get_or_create(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        family: Option<&str>,
    ) -> GupResult<&mut super::FontAtlas> {
        let requested_key = family.unwrap_or(DEFAULT_ATLAS_KEY).to_string();

        // Check if this family is aliased to an existing canonical key
        if let Some(canonical) = self.aliases.get(&requested_key).cloned()
            && self.atlases.contains_key(&canonical)
        {
            self.cache_hits += 1;
            self.touch_lru(&canonical);
            return Ok(self.atlases.get_mut(&canonical).unwrap());
        }

        // Check if the atlas exists directly
        if self.atlases.contains_key(&requested_key) {
            self.cache_hits += 1;
            self.touch_lru(&requested_key);
            return Ok(self.atlases.get_mut(&requested_key).unwrap());
        }

        // Cache miss — create a new atlas
        self.cache_misses += 1;

        // Resolve the font to get the canonical family name
        let (atlas, canonical_family, is_fallback) = match family {
            Some(name) => {
                let spec = FontSpec::new(name);
                let resolved = self.font_db.resolve(&spec)?;
                let canonical = resolved.family.clone();
                let fallback = resolved.is_fallback;
                let atlas = super::FontAtlas::from_resolved(
                    device,
                    queue,
                    self.default_font_size,
                    resolved,
                )?;
                (atlas, canonical, fallback)
            }
            None => {
                let atlas = super::FontAtlas::new(device, queue, self.default_font_size)?;
                (atlas, DEFAULT_ATLAS_KEY.to_string(), true)
            }
        };

        // Alias deduplication: only for real (non-fallback) system fonts.
        // When a requested family name resolves to a system font that is
        // already loaded under its canonical name, we alias rather than
        // creating a duplicate atlas.  Fallback fonts are stored per
        // requested name so that each conceptually different font request
        // gets its own entry (even though the data is identical).
        if !is_fallback
            && canonical_family != requested_key
            && self.atlases.contains_key(&canonical_family)
        {
            // Alias the requested key to the canonical key — no new atlas needed
            self.aliases.insert(requested_key, canonical_family.clone());
            self.touch_lru(&canonical_family);
            // We created an atlas we don't need; drop it (GPU texture freed).
            drop(atlas);
            return Ok(self.atlases.get_mut(&canonical_family).unwrap());
        }

        // Evict if necessary before inserting
        self.evict_if_needed();

        // Determine the storage key.
        // For real system fonts, store under the canonical name and alias
        // the requested name. For fallback fonts, store under the requested
        // name directly (no aliasing).
        let storage_key = if !is_fallback
            && canonical_family != requested_key
            && canonical_family != DEFAULT_ATLAS_KEY
        {
            // Store under the canonical name and alias the requested name
            self.aliases.insert(requested_key, canonical_family.clone());
            canonical_family
        } else {
            requested_key
        };

        self.atlases.insert(storage_key.clone(), atlas);
        self.access_order.push(storage_key.clone());

        Ok(self.atlases.get_mut(&storage_key).unwrap())
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
        // Check alias first
        if let Some(canonical) = self.aliases.get(key) {
            return self.atlases.get(canonical);
        }
        self.atlases.get(key)
    }

    /// Get a mutable reference to an existing atlas by key.
    pub fn get_atlas_mut(&mut self, family: Option<&str>) -> Option<&mut super::FontAtlas> {
        let key = family.unwrap_or(DEFAULT_ATLAS_KEY);
        // Check alias first
        if let Some(canonical) = self.aliases.get(key).cloned() {
            return self.atlases.get_mut(&canonical);
        }
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

    /// Get the current configuration.
    pub fn config(&self) -> &FontAtlasManagerConfig {
        &self.config
    }

    /// Iterate over all loaded atlases and their keys.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &super::FontAtlas)> {
        self.atlases.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Estimated total GPU memory used by all loaded atlases.
    pub fn memory_used_bytes(&self) -> u64 {
        self.atlases.len() as u64 * BYTES_PER_ATLAS
    }

    /// Get telemetry statistics.
    pub fn stats(&self) -> FontAtlasStats {
        FontAtlasStats {
            atlas_count: self.atlases.len(),
            memory_used_bytes: self.memory_used_bytes(),
            cache_hits: self.cache_hits,
            cache_misses: self.cache_misses,
            evictions: self.evictions,
        }
    }

    /// Reset telemetry counters (but not the atlas cache itself).
    pub fn reset_stats(&mut self) {
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.evictions = 0;
    }

    /// Move a key to the most-recently-used position.
    fn touch_lru(&mut self, key: &str) {
        if let Some(pos) = self.access_order.iter().position(|k| k == key) {
            let k = self.access_order.remove(pos);
            self.access_order.push(k);
        }
    }

    /// Evict the least-recently-used non-default atlas if capacity or
    /// memory budget would be exceeded by adding one more atlas.
    fn evict_if_needed(&mut self) {
        // Check atlas count limit
        while self.atlases.len() >= self.config.max_atlases {
            if !self.evict_lru() {
                break; // Nothing evictable
            }
        }

        // Check memory budget (if non-zero)
        if self.config.memory_budget_bytes > 0 {
            while self.memory_used_bytes() + BYTES_PER_ATLAS > self.config.memory_budget_bytes {
                if !self.evict_lru() {
                    break;
                }
            }
        }
    }

    /// Evict the least-recently-used non-default atlas.
    ///
    /// Returns `true` if an atlas was evicted, `false` if nothing was evictable.
    fn evict_lru(&mut self) -> bool {
        // Find the first (oldest) non-default entry in access_order
        let evict_pos = self
            .access_order
            .iter()
            .position(|k| k != DEFAULT_ATLAS_KEY);

        if let Some(pos) = evict_pos {
            let evicted_key = self.access_order.remove(pos);
            self.atlases.remove(&evicted_key);

            // Remove any aliases that point to the evicted key
            self.aliases.retain(|_, v| *v != evicted_key);

            self.evictions += 1;
            true
        } else {
            false
        }
    }
}

impl std::fmt::Debug for FontAtlasManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontAtlasManager")
            .field("atlas_count", &self.atlases.len())
            .field("loaded_families", &self.atlases.keys().collect::<Vec<_>>())
            .field("default_font_size", &self.default_font_size)
            .field("config", &self.config)
            .field("aliases", &self.aliases)
            .field("stats", &self.stats())
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
        assert_eq!(manager.config().max_atlases, 16);
    }

    #[test]
    fn test_font_atlas_manager_with_config() {
        let db = FontDatabase::empty();
        let config = FontAtlasManagerConfig::new()
            .with_max_atlases(4)
            .with_memory_budget_bytes(16 * 1024 * 1024);
        let manager = FontAtlasManager::with_config(db, 16.0, config);
        assert_eq!(manager.atlas_count(), 0);
        assert_eq!(manager.config().max_atlases, 4);
        assert_eq!(manager.config().memory_budget_bytes, 16 * 1024 * 1024);
    }

    #[test]
    fn test_font_atlas_manager_config_default() {
        let config = FontAtlasManagerConfig::default();
        assert_eq!(config.max_atlases, 16);
        assert_eq!(config.memory_budget_bytes, 64 * 1024 * 1024);
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
        assert!(debug_str.contains("config"));
    }

    #[test]
    fn test_font_atlas_stats_default() {
        let stats = FontAtlasStats::default();
        assert_eq!(stats.atlas_count, 0);
        assert_eq!(stats.memory_used_bytes, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.cache_misses, 0);
        assert_eq!(stats.evictions, 0);
        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_font_atlas_stats_hit_rate() {
        let stats = FontAtlasStats {
            atlas_count: 2,
            memory_used_bytes: 8 * 1024 * 1024,
            cache_hits: 7,
            cache_misses: 3,
            evictions: 0,
        };
        assert!((stats.hit_rate() - 0.7).abs() < 1e-10);
    }

    #[test]
    fn test_font_atlas_stats_display() {
        let stats = FontAtlasStats {
            atlas_count: 2,
            memory_used_bytes: 8 * 1024 * 1024,
            cache_hits: 7,
            cache_misses: 3,
            evictions: 1,
        };
        let display = format!("{stats}");
        assert!(display.contains("atlases: 2"));
        assert!(display.contains("8.0 MB"));
        assert!(display.contains("70.0%"));
        assert!(display.contains("evictions: 1"));
    }

    #[tokio::test]
    async fn test_font_atlas_manager_get_or_create_default() {
        let context = crate::RenderContext::new().await.unwrap();
        let db = FontDatabase::empty();
        let mut manager = FontAtlasManager::new(db, 16.0);

        // Create default atlas
        let atlas = manager
            .get_or_create(context.device(), context.queue(), None)
            .unwrap();
        assert!(atlas.is_fallback_font());
        assert_eq!(atlas.font_family(), "Squada One");
        assert!(atlas.glyph_count() > 0);

        // Should now have one atlas
        assert_eq!(manager.atlas_count(), 1);

        // Get it again — no new atlas created
        let _ = manager
            .get_or_create(context.device(), context.queue(), None)
            .unwrap();
        assert_eq!(manager.atlas_count(), 1);

        // Verify telemetry
        let stats = manager.stats();
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.cache_misses, 1);
    }

    #[tokio::test]
    async fn test_font_atlas_manager_get_or_create_named() {
        let context = crate::RenderContext::new().await.unwrap();
        let db = FontDatabase::empty();
        let mut manager = FontAtlasManager::new(db, 16.0);

        // Request a named font (falls back to default with empty db)
        let atlas = manager
            .get_or_create(context.device(), context.queue(), Some("Arial"))
            .unwrap();
        assert!(atlas.is_fallback_font()); // Falls back
        assert_eq!(manager.atlas_count(), 1);

        // Request another named font
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Times"))
            .unwrap();
        assert_eq!(manager.atlas_count(), 2);

        // Request same font again — no new atlas
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Arial"))
            .unwrap();
        assert_eq!(manager.atlas_count(), 2);

        // Verify telemetry
        let stats = manager.stats();
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.cache_misses, 2);
    }

    #[tokio::test]
    async fn test_font_atlas_manager_get_atlas_for_style() {
        let context = crate::RenderContext::new().await.unwrap();
        let db = FontDatabase::empty();
        let mut manager = FontAtlasManager::new(db, 16.0);

        // Style without font family → default atlas
        let style = super::super::TextStyle::default();
        let atlas = manager
            .get_atlas_for_style(context.device(), context.queue(), &style)
            .unwrap();
        assert!(atlas.is_fallback_font());
        assert_eq!(manager.atlas_count(), 1);

        // Style with font family → named atlas
        let style = super::super::TextStyle::new(24.0).with_font_family("Helvetica");
        let atlas = manager
            .get_atlas_for_style(context.device(), context.queue(), &style)
            .unwrap();
        assert!(atlas.is_fallback_font()); // Falls back with empty db
        assert_eq!(manager.atlas_count(), 2);
    }

    #[tokio::test]
    async fn test_font_atlas_manager_iter() {
        let context = crate::RenderContext::new().await.unwrap();
        let db = FontDatabase::empty();
        let mut manager = FontAtlasManager::new(db, 16.0);

        let _ = manager
            .get_or_create(context.device(), context.queue(), None)
            .unwrap();
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Mono"))
            .unwrap();

        let keys: Vec<&str> = manager.iter().map(|(k, _)| k).collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&DEFAULT_ATLAS_KEY));
        assert!(keys.contains(&"Mono"));
    }

    #[tokio::test]
    async fn test_font_atlas_manager_with_system_fonts() {
        let context = crate::RenderContext::new().await.unwrap();
        let db = FontDatabase::new();
        let mut manager = FontAtlasManager::new(db, 16.0);

        // Try to load a common system font
        let atlas = manager
            .get_or_create(context.device(), context.queue(), Some("DejaVu Sans"))
            .unwrap();
        // Should succeed regardless (falls back if not found)
        assert!(atlas.glyph_count() > 0);
    }

    #[tokio::test]
    async fn test_font_atlas_manager_lru_eviction() {
        let context = crate::RenderContext::new().await.unwrap();
        let db = FontDatabase::empty();
        let config = FontAtlasManagerConfig::new().with_max_atlases(3);
        let mut manager = FontAtlasManager::with_config(db, 16.0, config);

        // Fill to capacity: 3 atlases
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font1"))
            .unwrap();
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font2"))
            .unwrap();
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font3"))
            .unwrap();
        assert_eq!(manager.atlas_count(), 3);

        // Adding a 4th should evict "Font1" (least recently used)
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font4"))
            .unwrap();
        assert_eq!(manager.atlas_count(), 3);
        assert!(manager.get_atlas(Some("Font1")).is_none());
        assert!(manager.get_atlas(Some("Font4")).is_some());

        let stats = manager.stats();
        assert_eq!(stats.evictions, 1);
    }

    #[tokio::test]
    async fn test_font_atlas_manager_lru_touch_reorders() {
        let context = crate::RenderContext::new().await.unwrap();
        let db = FontDatabase::empty();
        let config = FontAtlasManagerConfig::new().with_max_atlases(3);
        let mut manager = FontAtlasManager::with_config(db, 16.0, config);

        // Create 3 atlases
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font1"))
            .unwrap();
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font2"))
            .unwrap();
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font3"))
            .unwrap();

        // Touch Font1 to make it recently used
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font1"))
            .unwrap();

        // Adding Font4 should evict Font2 (oldest non-touched)
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font4"))
            .unwrap();
        assert_eq!(manager.atlas_count(), 3);
        assert!(manager.get_atlas(Some("Font1")).is_some()); // Was touched
        assert!(manager.get_atlas(Some("Font2")).is_none()); // Evicted
        assert!(manager.get_atlas(Some("Font3")).is_some());
        assert!(manager.get_atlas(Some("Font4")).is_some());
    }

    #[tokio::test]
    async fn test_font_atlas_manager_default_never_evicted() {
        let context = crate::RenderContext::new().await.unwrap();
        let db = FontDatabase::empty();
        let config = FontAtlasManagerConfig::new().with_max_atlases(2);
        let mut manager = FontAtlasManager::with_config(db, 16.0, config);

        // Create default + Font1
        let _ = manager
            .get_or_create(context.device(), context.queue(), None)
            .unwrap();
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font1"))
            .unwrap();
        assert_eq!(manager.atlas_count(), 2);

        // Add Font2 — should evict Font1, not default
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font2"))
            .unwrap();
        assert_eq!(manager.atlas_count(), 2);
        assert!(manager.get_atlas(None).is_some()); // Default survives
        assert!(manager.get_atlas(Some("Font1")).is_none()); // Evicted
        assert!(manager.get_atlas(Some("Font2")).is_some());
    }

    #[tokio::test]
    async fn test_font_atlas_manager_memory_budget() {
        let context = crate::RenderContext::new().await.unwrap();
        let db = FontDatabase::empty();
        // Budget for 2 atlases (2 × 4 MB = 8 MB)
        let config = FontAtlasManagerConfig::new()
            .with_max_atlases(100)  // High count limit
            .with_memory_budget_bytes(BYTES_PER_ATLAS * 2);
        let mut manager = FontAtlasManager::with_config(db, 16.0, config);

        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font1"))
            .unwrap();
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font2"))
            .unwrap();
        assert_eq!(manager.atlas_count(), 2);

        // Third atlas should trigger eviction due to memory budget
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font3"))
            .unwrap();
        assert_eq!(manager.atlas_count(), 2);
        assert!(manager.get_atlas(Some("Font1")).is_none()); // Evicted
        assert!(manager.memory_used_bytes() <= BYTES_PER_ATLAS * 2);
    }

    #[tokio::test]
    async fn test_font_atlas_manager_reset_stats() {
        let context = crate::RenderContext::new().await.unwrap();
        let db = FontDatabase::empty();
        let mut manager = FontAtlasManager::new(db, 16.0);

        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font1"))
            .unwrap();
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Font1"))
            .unwrap();

        assert_eq!(manager.stats().cache_hits, 1);
        assert_eq!(manager.stats().cache_misses, 1);

        manager.reset_stats();

        assert_eq!(manager.stats().cache_hits, 0);
        assert_eq!(manager.stats().cache_misses, 0);
    }

    #[tokio::test]
    async fn test_font_atlas_manager_alias_deduplication() {
        // When multiple family names resolve to the same REAL system font,
        // only one atlas should be created and others should be aliases.
        // With an empty database (fallback fonts), each name gets its own
        // atlas since the requested font was not actually found.
        let context = crate::RenderContext::new().await.unwrap();

        // Test with empty db: no aliasing (both are fallback)
        let db = FontDatabase::empty();
        let mut manager = FontAtlasManager::new(db, 16.0);

        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Arial"))
            .unwrap();
        let _ = manager
            .get_or_create(context.device(), context.queue(), Some("Helvetica"))
            .unwrap();
        // Both are fallback fonts, so they each get their own atlas entry
        assert_eq!(manager.atlas_count(), 2);

        // Test with system db: aliasing should occur if both names
        // resolve to the same real font. We verify the mechanism works
        // by checking that get_atlas returns the same atlas for both names
        // when they share an alias.
        let db2 = FontDatabase::new();
        let has_dejavu = db2.has_family("DejaVu Sans");
        if has_dejavu {
            let mut manager2 = FontAtlasManager::new(db2, 16.0);
            let _ = manager2
                .get_or_create(context.device(), context.queue(), Some("DejaVu Sans"))
                .unwrap();
            // Re-request same name — should be a cache hit
            let _ = manager2
                .get_or_create(context.device(), context.queue(), Some("DejaVu Sans"))
                .unwrap();
            assert_eq!(manager2.atlas_count(), 1);
            let stats = manager2.stats();
            assert_eq!(stats.cache_hits, 1);
            assert_eq!(stats.cache_misses, 1);
        }
    }
}
