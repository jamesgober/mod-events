<h1 align="center">
        <img width="108px" height="auto" src="https://raw.githubusercontent.com/jamesgober/jamesgober/main/media/icons/hexagon-3.svg" alt="Triple Hexagon">
    <br>
    <strong>Mod Events</strong>
    <sup><br><sup>USAGE &amp; EXAMPLES</sup></sup>
</h1>

Comprehensive examples showing different use cases and patterns with mod-events.

## Basic Examples

### Simple Event Logging

```rust
use mod_events::prelude::*;

#[derive(Debug, Clone)]
struct LogEvent {
    level: String,
    message: String,
    timestamp: u64,
}

impl Event for LogEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn main() {
    let dispatcher = EventDispatcher::new();
    
    // Console logger
    dispatcher.on(|event: &LogEvent| {
        println!("[{}] {}: {}", event.timestamp, event.level, event.message);
    });
    
    // File logger (with error handling)
    dispatcher.subscribe(|event: &LogEvent| {
        write_to_file(&event.message)?;
        Ok(())
    });
    
    // Emit log events
    dispatcher.emit(LogEvent {
        level: "INFO".to_string(),
        message: "Application started".to_string(),
        timestamp: now(),
    });
}

fn write_to_file(message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::fs::OpenOptions;
    use std::io::Write;
    
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("app.log")?;
    
    writeln!(file, "{}", message)?;
    Ok(())
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
```

### E-commerce Order Processing

```rust
use mod_events::prelude::*;

#[derive(Debug, Clone)]
struct OrderPlaced {
    order_id: u64,
    user_id: u64,
    amount: f64,
    items: Vec<String>,
}

impl Event for OrderPlaced {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone)]
struct PaymentProcessed {
    order_id: u64,
    amount: f64,
    payment_method: String,
}

impl Event for PaymentProcessed {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn main() {
    let dispatcher = EventDispatcher::new();
    
    // Order processing pipeline
    dispatcher.subscribe_with_priority(|event: &OrderPlaced| {
        validate_order(event)?;
        Ok(())
    }, Priority::Highest);
    
    dispatcher.on(|event: &OrderPlaced| {
        update_inventory(&event.items);
    });
    
    dispatcher.on(|event: &OrderPlaced| {
        send_order_confirmation(event.user_id, event.order_id);
    });
    
    // Payment processing
    dispatcher.on(|event: &PaymentProcessed| {
        update_order_status(event.order_id, "paid");
    });
    
    dispatcher.on(|event: &PaymentProcessed| {
        trigger_fulfillment(event.order_id);
    });
    
    // Process an order
    dispatcher.emit(OrderPlaced {
        order_id: 123,
        user_id: 456,
        amount: 99.99,
        items: vec!["laptop".to_string(), "mouse".to_string()],
    });
}

fn validate_order(order: &OrderPlaced) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if order.amount <= 0.0 {
        return Err("Invalid order amount".into());
    }
    if order.items.is_empty() {
        return Err("Order must have items".into());
    }
    Ok(())
}

fn update_inventory(items: &[String]) {
    for item in items {
        println!("📦 Updating inventory for: {}", item);
    }
}

fn send_order_confirmation(user_id: u64, order_id: u64) {
    println!("📧 Sending order confirmation to user {} for order {}", user_id, order_id);
}

fn update_order_status(order_id: u64, status: &str) {
    println!("📋 Order {} status updated to: {}", order_id, status);
}

fn trigger_fulfillment(order_id: u64) {
    println!("🚚 Triggering fulfillment for order {}", order_id);
}
```

## Advanced Examples

### Game Event System

