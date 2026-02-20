# Multi-stage build for cross-platform Rust compilation
FROM rust:1.82 as builder

# Install cross-compilation tools
RUN apt-get update && apt-get install -y \
    gcc-mingw-w64-x86-64 \
    gcc-x86-64-linux-gnu \
    && rm -rf /var/lib/apt/lists/*

# Add Windows target
RUN rustup target add x86_64-pc-windows-gnu
RUN rustup target add x86_64-unknown-linux-gnu

# Configure cross-linker for x86_64 Linux (needed when building on ARM hosts)
RUN printf '[target.x86_64-unknown-linux-gnu]\nlinker = "x86_64-linux-gnu-gcc"\n' >> /usr/local/cargo/config.toml

WORKDIR /app

# Copy source code
COPY Cargo.toml Cargo.lock* build.rs ./
COPY src/ ./src/
COPY assets/ ./assets/
COPY hotkey/ ./hotkey/

# Build for Windows
RUN cargo build --release --target x86_64-pc-windows-gnu

# Build for Linux
RUN cargo build --release --target x86_64-unknown-linux-gnu

# Build hotkey listener for Windows
RUN cd hotkey && cargo build --release --target x86_64-pc-windows-gnu

# Final stage - copy binaries out
FROM alpine:latest as export
RUN mkdir -p /output/windows /output/linux
COPY --from=builder /app/target/x86_64-pc-windows-gnu/release/project-switch.exe /output/windows/
COPY --from=builder /app/target/x86_64-unknown-linux-gnu/release/project-switch /output/linux/
COPY --from=builder /app/hotkey/target/x86_64-pc-windows-gnu/release/project-switch-hotkey.exe /output/windows/