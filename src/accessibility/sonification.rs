// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Data sonification and narration for non-visual data access.
//!
//! This module provides audio representations of data patterns and trends,
//! enabling users to understand data through sound.

use std::collections::HashMap;
use std::time::Duration;

/// Sonification engine for converting data to audio.
#[derive(Debug)]
pub struct SonificationEngine {
    /// Parameter mappings from data fields to audio parameters
    parameter_mappings: HashMap<String, SonificationMapping>,

    /// Whether sonification is currently enabled
    enabled: bool,
}

impl SonificationEngine {
    /// Create a new sonification engine.
    pub fn new() -> Self {
        Self {
            parameter_mappings: HashMap::new(),
            enabled: false,
        }
    }

    /// Enable or disable sonification.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if sonification is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Add a mapping from a data field to an audio parameter.
    pub fn add_mapping(
        &mut self,
        field_name: String,
        parameter: AudioParameter,
        mapping: MappingFunction,
    ) {
        let sonification_mapping = SonificationMapping {
            data_field: field_name.clone(),
            audio_parameter: parameter,
            mapping_function: mapping,
            range: (0.0, 1.0),
        };

        self.parameter_mappings
            .insert(field_name, sonification_mapping);
    }

    /// Remove a mapping.
    pub fn remove_mapping(&mut self, field_name: &str) {
        self.parameter_mappings.remove(field_name);
    }

    /// Get all current mappings.
    pub fn mappings(&self) -> &HashMap<String, SonificationMapping> {
        &self.parameter_mappings
    }

    /// Create a data narration describing patterns and trends.
    pub fn create_data_narration<T>(&self, data: &[T]) -> String {
        let mut narration = String::new();

        narration.push_str(&format!("Dataset contains {} data points. ", data.len()));

        if data.is_empty() {
            narration.push_str("No data to analyze.");
            return narration;
        }

        // Add basic statistics if available
        if data.len() == 1 {
            narration.push_str("Single data point.");
        } else if data.len() < 10 {
            narration.push_str("Small dataset.");
        } else if data.len() < 100 {
            narration.push_str("Medium-sized dataset.");
        } else if data.len() < 10000 {
            narration.push_str("Large dataset.");
        } else {
            narration.push_str("Very large dataset.");
        }

        narration
    }

    /// Create an audio track from data (stub for now).
    pub fn create_audio_track(&self, events: Vec<AudioEvent>) -> AudioTrack {
        let duration = events
            .iter()
            .map(|e| e.timestamp)
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        AudioTrack {
            events,
            duration: Duration::from_secs_f32(duration),
        }
    }
}

impl Default for SonificationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Mapping from data field to audio parameter.
#[derive(Debug, Clone)]
pub struct SonificationMapping {
    /// Name of the data field
    pub data_field: String,

    /// Audio parameter to map to
    pub audio_parameter: AudioParameter,

    /// Function to map data values to audio values
    pub mapping_function: MappingFunction,

    /// Output range for audio parameter
    pub range: (f32, f32),
}

/// Audio parameters that can be controlled by data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioParameter {
    /// Frequency/pitch of sound
    Pitch,

    /// Amplitude/volume
    Volume,

    /// Timbre/waveform characteristics
    Timbre,

    /// Stereo position
    Pan,

    /// Note duration
    Duration,

    /// Rhythm/timing patterns
    Rhythm,
}

/// Mapping function from data to audio values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MappingFunction {
    /// Linear mapping
    Linear,

    /// Logarithmic mapping
    Logarithmic,

    /// Exponential mapping
    Exponential,

    /// Custom scaling factor
    Custom(f32),
}

impl MappingFunction {
    /// Apply the mapping function to a value.
    pub fn apply(&self, value: f32, range: (f32, f32)) -> f32 {
        let (min, max) = range;

        match self {
            Self::Linear => {
                // Direct linear interpolation
                min + value * (max - min)
            }
            Self::Logarithmic => {
                // Logarithmic scale
                let log_val = value.ln();
                let log_min = 0.001f32.ln();
                let log_max = 1.0f32.ln();
                let normalized = (log_val - log_min) / (log_max - log_min);
                min + normalized * (max - min)
            }
            Self::Exponential => {
                // Exponential scale
                let exp_val = value.exp();
                let exp_min = 0.0f32.exp();
                let exp_max = 1.0f32.exp();
                let normalized = (exp_val - exp_min) / (exp_max - exp_min);
                min + normalized * (max - min)
            }
            Self::Custom(factor) => {
                // Custom scaling
                min + (value * factor) * (max - min)
            }
        }
    }
}

/// An audio event in a sonification.
#[derive(Debug, Clone)]
pub struct AudioEvent {
    /// Time of the event in seconds
    pub timestamp: f32,

    /// Audio parameter being controlled
    pub parameter: AudioParameter,

    /// Value of the parameter
    pub value: f32,
}

impl AudioEvent {
    /// Create a new audio event.
    pub fn new(timestamp: f32, parameter: AudioParameter, value: f32) -> Self {
        Self {
            timestamp,
            parameter,
            value,
        }
    }
}

/// An audio track representing sonified data.
#[derive(Debug, Clone)]
pub struct AudioTrack {
    /// All audio events in the track
    pub events: Vec<AudioEvent>,

    /// Total duration of the track
    pub duration: Duration,
}

