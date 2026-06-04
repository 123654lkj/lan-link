# Contributing to lan-link

Thank you for considering contributing to lan-link!

## How to Contribute

1. **Fork the repository** on GitHub.
2. **Create a feature branch** from `main`.
3. **Make your changes** following the existing code style.
4. **Run tests**: `cargo test` and `cargo clippy`.
5. **Submit a Pull Request** with a clear description of your changes.

## Code Style

- Follow standard Rust formatting (`cargo fmt`).
- Keep functions focused and well-documented.
- Use `tracing` for logging, not `println!`.
- All new features should include tests.

## Commit Messages

Use conventional commits (e.g., `feat:`, `fix:`, `docs:`, `refactor:`).

## Development Setup

```bash
# Clone the repo
git clone https://github.com/123654lkj/lan-link.git
cd lan-link

# Build
cargo build

# Run daemon
sudo ./target/debug/lan-linkd

# Run client (from another machine)
./target/debug/lan-linkctl exec "whoami"
```

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
