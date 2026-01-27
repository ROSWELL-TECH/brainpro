# Brainpro multi-stage Docker build
# Runs both gateway and agent daemons via supervisord

FROM rust:1.83-slim-bookworm as builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy source code
COPY . .

# Build release binaries
RUN cargo build --release

# Runtime image
FROM debian:bookworm-slim

# Create non-root user
RUN groupadd -r brainpro && useradd -r -g brainpro brainpro

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    supervisor \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /run /var/log/supervisor /app/data /app/logs \
    && chown -R brainpro:brainpro /app /run /var/log/supervisor

# Copy binaries from builder
COPY --from=builder /app/target/release/brainpro-gateway /usr/local/bin/
COPY --from=builder /app/target/release/brainpro-agent /usr/local/bin/
COPY --from=builder /app/target/release/brainpro /usr/local/bin/

# Copy supervisord config
COPY supervisord.conf /etc/supervisor/conf.d/brainpro.conf

# Copy entrypoint script
COPY docker-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# Expose gateway port
EXPOSE 18789

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:18789/health || exit 1

# Set working directory
WORKDIR /app

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["supervisord", "-n", "-c", "/etc/supervisor/supervisord.conf"]
