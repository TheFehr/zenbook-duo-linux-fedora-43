# Development Guide

## Release Process

This project uses `cargo-release` for managing versioning and tags.

To release a new version (e.g., a patch for bug fixes):

```bash
cargo release patch --execute
```

This will:
1. Bump the version in `Cargo.toml`.
2. Commit the change.
3. Create a git tag.
4. Push to the remote repository (if configured).
5. Trigger the CI/CD release workflow.

## Environment Setup

Ensure you have the following tools installed:
- Rust toolchain (latest stable)
- `cargo-release`: `cargo install cargo-release`
- `gdctl` (for GNOME testing)
- `kscreen-doctor` (for KDE testing)
