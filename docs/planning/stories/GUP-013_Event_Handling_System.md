# GUP-013: Event Handling System

## Story Overview

**Title**: Implement High-Level Event Handling System  
**Epic**: Phase 1 Initiative 4 - Interaction System and Performance  
**Priority**: High  
**Story Points**: 8  

## Context

The event handling system provides the high-level interface for developers to respond to user interactions. It must connect GPU interaction results with familiar event handling patterns, support event propagation and bubbling, and provide a clean API that feels natural to developers coming from web or desktop UI frameworks.

## User Story

**As a** visualization developer  
**I want** a familiar, powerful event handling system  
**So that** I can easily create interactive visualizations with hover effects, click handlers, drag operations, and complex interaction patterns  

## Acceptance Criteria

### Event System Features

- [ ] **Familiar API**: Event handling that feels like DOM events or modern UI frameworks
- [ ] **Event Types**: Support for mouse, touch, keyboard, and custom events
- [ ] **Event Propagation**: Bubbling, capturing, and cancellation mechanisms
- [ ] **Event Filtering**: Ability to filter events by conditions and priorities

### Developer Experience

```rust
// Familiar event handling API
chart.select_all::<Circle>()
    .on("click", |event, data| {
        println!("Clicked circle with data: {:?}", data);
    })
    .on("hover", |event, data| {
        // Update tooltip or highlight
    })
    .on("drag", |event, data| {
        // Handle drag operations
    });
```

### Event Performance

- [ ] **Low Latency**: <16ms from user input to event handler execution
- [ ] **High Throughput**: Handle 1000+ events per second without performance degradation
- [ ] **Memory Efficiency**: Minimal memory overhead for event handler storage
- [ ] **Async Support**: Support for both synchronous and asynchronous event handlers

## Technical Tasks

### 1. Core Event System

- [ ] Define event types and event data structures
- [ ] Implement event handler registration and management
- [ ] Create event propagation and bubbling mechanism
- [ ] Add event filtering and priority systems

### 2. Input Event Processing

- [ ] Integrate with window/canvas input events
- [ ] Convert raw input to visualization-space coordinates
- [ ] Handle different input devices (mouse, touch, keyboard)
- [ ] Support platform-specific input patterns

### 3. Event-Interaction Bridge

- [ ] Connect GPU interaction results with event handlers
- [ ] Map interaction hits to data objects and event contexts
- [ ] Handle multi-selection and overlapping elements
- [ ] Provide event context and metadata

### 4. Advanced Event Features

- [ ] Implement gesture recognition (pinch, zoom, rotation)
- [ ] Add animation and transition support for event responses
- [ ] Create event recording and playback for testing
- [ ] Support custom event types and user-defined events

## Detailed Requirements

### Core Event Types

```rust
#[derive(Debug, Clone)]
pub enum EventType {
    // Mouse events
    MouseMove(Vec2),
    MouseDown(Vec2, MouseButton),
    MouseUp(Vec2, MouseButton),
    MouseEnter(Vec2),
    MouseLeave(Vec2),
    
    // Touch events
    TouchStart(Vec<TouchPoint>),
    TouchMove(Vec<TouchPoint>),
    TouchEnd(Vec<TouchPoint>),
    
    // Gesture events
    Pinch(PinchGesture),
    Zoom(ZoomGesture),
    Rotate(RotateGesture),
    
    // Custom events
    Custom(String, Box<dyn Any + Send + Sync>),
}

#[derive(Debug, Clone)]
pub struct InteractionEvent {
    pub event_type: EventType,
    pub position: Vec2,
    pub timestamp: Instant,
    pub modifiers: KeyModifiers,
    pub hit: Option<ElementHit>,
    pub propagation_stopped: bool,
    pub default_prevented: bool,
}

#[derive(Debug, Clone)]
pub struct ElementHit {
    pub element_id: u32,
    pub selection_id: SelectionId,
    pub distance: f32,
    pub intersection_point: Vec2,
    pub data_index: usize,
}
```

### Event Handler System