```rust
use mod_events::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
struct PlayerMoved {
    player_id: u64,
    x: f32,
    y: f32,
    timestamp: u64,
}

impl Event for PlayerMoved {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone)]
struct PlayerDied {
    player_id: u64,
    killer_id: Option<u64>,
    position: (f32, f32),
}

impl Event for PlayerDied {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone)]
struct GameFrameUpdate {
    frame_number: u64,
    delta_time: f32,
}

impl Event for GameFrameUpdate {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct GameSystem {
    dispatcher: EventDispatcher,
    player_positions: Arc<Mutex<HashMap<u64, (f32, f32)>>>,
}

impl GameSystem {
    fn new() -> Self {
        let dispatcher = EventDispatcher::new();
        let player_positions = Arc::new(Mutex::new(HashMap::new()));
        
        // High priority - update game state
        let positions = player_positions.clone();
        dispatcher.subscribe_with_priority(move |event: &PlayerMoved| {
            let mut positions = positions.lock().unwrap();
            positions.insert(event.player_id, (event.x, event.y));
            println!("🎮 Player {} moved to ({}, {})", event.player_id, event.x, event.y);
            Ok(())
        }, Priority::High);
        
        // Normal priority - handle player death
        dispatcher.on(|event: &PlayerDied| {
            if let Some(killer_id) = event.killer_id {
                println!("💀 Player {} was killed by player {} at ({}, {})", 
                    event.player_id, killer_id, event.position.0, event.position.1);
            } else {
                println!("💀 Player {} died at ({}, {})", 
                    event.player_id, event.position.0, event.position.1);
            }
        });
        
        // Low priority - analytics
        dispatcher.subscribe_with_priority(|event: &PlayerMoved| {
            record_player_movement(event.player_id, event.x, event.y);
            Ok(())
        }, Priority::Low);
        
        // Frame update handling
        dispatcher.on(|event: &GameFrameUpdate| {
            if event.frame_number % 60 == 0 {
                println!("🎯 Frame {} - Delta: {:.2}ms", event.frame_number, event.delta_time * 1000.0);
            }
        });
        
        Self {
            dispatcher,
            player_positions,
        }
    }
    
    fn game_loop(&self) {
        for frame in 0..180 { // 3 seconds at 60 FPS
            // Simulate frame update
            self.dispatcher.emit(GameFrameUpdate {
                frame_number: frame,
                delta_time: 0.016, // 60 FPS
            });
            
            // Simulate player movement
            if frame % 30 == 0 {
                self.dispatcher.emit(PlayerMoved {
                    player_id: 1,
                    x: (frame as f32) * 0.1,
                    y: (frame as f32) * 0.05,
                    timestamp: frame,
                });
            }
            
            // Simulate player death
            if frame == 150 {
                self.dispatcher.emit(PlayerDied {
                    player_id: 1,
                    killer_id: Some(2),
                    position: (15.0, 7.5),
                });
            }
            
            // Simulate frame timing
            std::thread::sleep(std::time::Duration::from_millis(16));
        }
    }
}

fn record_player_movement(player_id: u64, x: f32, y: f32) {
    // Analytics recording
    println!("📊 Analytics: Player {} at ({:.2}, {:.2})", player_id, x, y);
}

fn main() {
    let game = GameSystem::new();
    game.game_loop();
}
```

### Microservices Event Bus

