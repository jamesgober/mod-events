//! Basic usage example for mod-events

use mod_events::prelude::*;

// Define your events
#[derive(Debug, Clone)]
struct UserRegistered {
    user_id: u64,
    email: String,
    _timestamp: u64,
}

impl Event for UserRegistered {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone)]
struct OrderPlaced {
    order_id: u64,
    _user_id: u64,
    amount: f64,
    items: Vec<String>,
}

impl Event for OrderPlaced {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn main() {
    println!("Mod Events - Basic Usage Example");

    let dispatcher = EventDispatcher::new();

    // Subscribe to user registration events
    dispatcher.on(|event: &UserRegistered| {
        println!("📧 Welcome email queued for {}", event.email);
    });

    dispatcher.subscribe_with_priority(
        |event: &UserRegistered| {
            println!("🔔 High priority: New user {} registered", event.user_id);
            Ok(())
        },
        Priority::High,
    );

    // Subscribe to order events
    dispatcher.on(|event: &OrderPlaced| {
        println!(
            "📦 Processing order {} for ${:.2}",
            event.order_id, event.amount
        );
        println!("    Items: {:?}", event.items);
    });

    // Add middleware for logging
    dispatcher.add_middleware(|event: &dyn Event| {
        println!("🔍 Event: {}", event.event_name());
        true // Allow all events
    });

    // Dispatch some events
    println!("\n--- Dispatching Events ---");

    dispatcher.emit(UserRegistered {
        user_id: 123,
        email: "alice@example.com".to_string(),
        _timestamp: 1634567890,
    });

    dispatcher.emit(OrderPlaced {
        order_id: 456,
        _user_id: 123,
        amount: 99.99,
        items: vec!["Laptop".to_string(), "Mouse".to_string()],
    });

    // Show metrics
    println!("\n--- Metrics ---");
    let metrics = dispatcher.metrics();
    for (_, meta) in metrics {
        println!(
            "Event: {} - Dispatched: {} times",
            meta.event_name, meta.dispatch_count
        );
    }

    println!("\n--- Listener Counts ---");
    println!(
        "UserRegistered: {}",
        dispatcher.listener_count::<UserRegistered>()
    );
    println!(
        "OrderPlaced: {}",
        dispatcher.listener_count::<OrderPlaced>()
    );
}
