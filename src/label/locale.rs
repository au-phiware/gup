// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Locale-aware formatting support for labels.

use std::collections::HashMap;

/// Locale information for number and date formatting.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Locale {
    /// Language code (e.g., "en", "fr", "de")
    pub language: String,
    /// Country code (e.g., "US", "GB", "FR")
    pub country: Option<String>,
    /// Decimal separator character
    pub decimal_separator: char,
    /// Thousands separator character
    pub thousands_separator: char,
    /// Currency symbol
    pub currency_symbol: String,
    /// Whether currency symbol comes before value
    pub currency_before: bool,
}

impl Default for Locale {
    fn default() -> Self {
        Self::en_us()
    }
}

impl Locale {
    /// Create US English locale.
    pub fn en_us() -> Self {
        Self {
            language: "en".to_string(),
            country: Some("US".to_string()),
            decimal_separator: '.',
            thousands_separator: ',',
            currency_symbol: "$".to_string(),
            currency_before: true,
        }
    }

    /// Create UK English locale.
    pub fn en_gb() -> Self {
        Self {
            language: "en".to_string(),
            country: Some("GB".to_string()),
            decimal_separator: '.',
            thousands_separator: ',',
            currency_symbol: "£".to_string(),
            currency_before: true,
        }
    }

    /// Create French locale.
    pub fn fr_fr() -> Self {
        Self {
            language: "fr".to_string(),
            country: Some("FR".to_string()),
            decimal_separator: ',',
            thousands_separator: ' ',
            currency_symbol: "€".to_string(),
            currency_before: false,
        }
    }

    /// Create German locale.
    pub fn de_de() -> Self {
        Self {
            language: "de".to_string(),
            country: Some("DE".to_string()),
            decimal_separator: ',',
            thousands_separator: '.',
            currency_symbol: "€".to_string(),
            currency_before: false,
        }
    }

    /// Create Japanese locale.
    pub fn ja_jp() -> Self {
        Self {
            language: "ja".to_string(),
            country: Some("JP".to_string()),
            decimal_separator: '.',
            thousands_separator: ',',
            currency_symbol: "¥".to_string(),
            currency_before: true,
        }
    }

    /// Get locale identifier string.
    pub fn identifier(&self) -> String {
        if let Some(ref country) = self.country {
            format!("{}_{}", self.language, country)
        } else {
            self.language.clone()
        }
    }

    /// Format number with this locale's conventions.
    pub fn format_number(&self, value: f64, precision: usize) -> String {
        // Split into integer and decimal parts
        let formatted = format!("{value:.precision$}");
        let parts: Vec<&str> = formatted.split('.').collect();
        let integer_part = parts[0];
        let decimal_part = if parts.len() > 1 { parts[1] } else { "" };

        // Format integer part with thousands separators
        let formatted_integer = self.format_integer_with_separators(integer_part);

        // Combine with locale-specific decimal separator
        if !decimal_part.is_empty() && precision > 0 {
            format!(
                "{}{}{}",
                formatted_integer, self.decimal_separator, decimal_part
            )
        } else {
            formatted_integer
        }
    }

    /// Format currency value.
    pub fn format_currency(&self, value: f64, precision: usize) -> String {
        let number = self.format_number(value, precision);

        if self.currency_before {
            format!("{}{}", self.currency_symbol, number)
        } else {
            format!("{} {}", number, self.currency_symbol)
        }
    }

    /// Format integer part with thousands separators.
    fn format_integer_with_separators(&self, integer_part: &str) -> String {
        let mut result = String::new();
        let chars: Vec<char> = integer_part.chars().collect();
        let start_idx = if chars.first() == Some(&'-') {
            result.push('-');
            1
        } else {
            0
        };

        for (i, &ch) in chars[start_idx..].iter().enumerate() {
            if i > 0 && (chars.len() - start_idx - i) % 3 == 0 {
                result.push(self.thousands_separator);
            }
            result.push(ch);
        }

        result
    }
}

/// Locale manager for resolving and caching locale information.
pub struct LocaleManager {
    /// Cache of loaded locales
    locales: HashMap<String, Locale>,
    /// Default locale
    default_locale: Locale,
}

impl LocaleManager {
    /// Create a new locale manager.
    pub fn new() -> Self {
        let mut manager = Self {
            locales: HashMap::new(),
            default_locale: Locale::en_us(),
        };

        // Pre-load common locales
        manager.register_locale(Locale::en_us());
        manager.register_locale(Locale::en_gb());
        manager.register_locale(Locale::fr_fr());
        manager.register_locale(Locale::de_de());
        manager.register_locale(Locale::ja_jp());

        manager
    }

    /// Register a locale.
    pub fn register_locale(&mut self, locale: Locale) {
        let identifier = locale.identifier();
        self.locales.insert(identifier, locale);
    }

    /// Get a locale by identifier.
    pub fn get_locale(&self, identifier: &str) -> Option<&Locale> {
        self.locales.get(identifier)
    }

    /// Get locale or default if not found.
    pub fn get_locale_or_default(&self, identifier: &str) -> &Locale {
        self.locales.get(identifier).unwrap_or(&self.default_locale)
    }

