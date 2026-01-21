// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Plugin system for third-party Mixable implementations.
//!
//! This module provides a framework for registering and using third-party plugins
//! that can create Mixable wrappers for external visualization types. The plugin
//! system enables ecosystem integration while maintaining type safety and performance.
//!
//! # Examples
//!
//! ## Creating a Plugin
//!
//! ```rust
//! use gup::plugins::{MixablePlugin, MixablePluginRegistry};
//! use gup::{Mixable, RenderContext, GupResult};
//! use std::any::{Any, TypeId};
//!
//! #[derive(Debug)]
//! struct MyExternalType { data: Vec<f32> }
//! impl MyExternalType { fn new() -> Self { Self { data: vec![] } } }
//!
//! #[derive(Debug)]
//! struct MyTypeWrapper(MyExternalType);
//! impl Mixable for MyTypeWrapper {
//!     type Output = ();
//!     fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> { Ok(()) }
//!     fn is_valid(&self) -> bool { true }
//!     fn description(&self) -> String { "MyType".to_string() }
//! }
//!
//! #[derive(Debug)]
//! struct MyLibraryPlugin;
//!
//! impl MixablePlugin for MyLibraryPlugin {
//!     fn name(&self) -> &str {
//!         "my_visualization_library"
//!     }
//!
//!     fn version(&self) -> &str {
//!         "1.0.0"
//!     }
//!
//!     fn can_handle(&self, type_id: TypeId) -> bool {
//!         // Check if this plugin can handle the given type
//!         type_id == TypeId::of::<MyExternalType>()
//!     }
//!
//!     fn create_mixable(&self, object: Box<dyn Any + Send + Sync>) -> Box<dyn Mixable<Output = ()>> {
//!         // Downcast and wrap the external type
//!         if let Ok(my_type) = object.downcast::<MyExternalType>() {
//!             // Create a Mixable wrapper
//!             Box::new(MyTypeWrapper(*my_type))
//!         } else {
//!             panic!("Plugin received unexpected type");
//!         }
//!     }
//!
//!     fn validate(&self) -> Result<(), String> {
//!         // Validate plugin compatibility and dependencies
//!         Ok(())
//!     }
//! }
//!
//! // Register the plugin
//! let mut registry = MixablePluginRegistry::new();
//! registry.register_plugin(Box::new(MyLibraryPlugin)).unwrap();
//! ```
//!
//! ## Using the Global Registry
//!
//! ```rust
//! use gup::plugins::{global_registry, try_make_mixable};
//!
//! #[derive(Debug)]
//! struct MyExternalType { data: Vec<f32> }
//! impl MyExternalType { fn new() -> Self { Self { data: vec![] } } }
//!
//! // Use the global registry to create mixables
//! let external_data = MyExternalType::new();
//! if let Some(mixable) = try_make_mixable(external_data) {
//!     // The external data is now a Mixable that can be composed
//! }
//! ```

use crate::{GupResult, Mixable, RenderContext};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, Mutex, OnceLock};

/// Registry for Mixable plugin implementations.
///
/// The registry manages third-party plugins that can create Mixable wrappers
/// for external visualization types. Plugins are identified by name and can
/// be queried to determine if they can handle specific types.
#[derive(Debug)]
pub struct MixablePluginRegistry {
    plugins: HashMap<String, Box<dyn MixablePlugin>>,
    type_mappings: HashMap<TypeId, String>,
}

/// Trait for Mixable plugins that can wrap external visualization types.
///
/// Implement this trait to create plugins that integrate external visualization
/// libraries with Gup's composition system.
pub trait MixablePlugin: Send + Sync + Debug {
    /// Get the plugin name.
    ///
    /// This should be a unique identifier for the plugin, typically matching
    /// the external library name.
    fn name(&self) -> &str;

    /// Get the plugin version.
    ///
    /// This should follow semantic versioning conventions.
    fn version(&self) -> &str;

    /// Check if this plugin can handle the given type.
    ///
    /// # Arguments
    ///
    /// * `type_id` - The TypeId of the external type to check
    ///
    /// # Returns
    ///
    /// `true` if this plugin can create a Mixable wrapper for the type
    fn can_handle(&self, type_id: TypeId) -> bool;

