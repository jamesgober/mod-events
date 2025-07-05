//! Async usage example for mod-events

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
    println!("Mod Events - Async Usage Example");

    let dispatcher = EventDispatcher::new();

    // Async email sender - clone the data to avoid lifetime issues
    dispatcher.subscribe_async(|event: &EmailEvent| {
        let to = event.to.clone();
        let subject = event.subject.clone();
        let body = event.body.clone();
        async move {
            println!("📧 Sending email to: {to}");
            println!("    Subject: {subject}");
            println!("    Body: {body}");

            // Simulate async email sending
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            println!("✅ Email sent successfully!");
            Ok(())
        }
    });

    // High priority async logger
    dispatcher.subscribe_async_with_priority(
        |event: &EmailEvent| {
            let to = event.to.clone(); // Clone to avoid lifetime issues
            async move {
                println!("📝 Logging email event for: {to}");

                // Simulate async logging
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                println!("✅ Email event logged!");
                Ok(())
            }
        },
        Priority::High,
    );

    // Dispatch async events
    println!("\n--- Dispatching Async Events ---");

    let result = dispatcher
        .dispatch_async(EmailEvent {
            to: "user@example.com".to_string(),
            subject: "Welcome!".to_string(),
            body: "Welcome to our service!".to_string(),
        })
        .await;

    if result.all_succeeded() {
        println!("✅ All async handlers completed successfully!");
    }

    println!("Event handled by {} listeners", result.success_count());
}

#[cfg(not(feature = "async"))]
fn main() {
    println!("This example requires the 'async' feature to be enabled.");
    println!("Run with: cargo run --features async --example async_usage");
}
