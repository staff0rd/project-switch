# Multi-stage build for cross-platform Rust compilation
FROM rust:1.82 as builder

ARG BUILD_TARGET=all

# Install cross-compilation tools
RUN apt-get update && apt-get install -y \
    $([ "$BUILD_TARGET" = "windows" ] || [ "$BUILD_TARGET" = "all" ] && echo "gcc-mingw-w64-x86-64") \
    $([ "$BUILD_TARGET" = "linux" ] || [ "$BUILD_TARGET" = "all" ] && echo "gcc-x86-64-linux-gnu") \
    && rm -rf /var/lib/apt/lists/*

# Add targets
RUN if [ "$BUILD_TARGET" = "windows" ] || [ "$BUILD_TARGET" = "all" ]; then \
      rustup target add x86_64-pc-windows-gnu; \
    fi
RUN if [ "$BUILD_TARGET" = "linux" ] || [ "$BUILD_TARGET" = "all" ]; then \
      rustup target add x86_64-unknown-linux-gnu; \
    fi

# Configure cross-linker for x86_64 Linux (needed when building on ARM hosts)
RUN if [ "$BUILD_TARGET" = "linux" ] || [ "$BUILD_TARGET" = "all" ]; then \
      printf '[target.x86_64-unknown-linux-gnu]\nlinker = "x86_64-linux-gnu-gcc"\n' >> /usr/local/cargo/config.toml; \
    fi

WORKDIR /app

# Copy source code
COPY Cargo.toml Cargo.lock* build.rs ./
COPY src/ ./src/
COPY assets/ ./assets/
COPY hotkey/ ./hotkey/

# Build for Windows
RUN if [ "$BUILD_TARGET" = "windows" ] || [ "$BUILD_TARGET" = "all" ]; then \
      cargo build --release --target x86_64-pc-windows-gnu; \
    fi

# Build for Linux
RUN if [ "$BUILD_TARGET" = "linux" ] || [ "$BUILD_TARGET" = "all" ]; then \
      cargo build --release --target x86_64-unknown-linux-gnu; \
    fi

# Build hotkey listener for Windows
RUN if [ "$BUILD_TARGET" = "windows" ] || [ "$BUILD_TARGET" = "all" ]; then \
      cd hotkey && cargo build --release --target x86_64-pc-windows-gnu; \
    fi

# Final stage - copy binaries out
FROM alpine:latest as export
ARG BUILD_TARGET=all
RUN if [ "$BUILD_TARGET" = "windows" ] || [ "$BUILD_TARGET" = "all" ]; then mkdir -p /output/windows; fi
RUN if [ "$BUILD_TARGET" = "linux" ] || [ "$BUILD_TARGET" = "all" ]; then mkdir -p /output/linux; fi
COPY --from=builder /app /app
RUN if [ "$BUILD_TARGET" = "windows" ] || [ "$BUILD_TARGET" = "all" ]; then \
      cp /app/target/x86_64-pc-windows-gnu/release/project-switch.exe /output/windows/ && \
      cp /app/hotkey/target/x86_64-pc-windows-gnu/release/project-switch-hotkey.exe /output/windows/; \
    fi
RUN if [ "$BUILD_TARGET" = "linux" ] || [ "$BUILD_TARGET" = "all" ]; then \
      cp /app/target/x86_64-unknown-linux-gnu/release/project-switch /output/linux/; \
    fi