```rust
use mod_events::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct UserRegistered {
    user_id: u64,
    email: String,
    service: String,
}

impl Event for UserRegistered {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone)]
struct ServiceHealthCheck {
    service_name: String,
    status: String,
    timestamp: u64,
}

impl Event for ServiceHealthCheck {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct MicroservicesBus {
    dispatcher: EventDispatcher,
    event_counter: Arc<AtomicU64>,
}

impl MicroservicesBus {
    fn new() -> Self {
        let dispatcher = EventDispatcher::new();
        let event_counter = Arc::new(AtomicU64::new(0));
        
        // Service orchestration
        dispatcher.subscribe_with_priority(|event: &UserRegistered| {
            // User service acknowledges
            println!("👤 User Service: User {} registered", event.user_id);
            Ok(())
        }, Priority::Critical);
        
        dispatcher.on(|event: &UserRegistered| {
            // Email service
            println!("📧 Email Service: Sending welcome email to {}", event.email);
        });
        
        dispatcher.on(|event: &UserRegistered| {
            // Analytics service
            println!("📊 Analytics Service: Recording user registration");
        });
        
        dispatcher.on(|event: &UserRegistered| {
            // Notification service
            println!("🔔 Notification Service: User {} joined from {}", event.user_id, event.service);
        });
        
        // Health check monitoring
        dispatcher.on(|event: &ServiceHealthCheck| {
            match event.status.as_str() {
                "healthy" => println!("✅ {} is healthy", event.service_name),
                "degraded" => println!("⚠️  {} is degraded", event.service_name),
                "down" => println!("❌ {} is down!", event.service_name),
                _ => println!("❓ {} status unknown", event.service_name),
            }
        });
        
        // Event counting middleware
        let counter = event_counter.clone();
        dispatcher.add_middleware(move |event: &dyn Event| {
            counter.fetch_add(1, Ordering::Relaxed);
            println!("📈 Event #{}: {}", counter.load(Ordering::Relaxed), event.event_name());
            true
        });
        
        Self {
            dispatcher,
            event_counter,
        }
    }
    
    fn simulate_microservices(&self) {
        println!("🚀 Starting microservices simulation...\n");
        
        // Simulate user registrations
        for i in 1..=5 {
            self.dispatcher.emit(UserRegistered {
                user_id: i,
                email: format!("user{}@example.com", i),
                service: "web".to_string(),
            });
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        
        println!("\n🏥 Running health checks...\n");
        
        // Simulate health checks
        let services = vec![
            ("user-service", "healthy"),
            ("email-service", "healthy"),
            ("analytics-service", "degraded"),
            ("notification-service", "down"),
        ];
        
        for (service, status) in services {
            self.dispatcher.emit(ServiceHealthCheck {
                service_name: service.to_string(),
                status: status.to_string(),
                timestamp: now(),
            });
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        
        println!("\n📊 Total events processed: {}", self.event_counter.load(Ordering::Relaxed));
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn main() {
    let bus = MicroservicesBus::new();
    bus.simulate_microservices();
}
```

### Async Web Server Events

```rust
#[cfg(feature = "async")]
mod async_web_example {
    use mod_events::prelude::*;
    use std::sync::Arc;
    use tokio::time::{sleep, Duration};

    #[derive(Debug, Clone)]
    struct HttpRequest {
        method: String,
        path: String,
        user_id: Option<u64>,
        timestamp: u64,
    }

    impl Event for HttpRequest {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[derive(Debug, Clone)]
    struct DatabaseQuery {
        query: String,
        duration_ms: u64,
    }

    impl Event for DatabaseQuery {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    struct AsyncWebServer {
        dispatcher: EventDispatcher,
    }

    impl AsyncWebServer {
        fn new() -> Self {
            let dispatcher = EventDispatcher::new();
            
            // High priority - authentication
            dispatcher.subscribe_async_with_priority(|event: &HttpRequest| async {
                if event.user_id.is_some() {
                    println!("🔐 Auth: User {} authenticated", event.user_id.unwrap());
                } else {
                    println!("🔐 Auth: Anonymous request");
                }
                Ok(())
            }, Priority::High);
            
            // Normal priority - request logging
            dispatcher.subscribe_async(|event: &HttpRequest| async {
                println!("📝 Logger: {} {} - User: {:?}", 
                    event.method, event.path, event.user_id);
                sleep(Duration::from_millis(10)).await; // Simulate async I/O
                Ok(())
            });
            
            // Normal priority - analytics
            dispatcher.subscribe_async(|event: &HttpRequest| async {
                println!("📊 Analytics: Recording {} request to {}", 
                    event.method, event.path);
                sleep(Duration::from_millis(5)).await; // Simulate async I/O
                Ok(())
            });
            
            // Database query monitoring
            dispatcher.subscribe_async(|event: &DatabaseQuery| async {
                if event.duration_ms > 1000 {
                    println!("⚠️  Slow query detected: {} ({}ms)", 
                        event.query, event.duration_ms);
                } else {
                    println!("✅ Query completed: {} ({}ms)", 
                        event.query, event.duration_ms);
                }
                Ok(())
            });
            
            Self { dispatcher }
        }
        
        async fn handle_request(&self, method: &str, path: &str, user_id: Option<u64>) {
            let request = HttpRequest {
                method: method.to_string(),
                path: path.to_string(),
                user_id,
                timestamp: now(),
            };
            
            // Dispatch async event
            let result = self.dispatcher.dispatch_async(request).await;
            
            if result.all_succeeded() {
                println!("✅ Request processed successfully");
            } else {
                println!("❌ Some handlers failed: {} errors", result.error_count());
            }
            
            // Simulate database query
            if path.contains("users") {
                sleep(Duration::from_millis(100)).await; // Simulate processing
                
                self.dispatcher.emit(DatabaseQuery {
                    query: format!("SELECT * FROM users WHERE id = {:?}", user_id),
                    duration_ms: 150,
                });
            }
        }
        
        async fn simulate_web_traffic(&self) {
            println!("🌐 Starting web server simulation...\n");
            
            let requests = vec![
                ("GET", "/", None),
                ("POST", "/login", Some(1)),
                ("GET", "/users/profile", Some(1)),
                ("POST", "/users/update", Some(1)),
                ("GET", "/api/data", Some(2)),
                ("DELETE", "/users/1", Some(1)),
            ];
            
            for (method, path, user_id) in requests {
                println!("\n🔄 Processing {} {}", method, path);
                self.handle_request(method, path, user_id).await;
                sleep(Duration::from_millis(200)).await;
            }
        }
    }

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    #[tokio::main]
    async fn main() {
        let server = AsyncWebServer::new();
        server.simulate_web_traffic().await;
    }
}

#[cfg(feature = "async")]
fn main() {
    async_web_example::main();
}

#[cfg(not(feature = "async"))]
fn main() {
    println!("This example requires the 'async' feature.");
    println!("Run with: cargo run --features async --example async_web_server");
}
```

