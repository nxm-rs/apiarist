# Build stage
FROM rust:1.92-bookworm AS builder

WORKDIR /app

# Copy workspace manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY crates/apiarist-testkit/Cargo.toml ./crates/apiarist-testkit/

# Create dummy sources to cache dependencies
RUN mkdir -p src crates/apiarist-testkit/src && \
    echo "fn main() {}" > src/main.rs && \
    echo "// dummy" > crates/apiarist-testkit/src/lib.rs && \
    cargo build --release && \
    rm -rf src crates/apiarist-testkit/src

# Copy actual sources
COPY src ./src
COPY crates ./crates

# Build release binary
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/apiarist /usr/local/bin/apiarist

# Create non-root user
RUN useradd -m -u 1000 apiarist
USER apiarist

WORKDIR /home/apiarist

ENTRYPOINT ["apiarist"]
CMD ["--help"]