    /// Create a Mixable wrapper for the given object.
    ///
    /// This method should downcast the `Any` object to the expected type
    /// and create an appropriate Mixable wrapper.
    ///
    /// # Arguments
    ///
    /// * `object` - The external object to wrap
    ///
    /// # Returns
    ///
    /// A boxed Mixable implementation
    ///
    /// # Panics
    ///
    /// May panic if the object cannot be downcast to the expected type.
    /// Use `can_handle` to check compatibility first.
    fn create_mixable(&self, object: Box<dyn Any + Send + Sync>) -> Box<dyn Mixable<Output = ()>>;

    /// Validate plugin compatibility and dependencies.
    ///
    /// This method is called during plugin registration to ensure the plugin
    /// is compatible with the current system and has all required dependencies.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the plugin is valid, `Err(message)` otherwise
    fn validate(&self) -> Result<(), String>;

    /// Get plugin metadata for introspection.
    ///
    /// Override this method to provide additional metadata about the plugin.
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: self.name().to_string(),
            version: self.version().to_string(),
            description: format!("Plugin for {}", self.name()),
            author: "Unknown".to_string(),
            supported_types: Vec::new(),
        }
    }
}

/// Metadata about a plugin for introspection and documentation.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Plugin author
    pub author: String,
    /// List of supported type names (for documentation)
    pub supported_types: Vec<String>,
}

