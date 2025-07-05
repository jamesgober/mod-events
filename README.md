<div align="center">
        <img width="120px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <h1>
        <strong>Mod Events</strong>
        <sup><br><sup>RUST EVENTS LIBRARY</sup><br></sup>
    </h1>
    <div>
        <a href="https://crates.io/crates/mod-events" alt="Mod Events on Crates.io"><img alt="Crates.io" src="https://img.shields.io/crates/v/mod-events"></a>
        <span>&nbsp;</span>
        <a href="https://crates.io/crates/mod-events" alt="Download Mod Events"><img alt="Crates.io Downloads" src="https://img.shields.io/crates/d/mod-events?color=%230099ff"></a>
        <span>&nbsp;</span>
        <a href="https://docs.rs/mod-events" title="Mod Events Documentation"><img alt="docs.rs" src="https://img.shields.io/docsrs/mod-events"></a>
        <span>&nbsp;</span>
        <img alt="GitHub last commit" src="https://img.shields.io/github/last-commit/jamesgober/mod-events?color=%23347d39" alt="last commit badge">
    </div>
</div>

<div>
    <br>
    <p>
        A high-performance, zero-overhead <strong>event dispatcher</strong> library for <b>Rust</b> that implements the <em>observer pattern</em> with compile-time type safety and runtime efficiency.
    </p>
    <p>
        This library enables decoupled, event-driven architectures by allowing components to communicate through strongly-typed events without direct dependencies. <b>Built for performance</b>, it uses <b>zero-cost abstractions</b> and <b>efficient memory management</b> to ensure event dispatch has <b>minimal runtime overhead</b>, making it suitable for high-throughput applications, real-time systems, and microservice architectures.
    </p>
    <p>
         This <strong>event dispatcher</strong> supports both <em>synchronous</em> and <em>asynchronous</em> event handling with a <em>priority-based</em> execution system that allows fine-grained control over listener execution order. 
    </p>
    <p>
         <b>Thread-safe</b> by design, it handles <b>concurrent access</b> efficiently using read-write locks that allow multiple threads to dispatch events simultaneously without blocking. 
    </p>
    <p>
         This library includes a <em>flexible middleware system</em> for event filtering, transformation, and logging. It uses a comprehensive error handler that doesn't stop event propagation on individual listener failures, and it has built-in metrics for monitoring event dispatch performance and debugging. 
    </p>
    <p>
         Unlike string-based event systems common in other languages, this library leverages Rust's type system to prevent runtime errors and ensure listeners receive correctly typed events.
    </p>
    <br>
</div>


## Key Features

- **Zero-cost abstractions**: *No runtime overhead for event dispatch*.
- **Type-safe**: *Compile-time guarantees for event handling*.
- **Thread-safe**: *Built for concurrent applications*.
- **Async support**: *Full async/await compatibility*.
- **Flexible**: *Support for sync, async, and priority-based listeners*.
- **Easy to use**: *Simple API and intuitive methods*.
- **Performance**: *Optimized for high-throughput scenarios*.
- **Monitoring**: *Built-in metrics and middleware support*.

<br>


## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
mod-events = "0.1"
```

## Basic Usage

```rust
use mod_events::prelude::*;

// Define your event
#[derive(Debug, Clone)]
struct UserRegistered {
    user_id: u64,
    email: String,
}

impl Event for UserRegistered {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// Create dispatcher and subscribe
let dispatcher = EventDispatcher::new();
dispatcher.on(|event: &UserRegistered| {
    println!("Welcome {}!", event.email);
});

// Dispatch events
dispatcher.emit(UserRegistered {
    user_id: 123,
    email: "alice@example.com".to_string(),
});
```


## Features

### Priority System

```rust
use mod_events::{EventDispatcher, Priority};

let dispatcher = EventDispatcher::new();

// High priority listener executes first
dispatcher.subscribe_with_priority(|event: &MyEvent| {
    println!("High priority handler");
    Ok(())
}, Priority::High);

// Normal priority listener executes second
dispatcher.on(|event: &MyEvent| {
    println!("Normal priority handler");
});
```

### Async Support

```rust
// Enable with the "async" feature
dispatcher.subscribe_async(|event: &MyEvent| async {
    // Async processing
    tokio::time::sleep(Duration::from_millis(100)).await;
    println!("Async handler completed");
    Ok(())
});

let result = dispatcher.dispatch_async(MyEvent { /* ... */ }).await;
```

### Middleware

```rust
// Add middleware for logging, filtering, etc.
dispatcher.add_middleware(|event: &dyn Event| {
    println!("Processing: {}", event.event_name());
    true // Allow event to continue
});
```

### Error Handling

```rust
let result = dispatcher.dispatch(MyEvent { /* ... */ });

if result.all_succeeded() {
    println!("All handlers succeeded");
} else {
    for error in result.errors() {
        eprintln!("Handler error: {}", error);
    }
}
```

## Examples

Run the examples:

```bash
cargo run --example basic_usage
cargo run --features async --example async_usage
```

## Benchmarks

```bash
cargo test --release benchmark
```

## Documentation

- [Quick Start Guide]([.](https://github.com/jamesgober/mod-events/blob/da3d75763047fd9427480cbfd08425f050dc2b75)/docs/quick-start.md)
- [API Reference](https://github.com/jamesgober/mod-events/blob/da3d75763047fd9427480cbfd08425f050dc2b75/docs/api-reference.md) 
- [Performance Guide](https://github.com/jamesgober/mod-events/blob/da3d75763047fd9427480cbfd08425f050dc2b75/docs/performance.md)
- [Examples](https://github.com/jamesgober/mod-events/blob/da3d75763047fd9427480cbfd08425f050dc2b75/docs/examples.md)
- [Best Practices](https://github.com/jamesgober/mod-events/blob/da3d75763047fd9427480cbfd08425f050dc2b75/docs/best-practices.md)
- [Migration Guide](https://github.com/jamesgober/mod-events/blob/da3d75763047fd9427480cbfd08425f050dc2b75/docs/migration.md)

<br>

<!--
:: LICENSE
============================================================================ -->
<div id="license">
    <hr>
    <h2>📌 License</h2>
    <p>Licensed under the <b>Apache License</b>, version 2.0 (the <b>"License"</b>); you may not use this software, including, but not limited to the source code, media files, ideas, techniques, or any other associated property or concept belonging to, associated with, or otherwise packaged with this software except in compliance with the <b>License</b>.</p>
    <p>You may obtain a copy of the <b>License</b> at: <a href="http://www.apache.org/licenses/LICENSE-2.0" title="Apache-2.0 License" target="_blank">http://www.apache.org/licenses/LICENSE-2.0</a>.</p>
    <p>Unless required by applicable law or agreed to in writing, software distributed under the <b>License</b> is distributed on an "<b>AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND</b>, either express or implied.</p>
    <p>See the <a href="./LICENSE" title="Software License file">LICENSE</a> file included with this project for the specific language governing permissions and limitations under the <b>License</b>.</p>
    <br>
</div>

<!--
:: COPYRIGHT
============================================================================ -->
<div align="center">
  <br>
  <h2></h2>
  <sup>COPYRIGHT <small>&copy;</small> 2025 <strong>JAMES GOBER.</strong></sup>
</div>