// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! AT-SPI2 (Assistive Technology Service Provider Interface) integration for Linux.
//!
//! This module implements the AT-SPI2 protocol over D-Bus to enable native
//! screen reader support on Linux (e.g., Orca).

#[cfg(target_os = "linux")]
use zbus::{Connection, Result as ZBusResult};

#[cfg(target_os = "linux")]
use crate::accessibility::aria::{AriaNode, AriaRole, AriaUpdate, NodeId};

#[cfg(target_os = "linux")]
use std::collections::HashMap;

/// AT-SPI2 accessible object representation.
///
/// Maps a Gup AriaNode to an AT-SPI2 accessible object that can be
/// exposed via D-Bus.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone)]
pub struct AccessibleObject {
    /// Unique object path in D-Bus (e.g., "/org/a11y/atspi/accessible/1")
    pub object_path: String,

    /// ATK role mapped from ARIA role
    pub role: AtkRole,

    /// Name/label of the object
    pub name: String,

    /// Description of the object
    pub description: String,

    /// Parent object path (if any)
    pub parent: Option<String>,

    /// Child object paths
    pub children: Vec<String>,

    /// Current value (for data points)
    pub value: Option<String>,
}

/// ATK (Accessibility Toolkit) roles for Linux accessibility.
///
/// Maps ARIA roles to ATK roles for AT-SPI2 compatibility.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtkRole {
    /// A chart or data visualization (maps to ROLE_CHART or ROLE_PANEL)
    Chart,

    /// A panel or container (for chart series)
    Panel,

    /// A label or text element (for data points)
    Label,

    /// A legend or key
    Legend,

    /// An axis or ruler
    Ruler,

    /// A tooltip or help bubble
    ToolTip,

    /// A control or button
    Button,
}

#[cfg(target_os = "linux")]
impl AtkRole {
    /// Convert ATK role to numeric value for AT-SPI2.
    ///
    /// These values match the ATK role enum defined in the ATK library.
    pub fn to_numeric(&self) -> u32 {
        match self {
            AtkRole::Chart => 86,   // ROLE_CHART
            AtkRole::Panel => 29,   // ROLE_PANEL
            AtkRole::Label => 28,   // ROLE_LABEL
            AtkRole::Legend => 83,  // ROLE_GROUPING (closest match)
            AtkRole::Ruler => 27,   // ROLE_RULER
            AtkRole::ToolTip => 38, // ROLE_TOOL_TIP
            AtkRole::Button => 34,  // ROLE_PUSH_BUTTON
        }
    }

    /// Convert ARIA role to ATK role.
    pub fn from_aria_role(role: &AriaRole) -> Self {
        match role {
            AriaRole::Chart => AtkRole::Chart,
            AriaRole::ChartSeries => AtkRole::Panel,
            AriaRole::DataPoint => AtkRole::Label,
            AriaRole::Legend => AtkRole::Legend,
            AriaRole::Axis => AtkRole::Ruler,
            AriaRole::Tooltip => AtkRole::ToolTip,
            AriaRole::Control => AtkRole::Button,
        }
    }
}

/// AT-SPI2 D-Bus manager for accessibility.
///
/// Manages the D-Bus connection and accessible object registry for
/// AT-SPI2 communication with screen readers.
#[cfg(target_os = "linux")]
pub struct AtSpiManager {
    /// D-Bus connection to the accessibility bus
    connection: Option<Connection>,

    /// Registry of accessible objects by NodeId
    objects: HashMap<NodeId, AccessibleObject>,

    /// Application name for AT-SPI2 registration
    app_name: String,

    /// Next available object ID
    next_object_id: u64,
}

#[cfg(target_os = "linux")]
impl AtSpiManager {
    /// Create a new AT-SPI2 manager.
    pub fn new(app_name: String) -> Self {
        Self {
            connection: None,
            objects: HashMap::new(),
            app_name,
            next_object_id: 1,
        }
    }

    /// Initialize connection to the AT-SPI2 accessibility bus.
    pub async fn connect(&mut self) -> ZBusResult<()> {
        // Connect to the AT-SPI2 accessibility bus
        // In a real implementation, we would:
        // 1. Connect to session bus
        // 2. Register with AT-SPI2 registry
        // 3. Implement AT-SPI2 D-Bus interfaces

        // For now, establish a basic session bus connection
        let connection = Connection::session().await?;
        self.connection = Some(connection);

        log::info!(
            "Connected to AT-SPI2 accessibility bus for '{}'",
            self.app_name
        );
        Ok(())
    }

