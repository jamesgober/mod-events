//! Async usage example for mod-events.

#[cfg(feature = "async")]
use mod_events::prelude::*;

#[cfg(feature = "async")]
#[derive(Debug, Clone)]
struct EmailEvent {
    to: String,
    subject: String,
    body: String,
}

#[cfg(feature = "async")]
impl Event for EmailEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(feature = "async")]
#[tokio::main]
async fn main() {
    println!("mod-events :: async usage example");

    let dispatcher = EventDispatcher::new();

    // Async email sender — clone borrowed data so the future is `'static`.
    let _sender_id = dispatcher.subscribe_async(|event: &EmailEvent| {
        let to = event.to.clone();
        let subject = event.subject.clone();
        let body = event.body.clone();
        async move {
            println!("sending email to: {to}");
            println!("    subject: {subject}");
            println!("    body: {body}");

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            println!("email sent");
            Ok(())
        }
    });

    // High-priority async logger.
    let _logger_id = dispatcher.subscribe_async_with_priority(
        |event: &EmailEvent| {
            let to = event.to.clone();
            async move {
                println!("logging email event for: {to}");
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                println!("email event logged");
                Ok(())
            }
        },
        Priority::High,
    );

    println!("\n--- dispatching async events ---");

    let result = dispatcher
        .dispatch_async(EmailEvent {
            to: "user@example.com".to_string(),
            subject: "Welcome!".to_string(),
            body: "Welcome to our service!".to_string(),
        })
        .await;

    if result.all_succeeded() {
        println!("all async handlers completed successfully");
    }

    println!("event handled by {} listeners", result.success_count());
}

#[cfg(not(feature = "async"))]
fn main() {
    println!("this example requires the `async` feature to be enabled.");
    println!("run with: cargo run --features async --example async_usage");
}
