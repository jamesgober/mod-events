//! Basic usage example for mod-events.

use mod_events::prelude::*;

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

#[derive(Debug, Clone)]
struct OrderPlaced {
    order_id: u64,
    amount: f64,
    items: Vec<String>,
}

impl Event for OrderPlaced {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn main() {
    println!("mod-events :: basic usage example");

    let dispatcher = EventDispatcher::new();

    // Subscribe to user-registration events.
    let _welcome_id = dispatcher.on(|event: &UserRegistered| {
        println!("welcome email queued for {}", event.email);
    });

    let _high_priority_id = dispatcher.subscribe_with_priority(
        |event: &UserRegistered| {
            println!("high priority: new user {} registered", event.user_id);
            Ok(())
        },
        Priority::High,
    );

    // Subscribe to order events.
    let _order_id = dispatcher.on(|event: &OrderPlaced| {
        println!(
            "processing order {} for ${:.2}",
            event.order_id, event.amount
        );
        println!("    items: {:?}", event.items);
    });

    // Add middleware for logging.
    dispatcher.add_middleware(|event: &dyn Event| {
        println!("event: {}", event.event_name());
        true
    });

    // Dispatch some events.
    println!("\n--- dispatching events ---");

    dispatcher.emit(UserRegistered {
        user_id: 123,
        email: "alice@example.com".to_string(),
    });

    dispatcher.emit(OrderPlaced {
        order_id: 456,
        amount: 99.99,
        items: vec!["Laptop".to_string(), "Mouse".to_string()],
    });

    // Show metrics.
    println!("\n--- metrics ---");
    let metrics = dispatcher.metrics();
    for meta in metrics.values() {
        println!(
            "event: {} - dispatched: {} times",
            meta.event_name, meta.dispatch_count
        );
    }

    println!("\n--- listener counts ---");
    println!(
        "UserRegistered: {}",
        dispatcher.listener_count::<UserRegistered>()
    );
    println!(
        "OrderPlaced: {}",
        dispatcher.listener_count::<OrderPlaced>()
    );
}
