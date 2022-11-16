# Overwatch

[<img alt="github" src="https://img.shields.io/badge/github-logos-co/overwatch-rs-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/logos-co/Overwatch)
[<img alt="crates.io" src="https://img.shields.io/crates/v/overwatch-rs.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/overwatch-rs)
[<img alt="docs.rs" src="https://img.shields.io/badge/doc/overwatch-rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/overwatch-rs)
[<img alt="build status" src="https://img.shields.io/github/workflow/logos-co/Overwatch/CI/master?style=for-the-badge" height="20">](https://github.com/logos-co/Overwatch/actions?query=branch%3Amaster)

Overwatch is a framework to easily construct applications that requires of several independent
parts that needs communication between them.
Everything is self-contained, and it matches somewhat the advantages of microservices.

## Design Goals

- Modularity:
    - Components should be self-contained (as possible)
    - Communication relations between components should be specifically defined
    - Components should be mockable. This is rather important for measurements and testing.

- Single responsibility:
    - It is easier to isolate problems
    - Minimal sharing when unavoidable

- Debuggeability
    - Easy to track workflow
    - Easy to test
    - Easy to measure
    - Asynchronous Communication