impl MixablePluginRegistry {
    /// Create a new plugin registry.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            type_mappings: HashMap::new(),
        }
    }

    /// Register a new plugin.
    ///
    /// # Arguments
    ///
    /// * `plugin` - The plugin to register
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(message)` if registration fails
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Plugin validation fails
    /// - A plugin with the same name is already registered
    pub fn register_plugin(&mut self, plugin: Box<dyn MixablePlugin>) -> Result<(), String> {
        // Validate plugin before registration
        plugin.validate()?;

        let name = plugin.name().to_string();

        // Check for conflicts
        if self.plugins.contains_key(&name) {
            return Err(format!("Plugin '{name}' is already registered"));
        }

        // Register the plugin
        self.plugins.insert(name.clone(), plugin);

        Ok(())
    }

    /// Unregister a plugin by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the plugin to unregister
    ///
    /// # Returns
    ///
    /// `true` if a plugin was removed, `false` if no plugin with that name exists
    pub fn unregister_plugin(&mut self, name: &str) -> bool {
        if self.plugins.remove(name).is_some() {
            // Remove type mappings for this plugin
            self.type_mappings
                .retain(|_, plugin_name| plugin_name != name);
            true
        } else {
            false
        }
    }

    /// Create a Mixable from an external object using registered plugins.
    ///
    /// This method searches through registered plugins to find one that can
    /// handle the given type and creates a Mixable wrapper.
    ///
    /// # Arguments
    ///
    /// * `object` - The external object to wrap
    ///
    /// # Returns
    ///
    /// `Some(mixable)` if a compatible plugin is found, `None` otherwise
    pub fn create_mixable<T: Any + Send + Sync>(
        &self,
        object: T,
    ) -> Option<Box<dyn Mixable<Output = ()>>> {
        let type_id = TypeId::of::<T>();

        // Check if we have a cached mapping for this type
        if let Some(plugin_name) = self.type_mappings.get(&type_id)
            && let Some(plugin) = self.plugins.get(plugin_name)
        {
            let boxed_object = Box::new(object);
            return Some(plugin.create_mixable(boxed_object));
        }

        // Search through all plugins for one that can handle this type
        for plugin in self.plugins.values() {
            if plugin.can_handle(type_id) {
                let boxed_object = Box::new(object);
                return Some(plugin.create_mixable(boxed_object));
            }
        }

        None
    }

    /// Create a Mixable from a boxed Any object.
    ///
    /// This is useful when you have an already-boxed object and want to avoid
    /// additional boxing/unboxing.
    ///
    /// # Arguments
    ///
    /// * `object` - The boxed external object to wrap
    /// * `type_id` - The TypeId of the original object type
    ///
    /// # Returns
    ///
    /// `Some(mixable)` if a compatible plugin is found, `None` otherwise
    pub fn create_mixable_from_any(
        &self,
        object: Box<dyn Any + Send + Sync>,
        type_id: TypeId,
    ) -> Option<Box<dyn Mixable<Output = ()>>> {
        // Check if we have a cached mapping for this type
        if let Some(plugin_name) = self.type_mappings.get(&type_id)
            && let Some(plugin) = self.plugins.get(plugin_name)
        {
            return Some(plugin.create_mixable(object));
        }

        // Search through all plugins for one that can handle this type
        for plugin in self.plugins.values() {
            if plugin.can_handle(type_id) {
                return Some(plugin.create_mixable(object));
            }
        }

        None
    }

    /// List all registered plugins.
    ///
    /// # Returns
    ///
    /// Vector of tuples containing (plugin_name, plugin_version)
    pub fn list_plugins(&self) -> Vec<(&str, &str)> {
        self.plugins
            .values()
            .map(|plugin| (plugin.name(), plugin.version()))
            .collect()
    }

    /// Get metadata for all registered plugins.
    ///
    /// # Returns
    ///
    /// Vector of plugin metadata
    pub fn get_all_metadata(&self) -> Vec<PluginMetadata> {
        self.plugins
            .values()
            .map(|plugin| plugin.metadata())
            .collect()
    }

    /// Get metadata for a specific plugin.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the plugin
    ///
    /// # Returns
    ///
    /// Plugin metadata if found, `None` otherwise
    pub fn get_plugin_metadata(&self, name: &str) -> Option<PluginMetadata> {
        self.plugins.get(name).map(|plugin| plugin.metadata())
    }

    /// Check if a plugin is registered.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the plugin to check
    ///
    /// # Returns
    ///
    /// `true` if a plugin with the given name is registered
    pub fn has_plugin(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    /// Check if any plugin can handle the given type.
    ///
    /// # Arguments
    ///
    /// * `type_id` - The TypeId to check
    ///
    /// # Returns
    ///
    /// `true` if at least one plugin can handle the type
    pub fn can_handle_type(&self, type_id: TypeId) -> bool {
        self.plugins
            .values()
            .any(|plugin| plugin.can_handle(type_id))
    }

    /// Clear all registered plugins.
    pub fn clear(&mut self) {
        self.plugins.clear();
        self.type_mappings.clear();
    }
}

impl Default for MixablePluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Example plugin implementation for demonstration purposes.
///
/// This plugin shows how to implement the MixablePlugin trait for a
/// hypothetical external visualization library.
#[derive(Debug)]
pub struct ExampleExternalLibraryPlugin;

impl MixablePlugin for ExampleExternalLibraryPlugin {
    fn name(&self) -> &str {
        "example_external_library"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn can_handle(&self, _type_id: TypeId) -> bool {
        // In a real implementation, this would check for specific external types
        // For example: type_id == TypeId::of::<ExternalChart>()
        false // Placeholder - no actual types supported
    }

    fn create_mixable(&self, _object: Box<dyn Any + Send + Sync>) -> Box<dyn Mixable<Output = ()>> {
        // In a real implementation, this would downcast the object and create
        // an appropriate wrapper
        Box::new(PlaceholderMixable)
    }

    fn validate(&self) -> Result<(), String> {
        // In a real implementation, this might check for:
        // - External library availability
        // - Version compatibility
        // - Required system capabilities
        Ok(())
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: self.name().to_string(),
            version: self.version().to_string(),
            description: "Example plugin for demonstration purposes".to_string(),
            author: "Gup Development Team".to_string(),
            supported_types: vec![
                "ExampleChart".to_string(),
                "ExampleVisualization".to_string(),
            ],
        }
    }
}

/// Placeholder Mixable implementation for the example plugin.
#[derive(Debug)]
struct PlaceholderMixable;

impl Mixable for PlaceholderMixable {
    type Output = ();

    fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
        // Placeholder implementation
        Ok(())
    }

    fn is_valid(&self) -> bool {
        true
    }

    fn description(&self) -> String {
        "Placeholder Mixable".to_string()
    }
}

/// Global plugin registry instance.
///
/// This provides a convenient way to register and use plugins without
/// manually managing registry instances.
static GLOBAL_REGISTRY: OnceLock<Mutex<MixablePluginRegistry>> = OnceLock::new();

/// Get the global plugin registry.
///
/// # Returns
///
/// A reference to the global registry
///
/// # Thread Safety
///
/// This function is thread-safe and the returned registry is protected by a mutex.
pub fn global_registry() -> &'static Mutex<MixablePluginRegistry> {
    GLOBAL_REGISTRY.get_or_init(|| Mutex::new(MixablePluginRegistry::new()))
}

/// Convenience macro for registering plugins with the global registry.
///
/// # Examples
///
/// ```rust
/// use gup::{register_mixable_plugin, plugins::ExampleExternalLibraryPlugin};
///
/// // Register a plugin instance
/// register_mixable_plugin!(ExampleExternalLibraryPlugin);
/// ```
#[macro_export]
macro_rules! register_mixable_plugin {
    ($plugin:expr) => {
        $crate::plugins::global_registry()
            .lock()
            .expect("Failed to acquire plugin registry lock")
            .register_plugin(Box::new($plugin))
            .expect("Failed to register plugin");
    };
}

