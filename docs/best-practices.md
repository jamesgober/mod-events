<h1 align="center">
        <img width="108px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br>
    <strong>Mod Events</strong>
    <sup><br><sup>BEST PRACTICES</sup></sup>
</h1>

This guide covers recommended patterns and practices for using mod-events effectively.

## Event Design

### Keep Events Simple

```rust
// Good - simple, focused event
#[derive(Debug, Clone)]
struct UserRegistered {
    user_id: u64,
    email: String,
}

// Avoid - complex, multi-purpose event
#[derive(Debug, Clone)]
struct UserEvent {
    action: String,
    user_data: HashMap<String, Value>,
    metadata: Vec<String>,
    // ... too many fields
}
```

### Use Descriptive Names

```rust
// Good - clear intent
struct OrderPlaced { ... }
struct PaymentProcessed { ... }
struct EmailSent { ... }

// Avoid - vague names
struct UserThing { ... }
struct DataEvent { ... }
struct GenericEvent { ... }
```

### Implement Clone When Needed

```rust
// Events should be cloneable for sharing
#[derive(Debug, Clone)]
struct MyEvent {
    // Use types that implement Clone
    id: u64,
    message: String,
    timestamp: u64,
}
```

## Listener Design

### Prefer Simple Closures

```rust
// Good - simple and readable
dispatcher.on(|event: &UserRegistered| {
    log_user_registration(event);
});

// Good - with error handling
dispatcher.subscribe(|event: &UserRegistered| {
    send_welcome_email(&event.email)
        .map_err(|e| format!("Email failed: {}", e).into())
});
```

### Use Structs for Complex Logic

```rust
// Good - complex logic in dedicated struct
struct UserRegistrationHandler {
    email_service: EmailService,
    analytics: Analytics,
}

impl EventListener<UserRegistered> for UserRegistrationHandler {
    fn handle(&self, event: &UserRegistered) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.email_service.send_welcome(&event.email)?;
        self.analytics.track_registration(event.user_id)?;
        Ok(())
    }
}
```

## Error Handling

### Use Appropriate Error Types

```rust
// Good - specific error types
dispatcher.subscribe(|event: &UserRegistered| {
    if event.email.is_empty() {
        return Err("Email cannot be empty".into());
    }
    // ... handle event
    Ok(())
});

// Good - custom error types
#[derive(Debug)]
enum UserError {
    InvalidEmail,
    ServiceUnavailable,
}

impl std::fmt::Display for UserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for UserError {}
```

### Handle Errors Gracefully

```rust
let result = dispatcher.dispatch(event);

if result.has_errors() {
    for error in result.errors() {
        eprintln!("Handler error: {}", error);
        // Log, but don't crash
    }
}
```

## Architecture Patterns

### Domain-Driven Design

```rust
// Organize events by domain
mod user_events {
    use super::*;
    
    #[derive(Debug, Clone)]
    pub struct UserRegistered { ... }
    
    #[derive(Debug, Clone)]
    pub struct UserUpdated { ... }
}

mod order_events {
    use super::*;
    
    #[derive(Debug, Clone)]
    pub struct OrderPlaced { ... }
    
    #[derive(Debug, Clone)]
    pub struct OrderShipped { ... }
}
```

### Service Layer Pattern

```rust
struct UserService {
    dispatcher: EventDispatcher,
    repository: UserRepository,
}

impl UserService {
    fn register_user(&self, email: String) -> Result<u64, UserError> {
        let user_id = self.repository.create_user(email.clone())?;
        
        // Emit event after successful operation
        self.dispatcher.emit(UserRegistered {
            user_id,
            email,
            timestamp: now(),
        });
        
        Ok(user_id)
    }
}
```

## Async Patterns

### Prefer Async for I/O Operations

