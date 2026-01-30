# VS Code Dev Container for Signal Backup Decode

This dev container provides a complete development environment for the signal-backup-decode project with all required dependencies pre-installed.

## Features

- **Rust toolchain**: Latest stable Rust compiler (1.75+)
- **System dependencies**: 
  - `libsqlite3-dev` - SQLite development libraries
  - `libssl-dev` - OpenSSL development libraries
  - `pkg-config` - Package configuration tool
  - `protobuf-compiler` - Protocol Buffers compiler (for regenerating proto files)
- **Development tools**:
  - `rustfmt` - Rust code formatter
  - `clippy` - Rust linter
- **VS Code extensions**:
  - Rust Analyzer - Language server for Rust
  - Even Better TOML - TOML file support
  - Crates - Cargo.toml dependency management

## Prerequisites

- [VS Code](https://code.visualstudio.com/)
- [Docker](https://www.docker.com/products/docker-desktop)
- [Remote - Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)

## Usage

1. Open this repository in VS Code
2. When prompted, click "Reopen in Container" (or run the command "Remote-Containers: Reopen in Container")
3. Wait for the container to build (first time only)
4. Once the container is ready, you can start developing!

## Building and Testing

Inside the dev container, you can use standard Cargo commands:

```bash
# Build the project
cargo build

# Run tests
cargo test

# Build with verbose output
cargo build --verbose

# Run with clippy (linter)
cargo clippy

# Format code
cargo fmt
```

## Rebuilding Protobuf Files

If you need to regenerate the protobuf files, use the `rebuild-protobuf` feature:

```bash
cargo build --features "rebuild-protobuf"
```

The `protobuf-compiler` is already installed in the container for this purpose.
