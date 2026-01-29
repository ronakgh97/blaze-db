FROM rust:1.92-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create a new directory for the application
WORKDIR /app

# Copy shit
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --bin blzsrv

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy the compiled binary from builder
COPY --from=builder /app/target/release/blzsrv /app/blzsrv

# Copy entrypoint script
COPY docker-entrypoint.sh /app/docker-entrypoint.sh

# Make entrypoint script executable
RUN chmod +x /app/docker-entrypoint.sh

# Create config and source directories upfront
RUN mkdir -p /home/blazedb/.config/blaze /home/blazedb/blaze/sources

EXPOSE 8080

# Set environment variables
ENV RUST_LOG=info
ENV HOME=/home/blazedb
#ENV PORT=8080
#ENV EMBEDDING_MODEL=text-embedding-qwen3-embedding-0.6b
#ENV EMBEDDING_API_URL=http://host.docker.internal:1234/v1/embeddings
#ENV EMBEDDING_API_KEY=local

# Health check - verify the binary exists
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s CMD test -f /app/blzsrv || exit 1

# Set the entrypoint to the initialization script
ENTRYPOINT ["/app/docker-entrypoint.sh"]

CMD ["serve"]