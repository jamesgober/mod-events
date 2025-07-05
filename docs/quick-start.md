<h1 align="center">
        <img width="108px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br>
    <strong>Mod Events</strong>
    <sup><br><sup>Quick Start Guide</sup></sup>
</h1>

This guide will get you up and running with mod-events in under 5 minutes.

## Installation

Add mod-events to your `Cargo.toml`:

```toml
[dependencies]
mod-events = "0.1"

# For async support
mod-events = { version = "0.1", features = ["async"] }
```

## Basic Usage

### 1. Define Your Events

Events are just structs that implement the `Event` trait:

```rust
use mod_events::Event;

#[derive(Debug, Clone)]
struct UserRegistered {
    user_id: u64,
    email: String,
    timestamp: u64,
}

impl Event for UserRegistered {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
```

### 2. Create a Dispatcher

```rust
use mod_events::EventDispatcher;

let dispatcher = EventDispatcher::new();
```

### 3. Subscribe to Events

```rust
// Simple subscription
dispatcher.on(|event: &UserRegistered| {
    println!("User {} registered!", event.user_id);
});

// With error handling
dispatcher.subscribe(|event: &UserRegistered| {
    // Your logic here
    if event.email.is_empty() {
        return Err("Email cannot be empty".into());
    }
    println!("Processing user: {}", event.email);
    Ok(())
});
```

### 4. Dispatch Events

```rust
// Fire and forget
dispatcher.emit(UserRegistered {
    user_id: 123,
    email: "alice@example.com".to_string(),
    timestamp: 1234567890,
});

// Check results
let result = dispatcher.dispatch(UserRegistered {
    user_id: 456,
    email: "bob@example.com".to_string(),
    timestamp: 1234567891,
});

if result.all_succeeded() {
    println!("All handlers succeeded!");
}
```

## Advanced Features

### Priority System

```rust
use mod_events::Priority;

// High priority runs first
dispatcher.subscribe_with_priority(|event: &UserRegistered| {
    println!("High priority handler");
    Ok(())
}, Priority::High);

// Normal priority runs second
dispatcher.on(|event: &UserRegistered| {
    println!("Normal priority handler");
});
```

### Middleware

```rust
// Add logging middleware
dispatcher.add_middleware(|event: &dyn Event| {
    println!("Event: {}", event.event_name());
    true // Allow event to continue
});

// Add filtering middleware
dispatcher.add_middleware(|event: &dyn Event| {
    // Block events during maintenance
    !is_maintenance_mode()
});
```

### Async Support

```rust
// Async event handler
dispatcher.subscribe_async(|event: &UserRegistered| async {
    send_welcome_email(&event.email).await?;
    Ok(())
});

// Dispatch async
let result = dispatcher.dispatch_async(user_event).await;
```

<br>

## Next Steps

- Read the [API Reference](api-reference.md)
- Check out more [Examples](examples.md)
- Learn [Best Practices](best-practices.md)
- Review [Performance Guide](performance.md)
- See the [Migration Guide](migration.md)