    /// Set default locale.
    pub fn set_default_locale(&mut self, locale: Locale) {
        self.default_locale = locale;
    }

    /// Get default locale.
    pub fn default_locale(&self) -> &Locale {
        &self.default_locale
    }

    /// Detect system locale (placeholder implementation).
    pub fn detect_system_locale(&self) -> &Locale {
        // In a real implementation, this would detect the system locale
        // from environment variables or system APIs
        &self.default_locale
    }

    /// List all registered locales.
    pub fn list_locales(&self) -> Vec<&str> {
        self.locales.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for LocaleManager {
    fn default() -> Self {
        Self::new()
    }
}

use std::sync::{LazyLock, Mutex};

/// Global locale manager instance.
static GLOBAL_LOCALE_MANAGER: LazyLock<Mutex<LocaleManager>> =
    LazyLock::new(|| Mutex::new(LocaleManager::new()));

/// Get the global locale manager.
pub fn with_global_locale_manager<F, R>(f: F) -> R
where
    F: FnOnce(&LocaleManager) -> R,
{
    let manager = GLOBAL_LOCALE_MANAGER.lock().unwrap();
    f(&manager)
}

/// Get the global locale manager mutably (for configuration).
pub fn with_global_locale_manager_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut LocaleManager) -> R,
{
    let mut manager = GLOBAL_LOCALE_MANAGER.lock().unwrap();
    f(&mut manager)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locale_creation() {
        let en_us = Locale::en_us();
        assert_eq!(en_us.language, "en");
        assert_eq!(en_us.country, Some("US".to_string()));
        assert_eq!(en_us.decimal_separator, '.');
        assert_eq!(en_us.thousands_separator, ',');
        assert_eq!(en_us.currency_symbol, "$");
        assert!(en_us.currency_before);

        let fr_fr = Locale::fr_fr();
        assert_eq!(fr_fr.decimal_separator, ',');
        assert_eq!(fr_fr.thousands_separator, ' ');
        assert_eq!(fr_fr.currency_symbol, "€");
        assert!(!fr_fr.currency_before);
    }

    #[test]
    fn test_locale_identifier() {
        let en_us = Locale::en_us();
        assert_eq!(en_us.identifier(), "en_US");

        let fr_locale = Locale {
            language: "fr".to_string(),
            country: None,
            ..Locale::fr_fr()
        };
        assert_eq!(fr_locale.identifier(), "fr");
    }

    #[test]
    fn test_number_formatting() {
        let en_us = Locale::en_us();
        assert_eq!(en_us.format_number(1234.567, 2), "1,234.57");
        assert_eq!(en_us.format_number(-1234.567, 2), "-1,234.57");
        assert_eq!(en_us.format_number(1234.0, 0), "1,234");

        let fr_fr = Locale::fr_fr();
        assert_eq!(fr_fr.format_number(1234.567, 2), "1 234,57");
    }

    #[test]
    fn test_currency_formatting() {
        let en_us = Locale::en_us();
        assert_eq!(en_us.format_currency(1234.56, 2), "$1,234.56");

        let fr_fr = Locale::fr_fr();
        assert_eq!(fr_fr.format_currency(1234.56, 2), "1 234,56 €");
    }

    #[test]
    fn test_thousands_separator_formatting() {
        let en_us = Locale::en_us();

        // Test various number sizes
        assert_eq!(en_us.format_integer_with_separators("123"), "123");
        assert_eq!(en_us.format_integer_with_separators("1234"), "1,234");
        assert_eq!(en_us.format_integer_with_separators("12345"), "12,345");
        assert_eq!(en_us.format_integer_with_separators("123456"), "123,456");
        assert_eq!(en_us.format_integer_with_separators("1234567"), "1,234,567");
        assert_eq!(
            en_us.format_integer_with_separators("-1234567"),
            "-1,234,567"
        );
    }

    #[test]
    fn test_locale_manager() {
        let manager = LocaleManager::new();

        // Test getting registered locales
        let en_us = manager.get_locale("en_US").unwrap();
        assert_eq!(en_us.currency_symbol, "$");

        let fr_fr = manager.get_locale("fr_FR").unwrap();
        assert_eq!(fr_fr.currency_symbol, "€");

        // Test fallback to default
        let unknown = manager.get_locale_or_default("unknown");
        assert_eq!(unknown.currency_symbol, "$"); // Should be en_US default

        // Test listing locales
        let locales = manager.list_locales();
        assert!(locales.contains(&"en_US"));
        assert!(locales.contains(&"fr_FR"));
        assert!(locales.len() >= 5); // Should have at least the pre-loaded ones
    }

    #[test]
    fn test_global_locale_manager() {
        with_global_locale_manager(|manager| {
            assert!(manager.get_locale("en_US").is_some());
        });

        // Test that the global manager is properly initialized
        with_global_locale_manager_mut(|manager| {
            let count_before = manager.list_locales().len();
            assert!(count_before >= 5); // Should have pre-loaded locales
        });
    }
}
