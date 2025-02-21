[![MIT licensed][mit-badge]][mit-url]  
[![Build Status][actions-badge]][actions-url]  
[![Codecov Status][codecov-badge]][codecov-url]

[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg

[mit-url]: https://github.com/logos-co/Overwatch/blob/master/LICENSE

[actions-badge]: https://github.com/logos-co/Overwatch/workflows/CI/badge.svg

[actions-url]: https://github.com/logos-co/Overwatch/actions/workflows/main.yml?query=workflow%3ACI+branch%3Amain

[codecov-badge]: https://codecov.io/github/logos-co/Overwatch/branch/main/graph/badge.svg?token=H4CQWRUCUS

[codecov-url]: https://codecov.io/github/logos-co/Overwatch

# Overwatch

**A lightweight framework for building modular, interconnected applications.**

Overwatch simplifies the development of complex systems by enabling seamless communication between independent
components. It combines the flexibility of microservices with the simplicity of a unified framework.

---

## Requirements

- Rust ≥ 1.63

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
