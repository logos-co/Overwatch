# 🤝 Contributing to Overwatch

We're thrilled you're interested in contributing to Overwatch! This document will help you get started.

---

## 📋 Table of Contents

- [Getting Started](#-getting-started)
- [Development Setup](#-development-setup)
- [Development Workflow](#-development-workflow)
- [Code Guidelines](#-code-guidelines)
- [Pull Request Process](#-pull-request-process)
- [Reporting Issues](#-reporting-issues)
- [Getting Help](#-getting-help)

---

## 🚀 Getting Started

### Prerequisites

- **Rust ≥ 1.63** (see [rust-lang.org](https://www.rust-lang.org/tools/install))
- **Git** for version control

### Fork and Clone

```bash
# Fork the repository on GitHub, then:
git clone https://github.com/YOUR_USERNAME/Overwatch.git
cd Overwatch

# Add upstream remote
git remote add upstream https://github.com/logos-co/Overwatch.git
```

---

## 🛠️ Development Setup

### Build the Project

```bash
# Build all crates
cargo build

# Build with all features
cargo build --all-features
```

### Run Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run a specific test
cargo test test_name
```

### Run Examples

```bash
# Run the ping-pong example
cargo run --example ping_pong
```

### Generate Documentation

```bash
# Build and open docs locally
cargo doc --open --no-deps
```

---

## 🔄 Development Workflow

### 1. Create a Branch

```bash
# Sync with upstream
git fetch upstream
git checkout main
git merge upstream/main

# Create your feature branch
git checkout -b feature/your-feature-name
```

### 2. Make Your Changes

- Write your code
- Add tests for new functionality
- Update documentation if needed

### 3. Validate Your Changes

```bash
# Format code
cargo fmt

# Run clippy (warnings are errors in CI!)
cargo clippy -- -D warnings

# Run tests
cargo test

# Check documentation builds
cargo doc --no-deps
```

### 4. Commit Your Changes

We recommend clear, descriptive commit messages:

```bash
git commit -m "feat: add support for service priority"
git commit -m "fix: resolve race condition in relay"
git commit -m "docs: update README with new examples"
```

### 5. Push and Create PR

```bash
git push origin feature/your-feature-name
```

Then open a Pull Request on GitHub.

---

## 📝 Code Guidelines

### Rust Style

- **Format**: Always run `cargo fmt` before committing
- **Linting**: Use `cargo clippy` — all warnings are errors in CI
- **Idioms**: Follow Rust's idiomatic practices

### Code Quality Checklist

- [ ] Code compiles without warnings
- [ ] All tests pass
- [ ] New functionality has tests
- [ ] Public APIs have documentation
- [ ] No unnecessary dependencies added

### Documentation

- Document all public types, functions, and modules
- Include examples in doc comments where helpful
- Update README if adding major features

```rust
/// Sends a message to the specified service.
///
/// # Arguments
///
/// * `message` - The message to send
///
/// # Example
///
/// ```rust
/// relay.send(MyMessage::Hello).await?;
/// ```
pub async fn send(&self, message: M) -> Result<(), Error> {
    // ...
}
```

---

## 🔀 Pull Request Process

### Before Submitting

1. ✅ Ensure all CI checks pass locally
2. ✅ Rebase on latest `main` if needed
3. ✅ Write a clear PR description

### PR Description Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
How were these changes tested?

## Related Issues
Fixes #123
```

### Review Process

1. A maintainer will review your PR
2. Address any feedback
3. Once approved, your PR will be merged

---

## 🐛 Reporting Issues

### Before Creating an Issue

- Search existing issues to avoid duplicates
- Check if it's already fixed in `main`

### Bug Report

Include:
- **Description**: Clear explanation of the bug
- **Steps to Reproduce**: Minimal steps to trigger the issue
- **Expected Behavior**: What should happen
- **Actual Behavior**: What actually happens
- **Environment**: Rust version, OS, etc.
- **Logs/Screenshots**: If applicable

### Feature Request

Include:
- **Problem**: What problem does this solve?
- **Proposed Solution**: How would you implement it?
- **Alternatives**: Other approaches considered
- **Use Cases**: Who benefits from this?

---

## 💬 Getting Help

Stuck or have questions? We're here to help!

| Channel | Use For |
|---------|---------|
| [Discord](https://discord.gg/G6q8FgZq) | Quick questions, discussions |
| [GitHub Issues](https://github.com/logos-co/Overwatch/issues) | Bug reports, feature requests |
| [GitHub Discussions](https://github.com/logos-co/Overwatch/discussions) | General questions, ideas |

---

## 🎉 Thank You!

Every contribution matters — whether it's fixing a typo, improving documentation, or adding a major feature. We appreciate your help in making Overwatch better!

Happy contributing! 🚀