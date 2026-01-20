[apache-badge]: https://img.shields.io/badge/License-Apache%202.0-blue?style=for-the-badge

[apache-url]: https://github.com/logos-co/Overwatch/blob/main/LICENSE-APACHE2.0

[mit-badge]: https://img.shields.io/badge/License-MIT-blue?style=for-the-badge

[mit-url]: https://github.com/logos-co/Overwatch/blob/main/LICENSE-MIT]

[actions-badge]: https://img.shields.io/github/actions/workflow/status/logos-co/Overwatch/main.yml?style=for-the-badge&logo=github

[actions-url]: https://github.com/logos-co/Overwatch/actions/workflows/main.yml?query=workflow%3ACI+branch%3Amain

[codecov-badge]: https://img.shields.io/codecov/c/github/logos-co/Overwatch?style=for-the-badge&logo=codecov

[codecov-url]: https://codecov.io/github/logos-co/Overwatch

[crates-badge]: https://img.shields.io/crates/v/overwatch-derive.svg?style=for-the-badge&color=fc8d62&logo=rust

[crates-url]: https://crates.io/crates/overwatch-derive

[docs-badge]: https://img.shields.io/docsrs/overwatch-derive?style=for-the-badge&logo=docs.rs

[docs-url]: https://docs.rs/overwatch-derive

# Overwatch Derive

[![MIT License][mit-badge]][mit-url]
[![Apache License][apache-badge]][apache-url]
[![Build Status][actions-badge]][actions-url]
[![Codecov Status][codecov-badge]][codecov-url]
[![crates.io][crates-badge]][crates-url]
[![docs.rs][docs-badge]][docs-url]

**Procedural macros for the Overwatch framework.**

This crate provides derive macros that reduce boilerplate when building Overwatch applications.

---

## 📦 Installation

```toml
[dependencies]
overwatch = "1"
overwatch-derive = "1"
```

---

## 🔧 Available Macros

### `#[derive_services]`

The main macro that transforms a struct into a complete Overwatch application.

#### Before (Manual Implementation)

```rust
// You would need to manually implement:
// - Services trait
// - Handle management for each service
// - Settings aggregation
// - Service registration
// ... hundreds of lines of boilerplate
```

#### After (With Macro)

```rust
use overwatch::derive_services;

#[derive_services]
struct MyApp {
    service_a: ServiceA,
    service_b: ServiceB,
    service_c: ServiceC,
}

// That's it! The macro generates everything you need.
```

---

## ✨ What Gets Generated

When you use `#[derive_services]`, the macro automatically generates:

### 1. Settings Struct

```rust
// Generated: MyAppServiceSettings
let settings = MyAppServiceSettings {
    service_a: ServiceASettings { /* ... */ },
    service_b: ServiceBSettings { /* ... */ },
    service_c: ServiceCSettings { /* ... */ },
};
```

### 2. Services Implementation

The `Services` trait is implemented, enabling:

- Service registration with the runner
- Lifecycle management
- Relay (message channel) creation

### 3. `RuntimeServiceId` Enum

```rust
// Generated: RuntimeServiceId
enum RuntimeServiceId {
    ServiceA,
    ServiceB,
    ServiceC,
}
```

This ensures that two services cannot communicate with each other if they are not part of the same Overwatch runtime.

---

## 📝 Complete Example

```rust
use async_trait::async_trait;
use overwatch::{
    derive_services,
    overwatch::OverwatchRunner,
    services::{
        ServiceCore, ServiceData,
        state::{NoOperator, NoState},
    },
    DynError, OpaqueServiceResourcesHandle,
};

// Define your services
struct Logger { /* ... */ }
struct Database { /* ... */ }
struct ApiServer { /* ... */ }

// Implement ServiceData and ServiceCore for each...

// Compose your application
#[derive_services]
struct MyApp {
    logger: Logger,
    database: Database,
    api: ApiServer,
}

fn main() {
    // Use the generated settings struct
    let settings = MyAppServiceSettings {
        logger: LoggerSettings { level: "info".into() },
        database: DatabaseSettings { url: "...".into() },
        api: ApiSettings { port: 8080 },
    };
    
    // Run the application
    let app = OverwatchRunner::<MyApp>::run(settings, None)
        .expect("Failed to start");
    
    app.runtime()
        .handle()
        .block_on(app.handle().start_all_services())
        .expect("Failed to start services");
    
    app.blocking_wait_finished();
}
```

---

## 📖 More Information

For complete documentation and examples, see the [main README](https://github.com/logos-co/Overwatch/blob/main/README.md).

---

## 📄 License

Dual-licensed under [Apache 2.0](../LICENSE-APACHE2.0) and [MIT](../LICENSE-MIT).