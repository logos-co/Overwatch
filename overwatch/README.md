[apache-badge]: https://img.shields.io/badge/License-Apache%202.0-blue?style=for-the-badge

[apache-url]: https://github.com/logos-co/Overwatch/blob/main/LICENSE-APACHE2.0

[mit-badge]: https://img.shields.io/badge/License-MIT-blue?style=for-the-badge

[mit-url]: https://github.com/logos-co/Overwatch/blob/main/LICENSE-MIT]

[actions-badge]: https://img.shields.io/github/actions/workflow/status/logos-co/Overwatch/main.yml?style=for-the-badge&logo=github

[actions-url]: https://github.com/logos-co/Overwatch/actions/workflows/main.yml?query=workflow%3ACI+branch%3Amain

[codecov-badge]: https://img.shields.io/codecov/c/github/logos-co/Overwatch?style=for-the-badge&logo=codecov

[codecov-url]: https://codecov.io/github/logos-co/Overwatch

[crates-badge]: https://img.shields.io/crates/v/overwatch.svg?style=for-the-badge&color=fc8d62&logo=rust

[crates-url]: https://crates.io/crates/overwatch

[docs-badge]: https://img.shields.io/docsrs/overwatch?style=for-the-badge&logo=docs.rs

[docs-url]: https://docs.rs/overwatch

# Overwatch (Core)

[![MIT License][mit-badge]][mit-url]
[![Apache License][apache-badge]][apache-url]
[![Build Status][actions-badge]][actions-url]
[![Codecov Status][codecov-badge]][codecov-url]
[![crates.io][crates-badge]][crates-url]
[![docs.rs][docs-badge]][docs-url]

**The core library for the Overwatch framework.**

This crate provides the fundamental building blocks for creating modular, interconnected applications.

---

## 📦 What's Inside

### Core Components

| Component | Description |
|-----------|-------------|
| `OverwatchRunner` | Bootstraps and runs your application |
| `OverwatchHandle` | Control handle for lifecycle management |
| `ServiceCore` | Trait that defines service behavior |
| `ServiceData` | Trait that defines service types |

### Service Utilities

| Utility | Description |
|---------|-------------|
| `Relay` | Type-safe async message channels |
| `ServiceState` | Trait for persistent state |
| `StateOperator` | Logic for state persistence |
| `LifecycleNotifier` | Service lifecycle events |
| `StatusWatcher` | Monitor service status |

---

## 🏗️ Architecture Overview

```
                    ┌─────────────────────────┐
                    │    OverwatchRunner      │
                    │  ─────────────────────  │
                    │  • Spawns services      │
                    │  • Manages lifecycle    │
                    │  • Routes messages      │
                    └───────────┬─────────────┘
                                │
            ┌───────────────────┼───────────────────┐
            │                   │                   │
            v                   v                   v
    ┌───────────────┐   ┌───────────────┐   ┌───────────────┐
    │ ServiceRunner │   │ ServiceRunner │   │ ServiceRunner │
    │ ───────────── │   │ ───────────── │   │ ───────────── │
    │ Your Service  │   │ Your Service  │   │ Your Service  │
    └───────────────┘   └───────────────┘   └───────────────┘
```

---

## 🚀 Quick Start

### Installation

```toml
[dependencies]
overwatch = "1"
overwatch-derive = "1"
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
```

### Creating a Service

Every service implements two traits:

#### 1. `ServiceData` - Define Types

```rust
impl ServiceData for MyService {
    type Settings = MySettings;      // Configuration
    type State = MyState;            // Persistent state
    type StateOperator = MyOperator; // State load/save logic
    type Message = MyMessage;        // Incoming message type
}
```

#### 2. `ServiceCore` - Define Behavior

```rust
#[async_trait]
impl ServiceCore<RuntimeServiceId> for MyService {
    fn init(
        handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
        initial_state: Self::State,
    ) -> Result<Self, DynError> {
        // Initialize your service
        Ok(Self { handle, state: initial_state })
    }

    async fn run(self) -> Result<(), DynError> {
        // Your service logic runs here
        loop {
            // Handle messages, do work, etc.
        }
    }
}
```

---

## 📬 Message Passing

Services communicate via **relays** - type-safe async channels:

```rust
// Get a relay to another service
let other_relay = self.handle
    .overwatch_handle
    .relay::<OtherService>()
    .await?;

// Send a message
other_relay.send(MyMessage::Hello).await?;

// Receive messages
while let Some(msg) = self.handle.inbound_relay.recv().await {
    // Handle incoming messages
}
```

---

## 💾 State Management

### No State (Stateless Services)

```rust
impl ServiceData for StatelessService {
    type State = NoState<Self::Settings>;
    type StateOperator = NoOperator<Self::State>;
    // ...
}
```

### With State (Stateful Services)

```rust
#[derive(Default, Clone, Serialize, Deserialize)]
struct MyState {
    counter: u32,
}

impl ServiceState for MyState {
    type Settings = MySettings;
    type Error = MyError;
    
    fn from_settings(settings: &Self::Settings) -> Result<Self, Self::Error> {
        Ok(Self::default())
    }
}
```

### State Operators

State operators handle persistence:

```rust
#[async_trait]
impl StateOperator for MyOperator {
    type State = MyState;
    type LoadError = std::io::Error;
    
    fn try_load(settings: &Settings) -> Result<Option<Self::State>, Self::LoadError> {
        // Load state from disk/database
    }
    
    async fn run(&mut self, state: Self::State) {
        // Save state when updated
    }
}
```

---

## ⚙️ Lifecycle Management

Control services programmatically:

```rust
let handle = app.handle();

// Start all services
handle.start_all_services().await?;

// Stop a specific service
handle.stop::<MyService>().await?;

// Shutdown everything
handle.shutdown().await;
```

---

## 📖 More Information

For complete documentation and examples, see the [main README](https://github.com/logos-co/Overwatch/blob/main/README.md).

---

## 📄 License

Dual-licensed under [Apache 2.0](LICENSE-APACHE2.0) and [MIT](LICENSE-MIT).