```rust
// Good - async for I/O
dispatcher.subscribe_async(|event: &UserRegistered| async {
    // Database write
    save_user_to_db(event).await?;
    
    // HTTP request
    send_webhook(event).await?;
    
    Ok(())
});

// Avoid - blocking I/O in sync handler
dispatcher.on(|event: &UserRegistered| {
    // This blocks the thread!
    std::thread::sleep(Duration::from_millis(100));
});
```

### Handle Async Errors

```rust
dispatcher.subscribe_async(|event: &UserRegistered| async {
    match send_email(&event.email).await {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Email failed: {}", e);
            // Decide whether to propagate error
            Ok(()) // Don't fail other handlers
        }
    }
});
```

## Testing

### Unit Test Event Handlers

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_user_registration_handler() {
        let dispatcher = EventDispatcher::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        
        dispatcher.on(move |_: &UserRegistered| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });
        
        dispatcher.emit(UserRegistered {
            user_id: 123,
            email: "test@example.com".to_string(),
            timestamp: 1234567890,
        });
        
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
```

### Integration Tests

```rust
#[test]
fn test_user_registration_flow() {
    let dispatcher = EventDispatcher::new();
    let results = Arc::new(Mutex::new(Vec::new()));
    
    // Set up handlers
    let results_clone = results.clone();
    dispatcher.on(move |event: &UserRegistered| {
        results_clone.lock().unwrap().push(format!("Email: {}", event.email));
    });
    
    let results_clone = results.clone();
    dispatcher.on(move |event: &UserRegistered| {
        results_clone.lock().unwrap().push(format!("Analytics: {}", event.user_id));
    });
    
    // Emit event
    dispatcher.emit(UserRegistered {
        user_id: 123,
        email: "test@example.com".to_string(),
        timestamp: 1234567890,
    });
    
    // Verify results
    let results = results.lock().unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|r| r.contains("Email")));
    assert!(results.iter().any(|r| r.contains("Analytics")));
}
```

## Performance Tips

### Use Appropriate Dispatch Methods

```rust
// Fastest - fire and forget
dispatcher.emit(event);

// Slower - when you need results
let result = dispatcher.dispatch(event);
```

### Batch Operations

```rust
// Good - batch related events
for user in users {
    dispatcher.emit(UserRegistered { ... });
}
```

### Monitor Performance

```rust
// Use built-in metrics
let metrics = dispatcher.metrics();
for (_, meta) in metrics {
    if meta.dispatch_count > 10000 {
        println!("High frequency event: {}", meta.event_name);
    }
}
```

## Common Pitfalls

### Avoid Circular Events

```rust
// Bad - can cause infinite loops
dispatcher.on(|event: &UserRegistered| {
    // This can create a cycle!
    dispatcher.emit(UserUpdated { ... });
});

dispatcher.on(|event: &UserUpdated| {
    // If this emits UserRegistered, we have a cycle!
    dispatcher.emit(UserRegistered { ... });
});
```

### Don't Block in Handlers

```rust
// Bad - blocks the thread
dispatcher.on(|event: &UserRegistered| {
    std::thread::sleep(Duration::from_secs(1));
});

// Good - use async for delays
dispatcher.subscribe_async(|event: &UserRegistered| async {
    tokio::time::sleep(Duration::from_secs(1)).await;
    Ok(())
});
```

### Handle Unsubscription Properly

```rust
// Good - store listener ID for cleanup
let listener_id = dispatcher.on(|event: &UserRegistered| {
    // Handle event
});

// Later, clean up
dispatcher.unsubscribe(listener_id);
```

## Conclusion

Following these best practices will help you build maintainable, performant, and robust event-driven applications with mod-events.

Key takeaways:
- Keep events simple and focused
- Use appropriate error handling
- Prefer async for I/O operations
- Test your event handlers
- Monitor performance
- Avoid common pitfalls


<br>

## Next Steps

- Get Started [Quick Start Guide](quick-start.md)
- Read the [API Reference](api-reference.md)
- Check out more [Examples](examples.md)
- Review [Performance Guide](performance.md)
- See the [Migration Guide](migration.md)