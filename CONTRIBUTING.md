# Contributing

Thank you for considering contributing to Regedited!

## Ways to Contribute

- Report bugs via GitHub Issues
- Suggest features via GitHub Issues
- Submit pull requests
- Improve documentation
- Share your use cases

## Development

### Prerequisites

- Rust 1.70 or later
- cargo

### Build

```bash
cargo build --release
```

### Test

```bash
# Run all tests
cargo test

# Run unit tests only
cargo test --lib

# Run in release mode (faster)
cargo test --release

# Run specific test
cargo test ascii_store::tests
```

### Code Style

Follow standard Rust formatting:

```bash
cargo fmt
cargo clippy
```

### Adding a New Command

1. Add the command enum variant in `src/main.rs` under `Commands`
2. Add the handler function at the bottom of `src/main.rs`
3. Wire it in the `run()` match expression
4. Add usage example to `docs/ARCHITECTURE.md`
5. Add test if applicable

### Adding a New Module

1. Create the file in `src/`
2. Add `pub mod new_module;` to `src/lib.rs`
3. Add re-exports to `src/lib.rs` if public API
4. Add module-level documentation (`//! ...`)
5. Add unit tests at the bottom of the file

## Pull Request Process

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes with tests
4. Run `cargo test` and ensure all tests pass
5. Run `cargo fmt` and `cargo clippy`
6. Commit with clear messages
7. Push to your fork
8. Open a Pull Request

## Commit Messages

Use clear, descriptive commit messages:

- `feat: add zone-copy command`
- `fix: handle empty hex-word stores`
- `docs: update Python guide`
- `test: add fast_grep section tests`

## License

By contributing, you agree to help support protection of the open-source through AGPL.
