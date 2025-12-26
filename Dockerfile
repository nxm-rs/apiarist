# syntax=docker/dockerfile:1

# Build stage
FROM rust:1.92-bookworm AS builder

WORKDIR /app

# Copy all source files
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates

# Build with BuildKit cache mounts for cargo registry and target directory
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    cp target/release/apiarist /apiarist

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /apiarist /usr/local/bin/apiarist

# Create non-root user
RUN useradd -m -u 1000 apiarist
USER apiarist

WORKDIR /home/apiarist

ENTRYPOINT ["apiarist"]
CMD ["--help"]
