set dotenv-load

# Port the server listens on
port := "8080"

# Run the server (listens on :{{port}})
run:
    PORT={{port}} cargo run --release

# Run with debug logging (listens on :{{port}})
debug:
    PORT={{port}} RUST_LOG=dotless=debug cargo run --release

# Check compilation
check:
    cargo check

# Build release binary
build:
    cargo build --release