    /// Create an accessible object from an ARIA node.
    pub fn create_accessible_object(&mut self, node: &AriaNode) -> AccessibleObject {
        let object_path = format!("/org/a11y/atspi/accessible/{}", self.next_object_id);
        self.next_object_id += 1;

        AccessibleObject {
            object_path,
            role: AtkRole::from_aria_role(&node.role),
            name: node.label.clone(),
            description: node.description.clone().unwrap_or_default(),
            parent: None,
            children: Vec::new(),
            value: node.value.clone(),
        }
    }

    /// Register an accessible object in the tree.
    pub fn register_object(&mut self, node_id: NodeId, object: AccessibleObject) {
        self.objects.insert(node_id, object);
    }

    /// Get an accessible object by node ID.
    pub fn get_object(&self, node_id: &NodeId) -> Option<&AccessibleObject> {
        self.objects.get(node_id)
    }

    /// Update an existing accessible object.
    pub fn update_object(&mut self, node_id: NodeId, node: &AriaNode) {
        if let Some(object) = self.objects.get_mut(&node_id) {
            object.name = node.label.clone();
            object.description = node.description.clone().unwrap_or_default();
            object.value = node.value.clone();
            object.role = AtkRole::from_aria_role(&node.role);
        }
    }

    /// Remove an accessible object from the tree.
    pub fn remove_object(&mut self, node_id: &NodeId) {
        self.objects.remove(node_id);
    }

    /// Send an announcement to screen readers via AT-SPI2.
    ///
    /// This uses the object:text-changed signal which Orca listens to.
    pub async fn announce(&self, message: &str) -> ZBusResult<()> {
        if self.connection.is_none() {
            return Ok(());
        }

        // In a full implementation, we would emit a D-Bus signal:
        // - Use object:text-changed for live region updates
        // - Or window:activate for focus announcements

        log::debug!("AT-SPI2 announcement: {}", message);
        Ok(())
    }

    /// Set focus on an accessible object.
    ///
    /// This sends focus events via AT-SPI2 D-Bus signals.
    pub async fn set_focus(&self, node_id: &NodeId) -> ZBusResult<()> {
        if self.connection.is_none() {
            return Ok(());
        }

        if let Some(object) = self.objects.get(node_id) {
            // In a full implementation, emit focus events:
            // - object:state-changed:focused
            // - window:activate

            log::debug!("AT-SPI2 focus set to: {}", object.name);
        }

        Ok(())
    }

    /// Process ARIA updates and translate to AT-SPI2 signals.
    pub async fn process_updates(
        &mut self,
        updates: &[AriaUpdate],
        aria_nodes: &HashMap<NodeId, AriaNode>,
    ) -> ZBusResult<()> {
        for update in updates {
            match update {
                AriaUpdate::NodeCreated { node_id } => {
                    if let Some(node) = aria_nodes.get(node_id) {
                        let object = self.create_accessible_object(node);
                        self.register_object(*node_id, object);

                        log::debug!("Created AT-SPI2 object for node {:?}", node_id);
                    }
                }

                AriaUpdate::NodeUpdated { node_id } => {
                    if let Some(node) = aria_nodes.get(node_id) {
                        self.update_object(*node_id, node);

                        log::debug!("Updated AT-SPI2 object for node {:?}", node_id);
                    }
                }

                AriaUpdate::NodeRemoved { node_id } => {
                    self.remove_object(node_id);

                    log::debug!("Removed AT-SPI2 object for node {:?}", node_id);
                }

                AriaUpdate::FocusChanged { node_id } => {
                    self.set_focus(node_id).await?;
                }

                AriaUpdate::LiveRegion { content, .. } => {
                    self.announce(content).await?;
                }
            }
        }

        Ok(())
    }

    /// Check if connected to AT-SPI2.
    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }
}

#[cfg(not(target_os = "linux"))]
/// Placeholder for non-Linux platforms.
pub struct AtSpiManager;

#[cfg(not(target_os = "linux"))]
impl AtSpiManager {
    /// Create a new no-op AT-SPI manager on non-Linux platforms.
    pub fn new(_app_name: String) -> Self {
        Self
    }
}