/// Create a Mixable from external data using the global plugin registry.
///
/// This is a convenience function that uses the global registry to attempt
/// to create a Mixable wrapper for external data.
///
/// # Arguments
///
/// * `object` - The external object to wrap
///
/// # Returns
///
/// `Some(mixable)` if a compatible plugin is found, `None` otherwise
///
/// # Examples
///
/// ```rust
/// use gup::plugins::try_make_mixable;
///
/// struct MyExternalData {
///     values: Vec<f32>,
/// }
///
/// let external_data = MyExternalData { values: vec![1.0, 2.0, 3.0] };
///
/// if let Some(mixable) = try_make_mixable(external_data) {
///     // The external data is now a Mixable
///     // let composed = mixable.mix(other_visualization);
/// } else {
///     // No plugin available for this type
///     println!("No plugin available for MyExternalData");
/// }
/// ```
pub fn try_make_mixable<T: Any + Send + Sync>(object: T) -> Option<Box<dyn Mixable<Output = ()>>> {
    global_registry().lock().ok()?.create_mixable(object)
}

/// Plugin development utilities.
pub mod development {

    use super::*;

    /// Type alias for point extraction functions to reduce complexity
    type PointExtractor<T> = Box<dyn Fn(&T) -> Vec<[f32; 2]> + Send + Sync>;

    /// Type alias for shared point extraction functions
    type SharedPointExtractor<T> = Arc<dyn Fn(&T) -> Vec<[f32; 2]> + Send + Sync>;

    /// Helper for creating simple point-based plugins.
    ///
    /// This utility creates a plugin that wraps external types by extracting
    /// 2D points from them.
    pub struct PointBasedPluginBuilder<T> {
        name: String,
        version: String,
        extractor: PointExtractor<T>,
        validator: Option<Box<dyn Fn() -> Result<(), String> + Send + Sync>>,
    }

    impl<T: Any + Send + Sync + Debug + 'static> PointBasedPluginBuilder<T> {
        /// Create a new point-based plugin builder.
        ///
        /// # Arguments
        ///
        /// * `name` - Plugin name
        /// * `version` - Plugin version
        /// * `extractor` - Function to extract 2D points from the external type
        pub fn new(
            name: String,
            version: String,
            extractor: impl Fn(&T) -> Vec<[f32; 2]> + Send + Sync + 'static,
        ) -> Self {
            Self {
                name,
                version,
                extractor: Box::new(extractor),
                validator: None,
            }
        }

        /// Add a custom validator to the plugin.
        ///
        /// # Arguments
        ///
        /// * `validator` - Function that validates plugin compatibility
        pub fn with_validator(
            mut self,
            validator: impl Fn() -> Result<(), String> + Send + Sync + 'static,
        ) -> Self {
            self.validator = Some(Box::new(validator));
            self
        }

