# mod-events vs. the alternatives

This page describes when `mod-events` is the right choice and when it
is not. It does not claim specific multipliers ("Nx faster than X")
because such claims need a reproducible benchmark methodology that
fairly stresses every candidate. The right answer to "is mod-events
faster than X" is almost always "for which workload?" — see the
[performance guide](performance.md) for the numbers we *have*
measured against the crate itself.

## At a glance

| Need | Use |
|------|-----|
| In-process pub/sub with priority + middleware | **mod-events** |
| In-process pub/sub, no priority, no middleware, runtime-locked to tokio | `tokio::sync::broadcast` |
| Lightweight wake-up signal across futures, no payload | `event-listener` |
| Sync-only broadcast bus, no async | `bus` |
| MPMC channel between specific senders + receivers | `crossbeam-channel` |
| Cross-process pub/sub | Redis Pub/Sub, NATS, Kafka — not mod-events |
| Job queue with retries / scheduling | A real job queue — not mod-events |
| Stateful per-listener queueing under back-pressure | Channels + worker tasks — not mod-events |

## Vs. `tokio::sync::broadcast`

`tokio::sync::broadcast` is a multi-producer, multi-consumer broadcast
channel. It is excellent at what it does and the right tool for
high-throughput pub/sub when consumers are tokio tasks.

What `mod-events` adds:
- **Type safety per event.** Every dispatch goes to listeners of that
  type. No runtime type tag, no enum dispatch. The compiler enforces
  routing.
- **Priority + FIFO.** Listeners run in descending priority, FIFO
  within equal priority. `broadcast` is unordered.
- **Middleware.** A middleware chain can short-circuit a dispatch.
  `broadcast` has no notion of filtering before fan-out.
- **Sync support.** `dispatch` is a synchronous call. `broadcast`
  requires an async context.
- **Runtime-agnostic.** Works with any executor that polls a
  `Future`. `broadcast` requires tokio.

What `tokio::sync::broadcast` adds:
- **Bounded queueing with back-pressure.** Slow consumers fall
  behind; lagging is observable. `mod-events` invokes listeners
  synchronously — there is no queue.
- **Lock-free under heavy concurrent send.** Optimised for the
  one-producer-many-consumer pattern.

Choose `tokio::sync::broadcast` if your consumers are independent
async tasks that need to handle back-pressure and you are already on
tokio. Choose `mod-events` if you need typed events, priority,
middleware, sync dispatch, or runtime independence.

## Vs. `event-listener`

`event-listener` is a tiny primitive for "wait until something
happens." It carries no payload — just the wake-up signal.

What `mod-events` adds:
- **Typed payload per event.** Listeners receive the event by reference.
- **Multiple listeners per event with priority.**
- **Middleware, metrics, error reporting.**

What `event-listener` adds:
- **Almost nothing.** It is ~200 lines of code with no dependencies.
  If a one-bit notification is all you need, `event-listener` is the
  right answer.

Choose `event-listener` when the answer to "what data does the
listener need?" is "nothing." Choose `mod-events` when listeners need
the event payload.

## Vs. `bus`

`bus` is a sync-only broadcast bus with bounded buffering. Producers
push to the bus; consumers receive via a clone of the receiver
handle.

What `mod-events` adds:
- **Async support.**
- **Listener priority and middleware.**
- **Type-erased multi-event dispatcher** — one dispatcher routes many
  event types. `bus` is one bus per event type.

What `bus` adds:
- **Bounded buffering.** Slow consumers fall behind; producers can
  block or drop based on configuration.
- **No locks on the producer side** for the common path.

Choose `bus` for a sync-only, bounded, single-event-type fan-out.
Choose `mod-events` for typed multi-event dispatch with priority and
async.

## Vs. `crossbeam-channel`

`crossbeam-channel` is a high-performance MPMC channel. It is the
state of the art for sync message passing between known senders and
known receivers.

`crossbeam-channel` and `mod-events` solve different problems.
A channel is point-to-point (or point-to-pool); a dispatcher is
broadcast (one event reaches every interested listener). If you
explicitly enumerate the producers and consumers, use a channel. If
you have an event that any number of unrelated subsystems should
react to without the producer knowing about the consumers, use a
dispatcher.

Many real architectures use both: `mod-events` for the pub/sub
boundary, `crossbeam-channel` (or `tokio::sync::mpsc`) for the
inside of each consumer subsystem.

## Vs. plain function calls

For very small applications, the question is "do I really need a
dispatcher at all?" If your producer knows exactly which functions
to call when an event happens, calling them directly is one
indirection cheaper and one dependency lighter. The dispatcher is
worth the cost when:

- Multiple subsystems react to the same event and you do not want
  the producer to know which subsystems exist (decoupling).
- Listeners need to be added or removed at runtime (plugin /
  extension architectures).
- You want a single place to add cross-cutting behavior (logging,
  metrics, filtering) via middleware.
- You want type-safe multi-event routing without writing the
  dispatch table by hand.

## Vs. building it yourself with `Vec<Box<dyn Fn>>`

Many Rust projects start with a hand-rolled
`Vec<Box<dyn Fn(&Event)>>` and grow until they reinvent most of
`mod-events`. The tipping point is usually one of:

- The first time you need *priority ordering* (you sort the Vec on
  every dispatch).
- The first time a listener panics and you realise you have not
  caught it.
- The first time you want metrics and you wire them through every
  dispatch site by hand.
- The first time you need async listeners and the manual `Vec` design
  does not extend cleanly.
- The first time multiple threads dispatch the same event type and
  you need a real lock primitive.

`mod-events` handles each of those once, well, with REPS-grade
testing and supply-chain auditing. If your hand-rolled version does
all of these, you are maintaining `mod-events` — just with worse
tests.

## What `mod-events` deliberately does *not* do

- **Bounded queueing.** Listeners run synchronously inside the
  dispatch call. There is no queue between producer and consumer; if
  you need one, put a channel inside the listener.
- **Retries.** A failing listener returns its error in
  `DispatchResult`. The dispatcher does not call it again.
- **Cross-process delivery.** Use Redis, NATS, Kafka, etc.
- **Schema evolution.** Events are typed Rust values; rename or add
  fields like any other Rust type and recompile.
- **Cancellation.** Dropping the dispatching task cancels the await
  chain in standard Rust async fashion. There is no separate
  cancellation-token API. Roadmap may add this if demand emerges.
- **Per-listener timeouts.** Wrap your listener body with your
  runtime's timeout primitive
  (`tokio::time::timeout(d, do_work())`). Built-in support would
  lock us to one runtime; we stay runtime-agnostic.

## Where to look next

- [Performance guide](performance.md) — measured numbers for
  `mod-events` itself.
- [Architecture](architecture.md) — design rationale for every
  primitive choice.
- [Best practices](best-practices.md) — patterns that fit the
  dispatcher's contracts.