impl AudioTrack {
    /// Create a new empty audio track.
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            duration: Duration::ZERO,
        }
    }

    /// Add an event to the track.
    pub fn add_event(&mut self, event: AudioEvent) {
        // Update duration if this event is later
        let event_time = Duration::from_secs_f32(event.timestamp);
        if event_time > self.duration {
            self.duration = event_time;
        }

        self.events.push(event);
    }

    /// Get all events of a specific type.
    pub fn events_for_parameter(&self, parameter: AudioParameter) -> Vec<&AudioEvent> {
        self.events
            .iter()
            .filter(|e| e.parameter == parameter)
            .collect()
    }
}

impl Default for AudioTrack {
    fn default() -> Self {
        Self::new()
    }
}

/// Analyze data patterns for narration.
#[derive(Debug, Clone)]
pub struct DataPatterns {
    /// Overall trend
    pub trend: Option<Trend>,

    /// Detected outliers
    pub outliers: Option<Vec<usize>>,

    /// Statistical summary
    pub summary: String,
}

/// Data trend types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trend {
    /// Values are increasing
    Increasing,

    /// Values are decreasing
    Decreasing,

    /// Values are stable
    Stable,

    /// High variability
    Volatile,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sonification_engine_creation() {
        let engine = SonificationEngine::new();
        assert!(!engine.is_enabled());
        assert_eq!(engine.mappings().len(), 0);
    }

    #[test]
    fn test_enable_disable() {
        let mut engine = SonificationEngine::new();

        engine.set_enabled(true);
        assert!(engine.is_enabled());

        engine.set_enabled(false);
        assert!(!engine.is_enabled());
    }

    #[test]
    fn test_add_mapping() {
        let mut engine = SonificationEngine::new();

        engine.add_mapping(
            "value".to_string(),
            AudioParameter::Pitch,
            MappingFunction::Linear,
        );

        assert_eq!(engine.mappings().len(), 1);
        assert!(engine.mappings().contains_key("value"));
    }

    #[test]
    fn test_remove_mapping() {
        let mut engine = SonificationEngine::new();

        engine.add_mapping(
            "value".to_string(),
            AudioParameter::Pitch,
            MappingFunction::Linear,
        );

        engine.remove_mapping("value");
        assert_eq!(engine.mappings().len(), 0);
    }

    #[test]
    fn test_data_narration() {
        let engine = SonificationEngine::new();

        let empty_data: Vec<i32> = vec![];
        let narration = engine.create_data_narration(&empty_data);
        assert!(narration.contains("0 data points"));
        assert!(narration.contains("No data"));

        let single_data = vec![1];
        let narration = engine.create_data_narration(&single_data);
        assert!(narration.contains("1 data point"));
        assert!(narration.contains("Single"));

        let large_data = vec![1; 500];
        let narration = engine.create_data_narration(&large_data);
        assert!(narration.contains("500 data points"));
        assert!(narration.contains("Large"));
    }

    #[test]
    fn test_audio_event() {
        let event = AudioEvent::new(1.0, AudioParameter::Pitch, 440.0);

        assert_eq!(event.timestamp, 1.0);
        assert_eq!(event.parameter, AudioParameter::Pitch);
        assert_eq!(event.value, 440.0);
    }

    #[test]
    fn test_audio_track() {
        let mut track = AudioTrack::new();
        assert_eq!(track.events.len(), 0);
        assert_eq!(track.duration, Duration::ZERO);

        track.add_event(AudioEvent::new(1.0, AudioParameter::Pitch, 440.0));
        track.add_event(AudioEvent::new(2.0, AudioParameter::Volume, 0.8));

        assert_eq!(track.events.len(), 2);
        assert!(track.duration > Duration::ZERO);
    }

    #[test]
    fn test_track_events_by_parameter() {
        let mut track = AudioTrack::new();

        track.add_event(AudioEvent::new(1.0, AudioParameter::Pitch, 440.0));
        track.add_event(AudioEvent::new(2.0, AudioParameter::Volume, 0.8));
        track.add_event(AudioEvent::new(3.0, AudioParameter::Pitch, 880.0));

        let pitch_events = track.events_for_parameter(AudioParameter::Pitch);
        assert_eq!(pitch_events.len(), 2);

        let volume_events = track.events_for_parameter(AudioParameter::Volume);
        assert_eq!(volume_events.len(), 1);
    }

    #[test]
    fn test_mapping_function_linear() {
        let mapping = MappingFunction::Linear;
        let result = mapping.apply(0.5, (0.0, 100.0));
        assert!((result - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_mapping_function_custom() {
        let mapping = MappingFunction::Custom(2.0);
        let result = mapping.apply(0.5, (0.0, 100.0));
        assert!((result - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_audio_parameters() {
        let pitch = AudioParameter::Pitch;
        let volume = AudioParameter::Volume;

        assert_ne!(pitch, volume);
        assert_eq!(pitch, AudioParameter::Pitch);
    }

    #[test]
    fn test_create_audio_track() {
        let engine = SonificationEngine::new();

        let events = vec![
            AudioEvent::new(0.0, AudioParameter::Pitch, 440.0),
            AudioEvent::new(1.0, AudioParameter::Pitch, 880.0),
            AudioEvent::new(2.0, AudioParameter::Volume, 0.5),
        ];

        let track = engine.create_audio_track(events);

        assert_eq!(track.events.len(), 3);
        assert!(track.duration.as_secs_f32() >= 2.0);
    }
}
