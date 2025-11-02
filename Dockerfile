# Build stage
FROM rust:1.90-slim AS builder

# Install required packages for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies including netcat for database connectivity check
RUN apt-get update && apt-get install -y \
    ca-certificates \
    netcat-openbsd \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false appuser

# Create app directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/whatsapp-nutrition-bot /app/whatsapp-nutrition-bot

# Copy entrypoint script
COPY docker-entrypoint.sh /app/docker-entrypoint.sh

# Create data directories with proper permissions
RUN mkdir -p /app/data/images && \
    chmod +x /app/docker-entrypoint.sh && \
    chown -R appuser:appuser /app

# Switch to non-root user
USER appuser

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Run the application with entrypoint
ENTRYPOINT ["/app/docker-entrypoint.sh"]
CMD ["./whatsapp-nutrition-bot"]
