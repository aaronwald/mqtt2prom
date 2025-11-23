# Build stage
FROM rust:1.91 AS builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build release binary
RUN cargo build --release && \
    strip target/release/mqtt2prom

# Runtime stage
FROM debian:trixie-slim

# Install CA certificates for HTTPS
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/mqtt2prom /usr/local/bin/mqtt2prom

# Create non-root user
RUN useradd -u 1000 -U -s /bin/false mqtt2prom

# Security: run as non-root
USER mqtt2prom:mqtt2prom

# Expose metrics port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/bin/curl", "-f", "http://localhost:8080/health", "||", "exit", "1"]

CMD ["mqtt2prom"]
