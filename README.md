[mit-badge]: https://img.shields.io/github/license/logos-co/Overwatch?style=for-the-badge

[mit-url]: https://github.com/logos-co/Overwatch/blob/main/LICENSE

[actions-badge]: https://img.shields.io/github/actions/workflow/status/logos-co/Overwatch/main.yml?style=for-the-badge&logo=github

[actions-url]: https://github.com/logos-co/Overwatch/actions/workflows/main.yml?query=workflow%3ACI+branch%3Amain

[codecov-badge]: https://img.shields.io/codecov/c/github/logos-co/Overwatch?style=for-the-badge&logo=codecov

[codecov-url]: https://codecov.io/github/logos-co/Overwatch

[crates-badge]: https://img.shields.io/crates/v/overwatch.svg?style=for-the-badge&color=fc8d62&logo=rust

[crates-url]: https://crates.io/crates/overwatch

[docs-badge]: https://img.shields.io/docsrs/overwatch?style=for-the-badge&logo=docs.rs

[docs-url]: https://docs.rs/overwatch

# Overwatch

[![Build Status][actions-badge]][actions-url]
[![Codecov Status][codecov-badge]][codecov-url]

[![MIT License][mit-badge]][mit-url]
[![crates.io][crates-badge]][crates-url]
[![docs.rs][docs-badge]][docs-url]

**A lightweight framework for building modular, interconnected applications.**

Overwatch simplifies the development of complex systems by enabling seamless communication between independent
components. It combines the flexibility of microservices with the simplicity of a unified framework.

---

## Table of Contents

- [Requirements](#requirements)
- [Quick Start](#quick-start)
- [Features](#features)
- [Design Goals](#design-goals)
- [Components](#components)
- [Project Structure](#project-structure)
- [Development Workflow](#development-workflow)
    - [Running Tests](#running-tests)
    - [Running Examples](#running-examples)
    - [Generating Documentation](#generating-documentation)
- [Contributing](#contributing)
- [License](#license)
- [Community](#community)

---

## Requirements

- Rust ≥ 1.63

---

## Quick Start

Add `overwatch` and `overwatch-derive` to your `Cargo.toml`:

```toml
[dependencies]
overwatch = "1"
overwatch-derive = "1"
```

Here's a simple example to get you started:

```rust ignore
// This example is for illustration purposes only and is not meant to compile.
// There's parts of the code that are required to be implemented by the user.
// Please refer to the examples directory for working code. 

use overwatch_rs::{
    overwatch::OverwatchRunner,
    services::{ServiceCore, ServiceData},
    OpaqueServiceHandle
};
use overwatch_derive::Services;

struct MyService;

impl ServiceData for MyService {
    // Implement ServiceData
}

#[async_trait::async_trait]
impl ServiceCore for MyService {
    // Implement ServiceCore
}

#[derive(Services)]
struct MyApp {
    my_service: OpaqueServiceHandle<MyService>,
    // ... other services
}

fn main() {
    // `MyAppServiceSettings` is a struct that contains the settings for each service.
    // Generated by the `Services` derive.
    let my_app_settings = MyAppServiceSettings {
        my_service: ()
    };
    let my_app =
        OverwatchRunner::<MyApp>::run(my_app_settings, None).expect("OverwatchRunner failed");
    my_app.wait_finished();
}
```

---

## Features

- **Modular Design**: Build self-contained, reusable components with clear interfaces.
- **Asynchronous Communication**: Scalable and non-blocking communication between components.
- **Lifecycle Management**: Centralized control over component initialization, updates, and shutdown.
- **Dynamic Configuration**: Handle runtime configuration updates seamlessly.
- **Testability**: Components are designed for easy testing and mocking.

---

## Design Goals

Our architecture is built on three core principles:

### **Modularity**

- Components are self-contained with well-defined interfaces.
- Communication between components is explicit and predictable.
- Designed for easy testing and performance evaluation.

### **Single Responsibility**

- Each component focuses on a single task for easier debugging.
- Shared state is minimized to reduce complexity.

### **Observability**

- Workflows are transparent and traceable.
- Components are designed for easy testing and monitoring.
- Asynchronous communication ensures scalability and clarity.

---

## Components

### **Overwatch**

- Acts as the central messaging relay for internal communications.
- Manages the lifecycle of all services.
- Handles dynamic configuration updates.

### **Services**

- Modular units that perform specific tasks within the system.
- Operated and coordinated by *Overwatch*.

---

## Project Structure

- `overwatch`: The core framework.
- `overwatch-derive`: Macros to simplify component implementation.

---

## Development Workflow

### **Running Tests**

- Run all tests: `cargo test`.
- View test outputs: `cargo test -- --nocapture`.

### **Running Examples**

- Execute an example: `cargo run --example {example_name}`.

### **Generating Documentation**

- Build and open documentation: `cargo doc --open --no-deps`.

## Contributing

We welcome contributions! Please read our [Contributing Guidelines](CONTRIBUTING.md) for details on how to get started.

---

## License

Overwatch is licensed under the [MIT License](LICENSE).

---

## Community

Join the conversation and get help:

- [Discord Server](https://discord.gg/G6q8FgZq)
