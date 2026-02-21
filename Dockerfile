# Toolchain image for cross-platform Rust compilation
# Uses cargo-zigbuild image which includes macOS SDK for framework linking
FROM ghcr.io/rust-cross/cargo-zigbuild:latest

# Install cross-compilation tools
RUN apt-get update && apt-get install -y \
    gcc-mingw-w64-x86-64 \
    gcc-x86-64-linux-gnu \
    && rm -rf /var/lib/apt/lists/*

# Add compilation targets
RUN rustup target add x86_64-pc-windows-gnu
RUN rustup target add x86_64-unknown-linux-gnu
RUN rustup target add aarch64-apple-darwin

# Configure cross-linker for x86_64 Linux (needed when building on ARM hosts)
RUN printf '[target.x86_64-unknown-linux-gnu]\nlinker = "x86_64-linux-gnu-gcc"\n' >> /usr/local/cargo/config.toml

WORKDIR /app