### Performance Testing Example

```rust
use mod_events::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone)]
struct BenchmarkEvent {
    id: u64,
    payload: Vec<u8>,
}

impl Event for BenchmarkEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct PerformanceTester {
    dispatcher: EventDispatcher,
    counter: Arc<AtomicU64>,
}

impl PerformanceTester {
    fn new() -> Self {
        let dispatcher = EventDispatcher::new();
        let counter = Arc::new(AtomicU64::new(0));
        
        // Add multiple listeners to test scaling
        for i in 0..10 {
            let counter = counter.clone();
            dispatcher.on(move |event: &BenchmarkEvent| {
                counter.fetch_add(1, Ordering::Relaxed);
                // Simulate some work
                let _result = event.id * (i + 1) as u64;
            });
        }
        
        Self { dispatcher, counter }
    }
    
    fn run_benchmark(&self, event_count: usize) {
        println!("🚀 Running performance benchmark with {} events", event_count);
        
        let start = Instant::now();
        
        for i in 0..event_count {
            self.dispatcher.emit(BenchmarkEvent {
                id: i as u64,
                payload: vec![0u8; 100], // 100 bytes payload
            });
        }
        
        let duration = start.elapsed();
        let events_per_second = event_count as f64 / duration.as_secs_f64();
        let handler_calls = self.counter.load(Ordering::Relaxed);
        
        println!("📊 Performance Results:");
        println!("  Events: {}", event_count);
        println!("  Duration: {:?}", duration);
        println!("  Events/sec: {:.0}", events_per_second);
        println!("  Avg time per event: {:?}", duration / event_count as u32);
        println!("  Handler calls: {}", handler_calls);
        println!("  Handlers/sec: {:.0}", handler_calls as f64 / duration.as_secs_f64());
    }
}

fn main() {
    let tester = PerformanceTester::new();
    
    // Run multiple benchmarks
    for &count in &[1_000, 10_000, 100_000] {
        tester.run_benchmark(count);
        println!();
    }
}
```

## Usage Patterns

### Pattern 1: Service Orchestration

```rust
// Define service events
#[derive(Debug, Clone)]
struct ServiceEvent {
    service: String,
    action: String,
    data: serde_json::Value,
}

impl Event for ServiceEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// Orchestrate services
fn setup_service_orchestration(dispatcher: &EventDispatcher) {
    // Critical path
    dispatcher.subscribe_with_priority(|event: &ServiceEvent| {
        if event.service == "payment" {
            process_payment(&event.data)?;
        }
        Ok(())
    }, Priority::Critical);
    
    // Secondary processes
    dispatcher.on(|event: &ServiceEvent| {
        if event.service == "email" {
            send_notification(&event.data);
        }
    });
}
```