```rust
pub trait EventHandler: Send + Sync {
    fn handle_event(&self, event: &InteractionEvent, data: &dyn Any) -> EventResult;
    fn event_filter(&self) -> Option<EventFilter>;
    fn priority(&self) -> EventPriority { EventPriority::Normal }
}

pub struct EventManager {
    handlers: HashMap<EventType, Vec<HandlerEntry>>,
    global_handlers: Vec<HandlerEntry>,
    event_queue: AsyncQueue<InteractionEvent>,
    filter_cache: HashMap<EventFilter, bool>,
}

impl EventManager {
    pub fn register_handler<F, T>(&mut self, 
        event_type: EventType,
        handler: F,
    ) where 
        F: Fn(&InteractionEvent, &T) -> EventResult + Send + Sync + 'static,
        T: 'static,
    {
        let wrapped_handler = move |event: &InteractionEvent, data: &dyn Any| {
            if let Some(typed_data) = data.downcast_ref::<T>() {
                handler(event, typed_data)
            } else {
                EventResult::Continue
            }
        };
        
        self.handlers.entry(event_type)
            .or_default()
            .push(HandlerEntry::new(Box::new(wrapped_handler)));
    }
    
    pub async fn process_event(&mut self, event: InteractionEvent) {
        // Find affected elements through GPU interaction system
        let hits = self.interaction_system.query_event(&event).await;
        
        // Process event for each hit element
        for hit in hits {
            let mut event_with_hit = event.clone();
            event_with_hit.hit = Some(hit.clone());
            
            // Get data for hit element
            if let Some(data) = self.get_data_for_hit(&hit) {
                self.execute_handlers(&event_with_hit, data).await;
            }
            
            if event_with_hit.propagation_stopped {
                break;
            }
        }
        
        // Process global handlers
        self.execute_global_handlers(&event).await;
    }
}
```

### Selection Event Integration

```rust
impl<T, M: Mark> Selection<T, M> {
    pub fn on<F>(&mut self, event_type: &str, handler: F) -> &mut Self
    where F: Fn(&InteractionEvent, &T) + Send + Sync + 'static
    {
        let selection_id = self.id();
        let event_type = event_type.parse().unwrap_or(EventType::Custom(event_type.to_string(), Box::new(())));
        
        // Create handler that only fires for this selection's elements
        let filtered_handler = move |event: &InteractionEvent, data: &dyn Any| {
            if let Some(hit) = &event.hit {
                if hit.selection_id == selection_id {
                    if let Some(element_data) = data.downcast_ref::<T>() {
                        handler(event, element_data);
                        return EventResult::Handled;
                    }
                }
            }
            EventResult::Continue
        };
        
        self.event_manager.register_handler(event_type, filtered_handler);
        self
    }
    
    pub fn on_hover<F>(&mut self, handler: F) -> &mut Self
    where F: Fn(&T) + Send + Sync + 'static
    {
        self.on("mouseenter", move |_event, data| handler(data))
    }
    
    pub fn on_click<F>(&mut self, handler: F) -> &mut Self  
    where F: Fn(&T) + Send + Sync + 'static
    {
        self.on("click", move |_event, data| handler(data))
    }
    
    pub fn on_drag<F>(&mut self, handler: F) -> &mut Self
    where F: Fn(&T, Vec2, Vec2) + Send + Sync + 'static
    {
        let mut drag_start = None;
        
        self.on("mousedown", move |event, _data| {
            drag_start = Some(event.position);
        });
        
        self.on("mousemove", move |event, data| {
            if let Some(start_pos) = drag_start {
                handler(data, start_pos, event.position);
            }
        });
        
        self.on("mouseup", move |_event, _data| {
            drag_start = None;
        });
        
        self
    }
}
```

### Gesture Recognition System

