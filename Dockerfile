# Build stage
FROM rust:1.93.0-slim-bullseye AS builder

# Install build dependencies
RUN apt-get update && \
    apt-get install -y \
    git \
    clang \
    curl \
    libssl-dev \
    llvm \
    libudev-dev \
    protobuf-compiler \
    build-essential \
    --no-install-recommends && \
    rm -rf /var/lib/apt/lists/*

# Configure Rust toolchain to use 1.93.0
RUN rustup default 1.93.0 && \
    rustup target add wasm32-unknown-unknown --toolchain 1.93.0 && \
    rustup component add rust-src --toolchain 1.93.0

# Create and set working directory
WORKDIR /charli3

# Copy only necessary project files
COPY Cargo.toml Cargo.lock ./
COPY node/ node/
COPY toolkit/ toolkit/

# Build the node
RUN cargo build --release

# Final stage
FROM debian:bullseye-20260202-slim

# Install runtime dependencies and curl
RUN apt-get update && \
    apt-get install -y \
    libssl1.1 \
    ca-certificates \
    curl \
    xxd \
    --no-install-recommends && \
    rm -rf /var/lib/apt/lists/*

# Copy the built binaries from builder
COPY --from=builder /charli3/target/release/partner-chains-node /usr/local/bin/
COPY --from=builder /charli3/target/release/partner-chains-cli /usr/local/bin/
COPY --from=builder /charli3/target/release/main-chain-follower-cli /usr/local/bin/

# Create directory for chain data
RUN mkdir -p /data

ENTRYPOINT []
CMD []