        /// Build the plugin.
        pub fn build(self) -> PointBasedPlugin<T> {
            PointBasedPlugin {
                name: self.name,
                version: self.version,
                extractor: Arc::from(self.extractor),
                validator: self.validator,
                _phantom: std::marker::PhantomData,
            }
        }
    }

    /// A plugin implementation for point-based external types.
    pub struct PointBasedPlugin<T> {
        name: String,
        version: String,
        extractor: SharedPointExtractor<T>,
        validator: Option<Box<dyn Fn() -> Result<(), String> + Send + Sync>>,
        _phantom: std::marker::PhantomData<T>,
    }

    impl<T> Debug for PointBasedPlugin<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("PointBasedPlugin")
                .field("name", &self.name)
                .field("version", &self.version)
                .finish()
        }
    }

    impl<T: Any + Send + Sync + Debug + 'static> MixablePlugin for PointBasedPlugin<T> {
        fn name(&self) -> &str {
            &self.name
        }

        fn version(&self) -> &str {
            &self.version
        }

        fn can_handle(&self, type_id: TypeId) -> bool {
            type_id == TypeId::of::<T>()
        }

        fn create_mixable(
            &self,
            object: Box<dyn Any + Send + Sync>,
        ) -> Box<dyn Mixable<Output = ()>> {
            if let Ok(typed_object) = object.downcast::<T>() {
                let extractor = self.extractor.clone();
                let wrapped =
                    crate::integration::wrap_point_data(*typed_object, move |data| extractor(data));
                Box::new(wrapped)
            } else {
                panic!("Plugin received unexpected type");
            }
        }

        fn validate(&self) -> Result<(), String> {
            if let Some(validator) = &self.validator {
                validator()
            } else {
                Ok(())
            }
        }

        fn metadata(&self) -> PluginMetadata {
            PluginMetadata {
                name: self.name.clone(),
                version: self.version.clone(),
                description: format!("Point-based plugin for {}", std::any::type_name::<T>()),
                author: "Generated by PointBasedPluginBuilder".to_string(),
                supported_types: vec![std::any::type_name::<T>().to_string()],
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::development::*;
    use super::*;

    #[derive(Debug)]
    struct TestExternalType {
        data: Vec<(f32, f32)>,
    }

    #[derive(Debug)]
    struct TestPlugin {
        should_fail_validation: bool,
    }

    impl TestPlugin {
        fn new() -> Self {
            Self {
                should_fail_validation: false,
            }
        }

        fn with_validation_failure() -> Self {
            Self {
                should_fail_validation: true,
            }
        }
    }

    impl MixablePlugin for TestPlugin {
        fn name(&self) -> &str {
            "test_plugin"
        }

        fn version(&self) -> &str {
            "1.0.0"
        }

        fn can_handle(&self, type_id: TypeId) -> bool {
            type_id == TypeId::of::<TestExternalType>()
        }

        fn create_mixable(
            &self,
            object: Box<dyn Any + Send + Sync>,
        ) -> Box<dyn Mixable<Output = ()>> {
            if let Ok(test_type) = object.downcast::<TestExternalType>() {
                let wrapped = crate::integration::wrap_point_data(*test_type, |data| {
                    data.data.iter().map(|&(x, y)| [x, y]).collect()
                });
                Box::new(wrapped)
            } else {
                panic!("TestPlugin received unexpected type");
            }
        }

        fn validate(&self) -> Result<(), String> {
            if self.should_fail_validation {
                Err("Validation failure for testing".to_string())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn test_registry_basic_operations() {
        let mut registry = MixablePluginRegistry::new();

        // Initially empty
        assert_eq!(registry.list_plugins().len(), 0);
        assert!(!registry.has_plugin("test_plugin"));

        // Register a plugin
        let plugin = TestPlugin::new();
        assert!(registry.register_plugin(Box::new(plugin)).is_ok());

        // Check registration
        assert_eq!(registry.list_plugins().len(), 1);
        assert!(registry.has_plugin("test_plugin"));

        // Check type handling
        assert!(registry.can_handle_type(TypeId::of::<TestExternalType>()));
        assert!(!registry.can_handle_type(TypeId::of::<String>()));
    }

    #[test]
    fn test_plugin_registration_conflict() {
        let mut registry = MixablePluginRegistry::new();

        // Register first plugin
        let plugin1 = TestPlugin::new();
        assert!(registry.register_plugin(Box::new(plugin1)).is_ok());

        // Try to register another plugin with the same name
        let plugin2 = TestPlugin::new();
        let result = registry.register_plugin(Box::new(plugin2));

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already registered"));
    }

    #[test]
    fn test_plugin_validation_failure() {
        let mut registry = MixablePluginRegistry::new();

        let failing_plugin = TestPlugin::with_validation_failure();
        let result = registry.register_plugin(Box::new(failing_plugin));

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Validation failure"));
    }

    #[test]
    fn test_create_mixable_from_registry() {
        let mut registry = MixablePluginRegistry::new();

        // Register plugin
        let plugin = TestPlugin::new();
        registry.register_plugin(Box::new(plugin)).unwrap();

        // Create external data
        let external_data = TestExternalType {
            data: vec![(0.0, 0.0), (1.0, 1.0)],
        };

        // Create mixable
        let mixable = registry.create_mixable(external_data);
        assert!(mixable.is_some());

        let mixable = mixable.unwrap();
        assert!(mixable.is_valid());
    }

    #[test]
    fn test_create_mixable_no_plugin() {
        let registry = MixablePluginRegistry::new(); // Empty registry

        let external_data = TestExternalType {
            data: vec![(0.0, 0.0)],
        };

        let mixable = registry.create_mixable(external_data);
        assert!(mixable.is_none());
    }

    #[test]
    fn test_unregister_plugin() {
        let mut registry = MixablePluginRegistry::new();

        // Register plugin
        let plugin = TestPlugin::new();
        registry.register_plugin(Box::new(plugin)).unwrap();
        assert!(registry.has_plugin("test_plugin"));

        // Unregister plugin
        assert!(registry.unregister_plugin("test_plugin"));
        assert!(!registry.has_plugin("test_plugin"));

        // Try to unregister again
        assert!(!registry.unregister_plugin("test_plugin"));
    }

    #[test]
    fn test_plugin_metadata() {
        let mut registry = MixablePluginRegistry::new();

        let plugin = TestPlugin::new();
        registry.register_plugin(Box::new(plugin)).unwrap();

        let metadata = registry.get_plugin_metadata("test_plugin");
        assert!(metadata.is_some());

        let metadata = metadata.unwrap();
        assert_eq!(metadata.name, "test_plugin");
        assert_eq!(metadata.version, "1.0.0");

        let all_metadata = registry.get_all_metadata();
        assert_eq!(all_metadata.len(), 1);
    }

    #[test]
    fn test_clear_registry() {
        let mut registry = MixablePluginRegistry::new();

        let plugin = TestPlugin::new();
        registry.register_plugin(Box::new(plugin)).unwrap();
        assert_eq!(registry.list_plugins().len(), 1);

        registry.clear();
        assert_eq!(registry.list_plugins().len(), 0);
    }

    #[test]
    fn test_point_based_plugin_builder() {
        let plugin = PointBasedPluginBuilder::new(
            "test_point_plugin".to_string(),
            "1.0.0".to_string(),
            |data: &TestExternalType| data.data.iter().map(|&(x, y)| [x, y]).collect(),
        )
        .with_validator(|| Ok(()))
        .build();

        assert_eq!(plugin.name(), "test_point_plugin");
        assert_eq!(plugin.version(), "1.0.0");
        assert!(plugin.can_handle(TypeId::of::<TestExternalType>()));
        assert!(plugin.validate().is_ok());
    }

    #[test]
    fn test_global_registry() {
        // Test that global registry works
        let registry = global_registry();
        assert!(registry.lock().is_ok());
    }

    #[test]
    fn test_try_make_mixable() {
        // Register a plugin in the global registry
        let plugin = TestPlugin::new();
        global_registry()
            .lock()
            .unwrap()
            .register_plugin(Box::new(plugin))
            .unwrap();

        // Test external data
        let external_data = TestExternalType {
            data: vec![(0.0, 0.0), (1.0, 1.0)],
        };

        // Try to make it mixable
        let mixable = try_make_mixable(external_data);
        assert!(mixable.is_some());

        let mixable = mixable.unwrap();
        assert!(mixable.is_valid());

        // Clean up for other tests
        global_registry().lock().unwrap().clear();
    }
}