```rust
pub struct GestureRecognizer {
    active_gestures: HashMap<GestureId, Box<dyn Gesture>>,
    gesture_history: VecDeque<InputEvent>,
    recognition_timeout: Duration,
}

pub trait Gesture: Send + Sync {
    fn update(&mut self, input: &InputEvent) -> GestureState;
    fn complete(&self) -> Option<EventType>;
    fn reset(&mut self);
}

pub struct PinchGesture {
    initial_distance: f32,
    current_distance: f32,
    center_point: Vec2,
    threshold: f32,
}

impl Gesture for PinchGesture {
    fn update(&mut self, input: &InputEvent) -> GestureState {
        match input {
            InputEvent::TouchMove(touches) if touches.len() == 2 => {
                let distance = (touches[0].position - touches[1].position).length();
                self.current_distance = distance;
                self.center_point = (touches[0].position + touches[1].position) * 0.5;
                
                if (self.current_distance - self.initial_distance).abs() > self.threshold {
                    GestureState::Active
                } else {
                    GestureState::Possible
                }
            }
            _ => GestureState::Failed
        }
    }
    
    fn complete(&self) -> Option<EventType> {
        Some(EventType::Pinch(PinchGestureData {
            scale: self.current_distance / self.initial_distance,
            center: self.center_point,
        }))
    }
}
```

## Dependencies

### Prerequisite Stories

- GUP-012: GPU Interaction System (provides interaction results)
- GUP-002: Core Selection Type (provides selections to add events to)

### Enables Stories

- GUP-014: Performance Validation (validates event handling performance)
- All interactive visualization features

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_event_handler_registration() {
    let mut manager = EventManager::new();
    let mut handler_called = false;
    
    manager.register_handler(EventType::MouseDown, |_event, _data: &TestData| {
        handler_called = true;
        EventResult::Handled
    });
    
    let event = create_test_mouse_event();
    manager.process_event(event).await;
    
    assert!(handler_called);
}

#[test]
fn test_event_propagation() {
    let mut manager = EventManager::new();
    let mut call_order = Vec::new();
    
    // Register multiple handlers with different priorities
    manager.register_handler_with_priority(
        EventType::MouseDown, 
        EventPriority::High,
        |_event, _data: &TestData| {
            call_order.push("high");
            EventResult::Continue
        }
    );
    
    manager.register_handler_with_priority(
        EventType::MouseDown,
        EventPriority::Normal, 
        |_event, _data: &TestData| {
            call_order.push("normal");
            EventResult::StopPropagation
        }
    );
    
    manager.register_handler_with_priority(
        EventType::MouseDown,
        EventPriority::Low,
        |_event, _data: &TestData| {
            call_order.push("low");
            EventResult::Continue
        }
    );
    
    let event = create_test_mouse_event();
    manager.process_event(event).await;
    
    assert_eq!(call_order, vec!["high", "normal"]);
    // "low" should not be called due to StopPropagation
}

#[test]
fn test_selection_event_filtering() {
    let mut selection1 = create_test_selection::<TestData, Circle>(1);
    let mut selection2 = create_test_selection::<TestData, Rectangle>(2);
    
    let mut selection1_clicked = false;
    let mut selection2_clicked = false;
    
    selection1.on("click", |_event, _data| {
        selection1_clicked = true;
    });
    
    selection2.on("click", |_event, _data| {
        selection2_clicked = true;
    });
    
    // Simulate click on selection1 element
    let event = create_click_event_for_selection(1, 0);
    process_event(event).await;
    
    assert!(selection1_clicked);
    assert!(!selection2_clicked);
}
```

### Integration Tests

```rust
#[test]
async fn test_complete_interaction_flow() {
    let device = create_test_device();
    let mut chart = Chart::new(&device);
    
    let mut tooltip_data = None;
    let mut click_count = 0;
    
    chart.select_all::<Circle>()
        .data(test_data)
        .on("hover", |_event, data| {
            tooltip_data = Some(data.clone());
        })
        .on("click", |_event, _data| {
            click_count += 1;
        });
    
    // Simulate mouse hover
    let hover_event = MouseEvent::Move(Vec2::new(50.0, 50.0));
    chart.process_input_event(hover_event).await;
    
    assert!(tooltip_data.is_some());
    assert_eq!(click_count, 0);
    
    // Simulate mouse click
    let click_event = MouseEvent::Down(Vec2::new(50.0, 50.0), MouseButton::Left);
    chart.process_input_event(click_event).await;
    
    assert_eq!(click_count, 1);
}

