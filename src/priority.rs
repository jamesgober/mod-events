//! Priority system for event listeners

/// Priority levels for event listeners
///
/// Listeners with higher priority are executed first.
/// This allows for controlling the execution order of event handlers.
///
/// # Example
///
/// ```rust
/// use mod_events::{EventDispatcher, Priority, Event};
///
/// #[derive(Debug, Clone)]
/// struct MyEvent {
///     message: String,
/// }
///
/// impl Event for MyEvent {
///     fn as_any(&self) -> &dyn std::any::Any {
///         self
///     }
/// }
///
/// let dispatcher = EventDispatcher::new();
///
/// // This will execute first
/// dispatcher.subscribe_with_priority(|event: &MyEvent| {
///     println!("High priority handler");
///     Ok(())
/// }, Priority::High);
///
/// // This will execute second
/// dispatcher.subscribe_with_priority(|event: &MyEvent| {
///     println!("Normal priority handler");
///     Ok(())
/// }, Priority::Normal);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[non_exhaustive]
pub enum Priority {
    /// Lowest priority (0)
    Lowest = 0,
    /// Low priority (25)
    Low = 25,
    /// Normal priority (50) - default
    #[default]
    Normal = 50,
    /// High priority (75)
    High = 75,
    /// Highest priority (100)
    Highest = 100,
    /// Critical priority (125) - use sparingly
    Critical = 125,
}

impl Priority {
    /// Every priority level, highest first: `Critical`, `Highest`,
    /// `High`, `Normal`, `Low`, `Lowest`. This is the order in which
    /// listeners of different priorities run.
    pub fn all() -> &'static [Priority] {
        &[
            Priority::Critical,
            Priority::Highest,
            Priority::High,
            Priority::Normal,
            Priority::Low,
            Priority::Lowest,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_lists_every_level_highest_first() {
        let all = Priority::all();
        assert_eq!(all.len(), 6);
        assert!(all.windows(2).all(|pair| pair[0] > pair[1]));
        assert_eq!(all.first(), Some(&Priority::Critical));
        assert_eq!(all.last(), Some(&Priority::Lowest));
        assert!(all.contains(&Priority::default()));
    }
}
