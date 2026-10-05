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

<div align="center">

# 🔭 Overwatch

**A lightweight framework for building modular, interconnected applications in Rust.**

[![MIT License][mit-badge]][mit-url]
[![Apache License][apache-badge]][apache-url]
[![Build Status][actions-badge]][actions-url]
[![Codecov Status][codecov-badge]][codecov-url]
[![crates.io][crates-badge]][crates-url]
[![docs.rs][docs-badge]][docs-url]

[Getting Started](#-getting-started) •
[Architecture](#-architecture) •
[Examples](#-examples) •
[Documentation](#-documentation)

</div>

---

## 🎯 What is Overwatch?

Overwatch simplifies the development of complex systems by enabling **seamless communication between independent components**. Think of it as a lightweight alternative to microservices that runs within a single process.

### Why Overwatch?

| Traditional Approach | With Overwatch |
|---------------------|----------------|
| Tightly coupled components | 🔌 Modular, independent services |
| Complex inter-process communication | 📨 Built-in async message passing |
| Manual lifecycle management | ⚡ Automatic service orchestration |
| Scattered configuration | ⚙️ Centralized settings management |
| Difficult to test | 🧪 Easy to mock and test services |

---

## 🏗️ Architecture

Overwatch uses a **mediator pattern** where the `OverwatchRunner` acts as the central coordinator for all services:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              OVERWATCH RUNNER                               │
│                           (Central Coordinator)                             │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                            MESSAGE RELAY                            │   │
│   │                Async communication between services                 │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                 |                    │                    │                 │
│          ┌─────────────┐      ┌─────────────┐      ┌─────────────┐          │
│          │  Service A  │ <--> │  Service B  │ <--> │  Service C  │          │
│          │             │      │             │      │             │          │
│          │ ┌─────────┐ │      │ ┌─────────┐ │      │ ┌─────────┐ │          │
│          │ │Settings │ │      │ │Settings │ │      │ │Settings │ │          │
│          │ └─────────┘ │      │ └─────────┘ │      │ └─────────┘ │          │
│          │ ┌─────────┐ │      │ ┌─────────┐ │      │ ┌─────────┐ │          │
│          │ │  State  │ │      │ │  State  │ │      │ │  State  │ │          │
│          │ └─────────┘ │      │ └─────────┘ │      │ └─────────┘ │          │
│          └─────────────┘      └─────────────┘      └─────────────┘          │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                      LIFECYCLE MANAGEMENT                           │   │
│   │         Start • Stop • Restart • Configuration Updates              │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Concepts

| Concept | Description |
|---------|-------------|
| **OverwatchRunner** | The central coordinator that manages all services |
| **Service** | An independent unit of work with its own lifecycle |
| **Relay** | Type-safe async channel for inter-service communication |
| **Settings** | Configuration for each service |
| **State** | Persistent state that survives restarts |
| **StateOperator** | Logic for loading/saving service state |

---

## 🚀 Getting Started

### Requirements

- **Rust ≥ 1.63**

### Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
overwatch = "1"
overwatch-derive = "1"
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
```

### Minimal Example

Here's the simplest possible Overwatch application:

```rust
use async_trait::async_trait;
use overwatch::{
    derive_services,
    overwatch::{OverwatchRunner, ShutdownOverwatch},
    services::{
        ServiceCore, ServiceData,
        state::{NoOperator, NoState},
    },
    DynError, OpaqueServiceResourcesHandle,
};

// 1️⃣ Define your service
struct HelloService {
    handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
}

// 2️⃣ Specify service data types
impl ServiceData for HelloService {
    type Settings = ();                             // No configuration needed
    type State = NoState<Self::Settings>;           // No persistent state
    type StateOperator = NoOperator<Self::State>;   // No state operations
    type Message = ();                              // No incoming messages
}

// 3️⃣ Implement the service logic
#[async_trait]
impl ServiceCore<RuntimeServiceId> for HelloService {
    fn init(
        handle: OpaqueServiceResourcesHandle<Self, RuntimeServiceId>,
        _state: Self::State,
    ) -> Result<Self, DynError> {
        Ok(Self { handle })
    }

    async fn run(self) -> Result<(), DynError> {
        println!("👋 Hello from Overwatch!");
        
        // Signal that this service is done. We can shut down the whole application.
        self.handle
            .overwatch_handle
            .shutdown()
            .await;
        
        Ok(())
    }
}

// 4️⃣ Compose your application
#[derive_services(panic_policy = ShutdownOverwatch)]
struct MyApp {
    hello: HelloService,
}

// 5️⃣ Run it!
fn main() {
    let settings = MyAppServiceSettings { hello: () };
    
    let app = OverwatchRunner::<MyApp>::run(settings, None)
        .expect("Failed to start");
    
    // Start all services
    app.runtime()
        .handle()
        .block_on(app.handle().start_all_services())
        .expect("Failed to start services");
    
    app.blocking_wait_finished()
        .expect("A service panicked");
}
```

---

## 📬 Inter-Service Communication

Services communicate through **typed message relays**:

```
┌──────────────┐         PongMessage          ┌──────────────┐
│              │ ───────────────────────────> │              │
│ Ping Service │                              │ Pong Service │
│              │ <─────────────────────────── │              │
└──────────────┘         PingMessage          └──────────────┘
```

```rust
// Define message types
#[derive(Debug)]
enum PingMessage { Pong }

#[derive(Debug)]  
enum PongMessage { Ping }

// In PingService::run()
async fn run(self) -> Result<(), DynError> {
    // Get a relay to send messages to PongService
    let pong_relay = self.handle
        .overwatch_handle
        .relay::<PongService>()
        .await?;
    
    // Send a message
    pong_relay.send(PongMessage::Ping).await?;
    
    // Receive messages
    while let Some(msg) = self.handle.inbound_relay.recv().await {
        match msg {
            PingMessage::Pong => println!("Received Pong!"),
        }
    }
    Ok(())
}
```

---

## 📦 Examples

### Ping-Pong Example

The [`examples/ping_pong`](examples/ping_pong) directory contains a complete working example demonstrating:

- ✅ Service definition and registration
- ✅ Inter-service messaging via relays
- ✅ Settings configuration
- ✅ State persistence and restoration
- ✅ Custom state operators

**Run it:**

```bash
cargo run --example ping_pong
```

**What it does:**

1. **Ping** sends a message to **Pong** every second
2. **Pong** receives it and replies back
3. **Ping** tracks the count and persists it to disk
4. After 30 pongs, the application exits

---

## 📖 Documentation

| Resource | Description |
|----------|-------------|
| [API Docs](https://docs.rs/overwatch) | Full API reference |
| [Examples](examples/) | Working code examples |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution guidelines |

---

## 🧩 Project Structure

```
Overwatch/
├── overwatch/          # Core framework library
│   └── src/
│       ├── overwatch/  # Runner, handle, commands
│       ├── services/   # Service traits and utilities
│       └── utils/      # Helper utilities
├── overwatch-derive/   # Procedural macros (#[derive_services])
└── examples/
    └── ping_pong/      # Complete working example
```

---

## 🔧 Development

### Running Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture
```

### Running Examples

```bash
cargo run --example ping_pong
```

### Generating Documentation

```bash
cargo doc --open --no-deps
```

---

## 🤝 Contributing

We welcome contributions! Please read our [Contributing Guidelines](CONTRIBUTING.md) for details.

---

## 📄 License

Dual-licensed under [Apache 2.0](LICENSE-APACHE2.0) and [MIT](LICENSE-MIT).

---

## 💬 Community

Join the conversation:

- 💬 [Discord Server](https://discord.gg/G6q8FgZq)
- 🐛 [GitHub Issues](https://github.com/logos-co/Overwatch/issues)

---