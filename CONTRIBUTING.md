# Contributing

Thank you for considering contributing to the IIIF Server project.

## Getting Started

1. Fork and clone the repository
2. Install Rust 1.94+ via [rustup](https://rustup.rs/)
3. Run `cargo build` to verify the setup
4. Run `cargo test` to ensure all tests pass

## Development Workflow

```bash
cargo build                  # Compile
cargo test                   # Run all tests
cargo clippy -- -D warnings  # Lint (must pass with zero warnings)
cargo fmt --check            # Check formatting
```

All four commands must pass before submitting a pull request.

## Pull Requests

- Create a feature branch from `main`
- Write tests for new functionality
- Ensure `cargo clippy -- -D warnings` produces zero warnings
- Ensure `cargo fmt --check` passes
- Follow the commit message convention: `<type>(<module>): <description>`
  - Types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`, `perf`, `security`
  - Example: `feat(image-api): add WebP output support`

## Code Style

- Follow standard Rust conventions (`snake_case`, `CamelCase` for types)
- Maximum line length: 100 characters (enforced by `rustfmt`)
- Document all public APIs with `///` doc comments
- No `unwrap()` in production code; use `expect()` only with descriptive messages for invariants
- No `unsafe` without a `// SAFETY:` comment

## Reporting Issues

- Use GitHub Issues
- Include steps to reproduce, expected vs actual behavior
- Include the IIIF specification section if relevant

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
