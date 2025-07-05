<h1 align="center">
        <img width="108px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br>
    <strong>Mod Events</strong>
    <sup><br><sup>API REFERENCE</sup></sup>
</h1>

Complete API documentation for mod-events.

## Table of Contents

- [Core Traits](#core-traits)
- [EventDispatcher](#eventdispatcher)
- [Priority System](#priority-system)
- [Event Listeners](#event-listeners)
- [Results and Metrics](#results-and-metrics)
- [Async Support](#async-support)
- [Middleware](#middleware)
- [Type Aliases](#type-aliases)

## Core Traits

### Event

The fundamental trait that all events must implement.

```rust
pub trait Event: Any + Send + Sync + fmt::Debug {
    fn as_any(&self) -> &dyn Any;
    fn type_id(&self) -> TypeId { ... }
    fn event_name(&self) -> &'static str { ... }
}
```

#### Required Methods

- **`as_any(&self) -> &dyn Any`** - Returns the event as `Any` for downcasting

#### Provided Methods

- **`type_id(&self) -> TypeId`** - Returns a unique identifier for this event type
- **`event_name(&self) -> &'static str`** - Returns the event name for debugging

#### Example

```rust
use mod_events::Event;

#[derive(Debug, Clone)]
struct MyEvent {
    message: String,
}

impl Event for MyEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
```

## EventDispatcher

The main component for dispatching events to listeners.

```rust
pub struct EventDispatcher { /* private fields */ }
```

### Creation

#### `new() -> Self`

Creates a new event dispatcher.

```rust
let dispatcher = EventDispatcher::new();
```

### Event Subscription

#### `on<T, F>(&self, listener: F) -> ListenerId`

Subscribe to an event with a simple closure (no error handling).

**Parameters:**
- `T: Event + 'static` - The event type
- `F: Fn(&T) + Send + Sync + 'static` - The listener function

**Returns:** `ListenerId` - Unique identifier for the listener

```rust
let id = dispatcher.on(|event: &MyEvent| {
    println!("Received: {}", event.message);
});
```

#### `subscribe<T, F>(&self, listener: F) -> ListenerId`

Subscribe to an event with error handling.

**Parameters:**
- `T: Event + 'static` - The event type
- `F: Fn(&T) -> Result<(), Box<dyn Error + Send + Sync>> + Send + Sync + 'static` - The listener function

**Returns:** `ListenerId` - Unique identifier for the listener

```rust
let id = dispatcher.subscribe(|event: &MyEvent| {
    if event.message.is_empty() {
        return Err("Message cannot be empty".into());
    }
    println!("Message: {}", event.message);
    Ok(())
});
```

#### `subscribe_with_priority<T, F>(&self, listener: F, priority: Priority) -> ListenerId`

Subscribe to an event with a specific priority.

**Parameters:**
- `T: Event + 'static` - The event type
- `F: Fn(&T) -> Result<(), Box<dyn Error + Send + Sync>> + Send + Sync + 'static` - The listener function
- `priority: Priority` - The priority level

**Returns:** `ListenerId` - Unique identifier for the listener

```rust
let id = dispatcher.subscribe_with_priority(|event: &MyEvent| {
    println!("High priority handler");
    Ok(())
}, Priority::High);
```

### Async Event Subscription

*Available with the `async` feature*

#### `subscribe_async<T, F, Fut>(&self, listener: F) -> ListenerId`

Subscribe to an event with an async handler.

**Parameters:**
- `T: Event + 'static` - The event type
- `F: Fn(&T) -> Fut + Send + Sync + 'static` - The async listener function
- `Fut: Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send + 'static` - The future type

**Returns:** `ListenerId` - Unique identifier for the listener

```rust
let id = dispatcher.subscribe_async(|event: &MyEvent| async {
    // Async processing
    tokio::time::sleep(Duration::from_millis(100)).await;
    println!("Async handler completed");
    Ok(())
});
```

#### `subscribe_async_with_priority<T, F, Fut>(&self, listener: F, priority: Priority) -> ListenerId`

Subscribe to an event with an async handler and specific priority.

**Parameters:**
- `T: Event + 'static` - The event type
- `F: Fn(&T) -> Fut + Send + Sync + 'static` - The async listener function
- `Fut: Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send + 'static` - The future type
- `priority: Priority` - The priority level

**Returns:** `ListenerId` - Unique identifier for the listener

```rust
let id = dispatcher.subscribe_async_with_priority(|event: &MyEvent| async {
    // High priority async processing
    Ok(())
}, Priority::High);
```

### Event Dispatching

#### `emit<T: Event>(&self, event: T)`

Fire and forget event dispatch - fastest method.

**Parameters:**
- `event: T` - The event to dispatch

```rust
dispatcher.emit(MyEvent {
    message: "Hello World".to_string(),
});
```

#### `dispatch<T: Event>(&self, event: T) -> DispatchResult`

Dispatch an event and return results.

**Parameters:**
- `event: T` - The event to dispatch

**Returns:** `DispatchResult` - Information about the dispatch

```rust
let result = dispatcher.dispatch(MyEvent {
    message: "Hello".to_string(),
});

if result.all_succeeded() {
    println!("All handlers succeeded");
}
```

#### `dispatch_async<T: Event>(&self, event: T) -> impl Future<Output = DispatchResult>`

*Available with the `async` feature*

Dispatch an event asynchronously.

**Parameters:**
- `event: T` - The event to dispatch

**Returns:** `Future<Output = DispatchResult>` - Future resolving to dispatch results

```rust
let result = dispatcher.dispatch_async(MyEvent {
    message: "Async Hello".to_string(),
}).await;
```

### Listener Management

#### `unsubscribe(&self, listener_id: ListenerId) -> bool`

Remove a listener.

**Parameters:**
- `listener_id: ListenerId` - The listener to remove

**Returns:** `bool` - `true` if the listener was found and removed

```rust
let id = dispatcher.on(|event: &MyEvent| {
    println!("Handler");
});

let removed = dispatcher.unsubscribe(id);
assert!(removed);
```

#### `listener_count<T: Event + 'static>(&self) -> usize`

Get the number of listeners for an event type.

**Parameters:**
- `T: Event + 'static` - The event type

**Returns:** `usize` - Number of listeners

```rust
let count = dispatcher.listener_count::<MyEvent>();
println!("Listeners: {}", count);
```

#### `clear(&self)`

Remove all listeners.

```rust
dispatcher.clear();
```

### Middleware

#### `add_middleware<F>(&self, middleware: F)`

Add middleware that can filter or transform events.

**Parameters:**
- `F: Fn(&dyn Event) -> bool + Send + Sync + 'static` - Middleware function returning `true` to allow the event

```rust
dispatcher.add_middleware(|event: &dyn Event| {
    println!("Processing: {}", event.event_name());
    true // Allow all events
});
```

### Metrics

#### `metrics(&self) -> HashMap<TypeId, EventMetadata>`

Get event dispatch metrics.

**Returns:** `HashMap<TypeId, EventMetadata>` - Metrics for each event type

```rust
let metrics = dispatcher.metrics();
for (_, meta) in metrics {
    println!("Event: {} dispatched {} times", 
        meta.event_name, meta.dispatch_count);
}
```

## Priority System

### Priority

Enum defining listener execution priority.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Lowest = 0,
    Low = 25,
    Normal = 50,     // Default
    High = 75,
    Highest = 100,
    Critical = 125,
}
```

#### Values

- **`Lowest`** - Lowest priority (0)
- **`Low`** - Low priority (25)
- **`Normal`** - Normal priority (50) - Default
- **`High`** - High priority (75)
- **`Highest`** - Highest priority (100)
- **`Critical`** - Critical priority (125) - Use sparingly

#### Methods

##### `all() -> &'static [Priority]`

Get all priority levels in order.

```rust
let priorities = Priority::all();
for priority in priorities {
    println!("Priority: {:?}", priority);
}
```

#### Example

```rust
use mod_events::Priority;

// Critical operations first
dispatcher.subscribe_with_priority(|event: &PaymentEvent| {
    process_payment(event)?;
    Ok(())
}, Priority::Critical);

// Normal operations
dispatcher.on(|event: &PaymentEvent| {
    send_receipt(event);
});
```

## Event Listeners

### EventListener

Trait for reusable event listeners.

```rust
pub trait EventListener<T: Event>: Send + Sync {
    fn handle(&self, event: &T) -> Result<(), Box<dyn Error + Send + Sync>>;
    fn priority(&self) -> Priority { Priority::Normal }
}
```

#### Required Methods

- **`handle(&self, event: &T) -> Result<(), Box<dyn Error + Send + Sync>>`** - Handle the event

#### Provided Methods

- **`priority(&self) -> Priority`** - Get the priority (default: `Priority::Normal`)

#### Example

```rust
use mod_events::{Event, EventListener, Priority};

struct EmailNotifier {
    smtp_server: String,
}

impl EventListener<UserRegistered> for EmailNotifier {
    fn handle(&self, event: &UserRegistered) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        send_email(&self.smtp_server, &event.email)?;
        Ok(())
    }
    
    fn priority(&self) -> Priority {
        Priority::High
    }
}
```

### AsyncEventListener

*Available with the `async` feature*

Trait for async event listeners.

```rust
pub trait AsyncEventListener<T: Event>: Send + Sync {
    fn handle<'a>(&'a self, event: &'a T) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn Error + Send + Sync>>> + Send + 'a>>;
    fn priority(&self) -> Priority { Priority::Normal }
}
```

#### Required Methods

- **`handle<'a>(&'a self, event: &'a T) -> Pin<Box<dyn Future<...> + Send + 'a>>`** - Handle the event asynchronously

#### Provided Methods

- **`priority(&self) -> Priority`** - Get the priority (default: `Priority::Normal`)

#### Example

```rust
use mod_events::{AsyncEventListener, Priority};

struct AsyncEmailNotifier {
    smtp_client: AsyncSmtpClient,
}

impl AsyncEventListener<UserRegistered> for AsyncEmailNotifier {
    fn handle<'a>(&'a self, event: &'a UserRegistered) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>> {
        Box::pin(async move {
            self.smtp_client.send_email(&event.email).await?;
            Ok(())
        })
    }
    
    fn priority(&self) -> Priority {
        Priority::High
    }
}
```

## Results and Metrics

### DispatchResult

Information about event dispatch results.

```rust
pub struct DispatchResult { /* private fields */ }
```

#### Methods

##### `is_blocked(&self) -> bool`

Check if the event was blocked by middleware.

```rust
let result = dispatcher.dispatch(event);
if result.is_blocked() {
    println!("Event was blocked by middleware");
}
```

##### `listener_count(&self) -> usize`

Get the total number of listeners that were called.

```rust
let count = result.listener_count();
println!("Called {} listeners", count);
```

##### `success_count(&self) -> usize`

Get the number of successful handlers.

```rust
let successes = result.success_count();
println!("{} handlers succeeded", successes);
```

##### `error_count(&self) -> usize`

Get the number of failed handlers.

```rust
let errors = result.error_count();
println!("{} handlers failed", errors);
```

##### `all_succeeded(&self) -> bool`

Check if all handlers succeeded.

```rust
if result.all_succeeded() {
    println!("All handlers completed successfully");
}
```

##### `has_errors(&self) -> bool`

Check if any handlers failed.

```rust
if result.has_errors() {
    println!("Some handlers failed");
}
```

##### `errors(&self) -> Vec<&(dyn Error + Send + Sync)>`

Get all errors that occurred during dispatch.

```rust
for error in result.errors() {
    eprintln!("Handler error: {}", error);
}
```

### EventMetadata

Metadata about event dispatch history.

```rust
pub struct EventMetadata {
    pub event_name: &'static str,
    pub type_id: TypeId,
    pub last_dispatch: Instant,
    pub dispatch_count: usize,
    pub listener_count: usize,
}
```

#### Fields

- **`event_name`** - The name of the event type
- **`type_id`** - Type ID of the event
- **`last_dispatch`** - Timestamp of the last dispatch
- **`dispatch_count`** - Total number of times this event has been dispatched
- **`listener_count`** - Number of listeners currently subscribed

#### Methods

##### `time_since_last_dispatch(&self) -> Duration`

Get the time since the last dispatch.

```rust
let metrics = dispatcher.metrics();
for (_, meta) in metrics {
    let elapsed = meta.time_since_last_dispatch();
    println!("Last {} dispatch: {:?} ago", meta.event_name, elapsed);
}
```

### ListenerId

Unique identifier for event listeners.

```rust
pub struct ListenerId { /* private fields */ }
```

Used for unsubscribing listeners:

```rust
let id = dispatcher.on(|event: &MyEvent| {
    println!("Handler");
});

dispatcher.unsubscribe(id);
```

## Middleware

### MiddlewareFunction

Type alias for middleware functions.

```rust
pub type MiddlewareFunction = Box<dyn Fn(&dyn Event) -> bool + Send + Sync>;
```

Middleware functions receive an event and return `true` to allow processing or `false` to block it.

### MiddlewareManager

Manages middleware execution.

```rust
pub struct MiddlewareManager { /* private fields */ }
```

#### Methods

##### `new() -> Self`

Create a new middleware manager.

##### `add<F>(&mut self, middleware: F)`

Add middleware to the chain.

**Parameters:**
- `F: Fn(&dyn Event) -> bool + Send + Sync + 'static` - Middleware function

##### `process(&self, event: &dyn Event) -> bool`

Process an event through all middleware.

**Returns:** `bool` - `true` if the event should continue

##### `count(&self) -> usize`

Get the number of middleware functions.

##### `clear(&mut self)`

Remove all middleware.

## Type Aliases

### AsyncResult

*Available with the `async` feature*

```rust
type AsyncResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
```

Standard result type for async operations.

### AsyncHandler

*Available with the `async` feature*

```rust
type AsyncHandler = Arc<dyn for<'a> Fn(&'a dyn Event) -> Pin<Box<dyn Future<Output = AsyncResult> + Send + 'a>> + Send + Sync>;
```

Type alias for async event handlers.

## Feature Flags

### `async`

Enables async event handling support.

```toml
[dependencies]
mod-events = { version = "0.1", features = ["async"] }
```

When enabled, provides:
- `subscribe_async` methods
- `dispatch_async` method
- `AsyncEventListener` trait
- Async-related type aliases

## Thread Safety

All types in mod-events are thread-safe:

- `EventDispatcher` implements `Send + Sync`
- Events must implement `Send + Sync`
- Listeners must implement `Send + Sync`
- All operations are safe for concurrent use

## Performance Characteristics

- **Event dispatch**: ~1-2 microseconds
- **Memory overhead**: ~200 bytes per dispatcher
- **Scaling**: Linear with number of listeners
- **Thread contention**: Minimal (read-heavy workload)

## Error Handling

The library uses standard Rust error handling:

- `Result<T, E>` for fallible operations
- `Box<dyn Error + Send + Sync>` for dynamic errors
- Individual listener failures don't affect other listeners
- Middleware can block events by returning `false`

## Best Practices

1. **Keep events simple** - Only essential data
2. **Use appropriate priorities** - Critical operations first
3. **Handle errors gracefully** - Don't panic in listeners
4. **Prefer `emit()` for fire-and-forget** - Better performance
5. **Use middleware for cross-cutting concerns** - Logging, metrics
6. **Monitor with metrics** - Track dispatch counts and timing

## Examples

See the [Examples](examples.md) documentation for comprehensive usage examples.

## Migration Guide

See the [Migration Guide](migration.md) for upgrading from other event systems.

<br>

## Next Steps

- Get Started [Quick Start Guide](quick-start.md)
- Check out more [Examples](examples.md)
- Learn [Best Practices](best-practices.md)
- Review [Performance Guide](performance.md)
- See the [Migration Guide](migration.md)