### Pattern 2: Event Sourcing

```rust
#[derive(Debug, Clone)]
struct DomainEvent {
    aggregate_id: String,
    event_type: String,
    version: u64,
    data: serde_json::Value,
}

impl Event for DomainEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn setup_event_sourcing(dispatcher: &EventDispatcher) {
    // Persist events
    dispatcher.subscribe_with_priority(|event: &DomainEvent| {
        store_event_in_database(event)?;
        Ok(())
    }, Priority::Highest);
    
    // Update read models
    dispatcher.on(|event: &DomainEvent| {
        update_read_models(event);
    });
    
    // Trigger side effects
    dispatcher.on(|event: &DomainEvent| {
        trigger_side_effects(event);
    });
}
```

### Pattern 3: CQRS Implementation

```rust
#[derive(Debug, Clone)]
struct Command {
    id: String,
    command_type: String,
    payload: serde_json::Value,
}

impl Event for Command {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone)]
struct Query {
    id: String,
    query_type: String,
    parameters: serde_json::Value,
}

impl Event for Query {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn setup_cqrs(dispatcher: &EventDispatcher) {
    // Command handlers
    dispatcher.subscribe_with_priority(|command: &Command| {
        match command.command_type.as_str() {
            "create_user" => handle_create_user(&command.payload)?,
            "update_user" => handle_update_user(&command.payload)?,
            _ => return Err("Unknown command".into()),
        }
        Ok(())
    }, Priority::High);
    
    // Query handlers
    dispatcher.subscribe_with_priority(|query: &Query| {
        match query.query_type.as_str() {
            "get_user" => handle_get_user(&query.parameters)?,
            "list_users" => handle_list_users(&query.parameters)?,
            _ => return Err("Unknown query".into()),
        }
        Ok(())
    }, Priority::Normal);
}
```

## Helper Functions

```rust
// Common helper functions used in examples
fn process_payment(data: &serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("💳 Processing payment: {:?}", data);
    Ok(())
}

fn send_notification(data: &serde_json::Value) {
    println!("📬 Sending notification: {:?}", data);
}

fn store_event_in_database(event: &DomainEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("💾 Storing event: {} v{}", event.event_type, event.version);
    Ok(())
}

fn update_read_models(event: &DomainEvent) {
    println!("🔄 Updating read models for: {}", event.event_type);
}

fn trigger_side_effects(event: &DomainEvent) {
    println!("⚡ Triggering side effects for: {}", event.event_type);
}

fn handle_create_user(payload: &serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("👤 Creating user: {:?}", payload);
    Ok(())
}

fn handle_update_user(payload: &serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("📝 Updating user: {:?}", payload);
    Ok(())
}

fn handle_get_user(params: &serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🔍 Getting user: {:?}", params);
    Ok(())
}

fn handle_list_users(params: &serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("📋 Listing users: {:?}", params);
    Ok(())
}
```

## Running the Examples

```bash
# Basic examples
cargo run --example basic_logging
cargo run --example ecommerce_order

# Advanced examples
cargo run --example game_events
cargo run --example microservices_bus

# Async examples (requires async feature)
cargo run --features async --example async_web_server

# Performance examples
cargo run --release --example performance_test
```

## Tips for Your Own Implementation

1. **Keep events simple** - Focus on the essential data
2. **Use descriptive names** - Make intent clear
3. **Handle errors gracefully** - Don't let one handler break others
4. **Consider priorities** - Critical operations should run first
5. **Add middleware for cross-cutting concerns** - Logging, metrics, etc.
6. **Test your event handlers** - They're just functions!
7. **Monitor performance** - Use the built-in metrics
8. **Scale horizontally** - Multiple dispatchers for different domains


<br>

## Next Steps

- Get Started [Quick Start Guide](quick-start.md)
- Read the [API Reference](api-reference.md)
- Learn [Best Practices](best-practices.md)
- Review [Performance Guide](performance.md)
- See the [Migration Guide](migration.md)