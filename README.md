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

**A simple yet powerful framework for building modular, interconnected applications with ease.**

## Introduction

Overwatch simplifies the creation of complex systems by enabling seamless communication between independent
components, all while keeping everything self-contained.

Enjoy the flexibility and scalability of microservices without the overhead.

## Requirements

- Rust >= 1.63

## Design Goals

Our architecture is guided by three core principles:

- Modularity:
    - Components are self-contained with well-defined interfaces.
    - Communication between components is explicit and predictable.
    - Mockable design supports testing and measurement.

- Single Responsibility:
    - Each component focuses on one task for easier debugging.
    - Shared state is minimized to reduce complexity.

- Debuggability
    - Workflow is transparent and traceable.
    - Components are designed for easy testing and measurement.
    - Asynchronous communication ensures scalability and clarity.

## Components

- Overwatch: The central messaging relay for internal communications, ensuring seamless interaction between
  components.
    - Responsibilities:
        - Acts as the backbone for coordination, maintaining system stability and reliability.
        - Manages the lifecycle of all services.
        - Handles configuration updates.

- Services: Modular units that perform specific tasks or provide dedicated functionality within the system.

## Project Structure

- `overwatch`: The framework.
- `overwatch-derive`: Provides macros to simplify the implementation of Overwatch components.

## Usage

### Running Tests

Use `cargo test` for executing tests.

Alternatively, if you want to see test outputs, run: `cargo test -- --nocapture`.

### Examples

Use `cargo run --example {example_name}` to run an example.

### Documentation

Simply run `cargo doc --open --no-deps` to build and access a copy of the generated documentation.