#[test]
async fn test_gesture_recognition() {
    let mut recognizer = GestureRecognizer::new();
    let mut pinch_detected = false;
    
    recognizer.register_gesture_handler("pinch", |gesture_data| {
        pinch_detected = true;
    });
    
    // Simulate pinch gesture
    let touch1_start = TouchEvent::Start(TouchPoint { id: 1, position: Vec2::new(100.0, 100.0) });
    let touch2_start = TouchEvent::Start(TouchPoint { id: 2, position: Vec2::new(200.0, 200.0) });
    
    recognizer.process_input(touch1_start).await;
    recognizer.process_input(touch2_start).await;
    
    // Move touches closer together
    let touch1_move = TouchEvent::Move(TouchPoint { id: 1, position: Vec2::new(120.0, 120.0) });
    let touch2_move = TouchEvent::Move(TouchPoint { id: 2, position: Vec2::new(180.0, 180.0) });
    
    recognizer.process_input(touch1_move).await;
    recognizer.process_input(touch2_move).await;
    
    assert!(pinch_detected);
}
```

### Performance Tests

```rust
#[bench]
async fn bench_event_processing_throughput(b: &mut Bencher) {
    let manager = create_event_manager_with_handlers(1000);
    let events = create_test_events(1000);
    
    b.iter(|| async {
        for event in &events {
            manager.process_event(event.clone()).await;
        }
    });
}

#[bench]
async fn bench_event_handler_latency(b: &mut Bencher) {
    let manager = create_event_manager();
    let event = create_test_event();
    
    b.iter(|| async {
        let start = Instant::now();
        manager.process_event(event.clone()).await;
        let latency = start.elapsed();
        assert!(latency < Duration::from_millis(16)); // <16ms target
    });
}
```

## Success Metrics

### Performance Requirements

- [ ] **Event Latency**: <16ms from input to handler execution
- [ ] **Throughput**: Handle 1000+ events per second
- [ ] **Memory Usage**: <1MB overhead for 10,000 event handlers
- [ ] **CPU Usage**: <5% CPU usage during typical interaction patterns

### API Usability Requirements

- [ ] **Intuitive API**: Event handling feels familiar to web/desktop developers
- [ ] **Type Safety**: Invalid event handler registrations caught at compile time
- [ ] **Error Handling**: Clear error messages for event handling failures
- [ ] **Documentation**: Complete examples for all common interaction patterns

### Feature Completeness Requirements

- [ ] **Event Types**: Support all common mouse, touch, and gesture events
- [ ] **Event Propagation**: Bubbling, capturing, and cancellation working correctly
- [ ] **Gesture Recognition**: Basic gestures (pinch, zoom, rotate) working
- [ ] **Custom Events**: Support for user-defined event types

## Risk Assessment

### Technical Risks

- **Medium**: Event handler performance could degrade with large numbers of handlers
- **Medium**: Event propagation complexity could introduce bugs
- **Low**: Platform differences in input handling could cause inconsistencies

### Mitigation Strategies

- **Performance Testing**: Continuous benchmarking of event processing performance
- **Reference Implementation**: Compare against established event systems for correctness
- **Platform Testing**: Test on all supported platforms for consistency

## Implementation Notes

### Design Decisions

- Use trait objects for event handlers to allow different closure types
- Implement event propagation similar to DOM events for familiarity
- Support both synchronous and asynchronous event handlers
- Use weak references to prevent memory leaks from event handler retention

### Event Processing Strategy

- Process events asynchronously to avoid blocking the main thread
- Use event queuing to handle bursts of input events smoothly
- Implement event coalescing for high-frequency events like mouse move
- Cache event handler lookups for performance

### Memory Management Strategy

- Use weak references for event handler storage to allow automatic cleanup
- Implement event handler cleanup when selections are dropped
- Pool event objects to reduce allocation overhead
- Use efficient data structures for handler lookup and iteration

## Definition of Done

- [ ] Event handling system integrated with GPU interaction system
- [ ] Familiar API for registering event handlers on selections
- [ ] Event propagation and bubbling working correctly
- [ ] Support for mouse, touch, and basic gesture events
- [ ] Performance targets met for latency and throughput
- [ ] Integration tests passing for complete interaction flows
- [ ] Gesture recognition working for basic gestures
- [ ] Cross-platform input handling consistency verified
- [ ] Memory management preventing leaks from event handlers
- [ ] Documentation complete with interaction examples
- [ ] Code review completed and approved
