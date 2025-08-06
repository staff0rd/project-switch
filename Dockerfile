# Multi-stage build for cross-platform Rust compilation
FROM rust:1.75 as builder

# Install cross-compilation tools
RUN apt-get update && apt-get install -y \
    gcc-mingw-w64-x86-64 \
    && rm -rf /var/lib/apt/lists/*

# Add Windows target
RUN rustup target add x86_64-pc-windows-gnu
RUN rustup target add x86_64-unknown-linux-gnu

WORKDIR /app

# Copy source code
COPY Cargo.toml Cargo.lock* ./
COPY src/ ./src/

# Build for Windows
RUN cargo build --release --target x86_64-pc-windows-gnu

# Build for Linux
RUN cargo build --release --target x86_64-unknown-linux-gnu

# Final stage - copy binaries out
FROM scratch as export
COPY --from=builder /app/target/x86_64-pc-windows-gnu/release/project-switch.exe /windows/
COPY --from=builder /app/target/x86_64-unknown-linux-gnu/release/project-switch /linux/