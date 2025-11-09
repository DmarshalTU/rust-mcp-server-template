# Multi-stage Docker build for Rust MCP Server
#
# This Dockerfile uses a multi-stage build to create an optimized production image.
# Stage 1 (builder): Compiles the Rust application with all dependencies
# Stage 2 (production): Creates a minimal runtime image with only the binary

# =============================================================================
# Stage 1: Builder
# =============================================================================
FROM rustlang/rust:nightly-slim AS builder

# Install build dependencies required for compiling Rust with SSL support
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set working directory for the build
WORKDIR /app

# Copy dependency files first for Docker layer caching optimization
# This allows Docker to cache the dependency compilation layer separately
# from the source code, speeding up subsequent builds
COPY Cargo.toml Cargo.lock ./

# Create minimal source files to trigger dependency compilation
# This trick allows us to compile dependencies before copying the actual source,
# which significantly speeds up builds when only source code changes
RUN mkdir -p src/core src/tools && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub mod core; pub mod tools;" > src/core/mod.rs && \
    echo "pub mod server;" > src/core/server.rs && \
    echo "pub mod utils;" > src/core/utils.rs && \
    echo "pub mod echo;" > src/tools/echo.rs && \
    echo "pub mod mod;" > src/tools/mod.rs

# Build dependencies only (this layer will be cached if Cargo.toml doesn't change)
# Remove the dummy binary to force recompilation when we add real source
RUN cargo build --release && \
    rm -rf src target/release/deps/mcp-server*

# Copy actual source code and configuration files
COPY src/ ./src/
COPY kmcp.yaml ./
# Create .cargo directory and copy config if it exists
RUN mkdir -p .cargo
COPY .cargo/config.toml ./.cargo/config.toml

# Build the actual application with release optimizations
# Touch main.rs to ensure it's newer than the dummy file, forcing recompilation
# Strip the binary to reduce final image size
RUN touch src/main.rs && \
    cargo build --release && \
    strip target/release/mcp-server

# =============================================================================
# Stage 2: Production Runtime
# =============================================================================
FROM debian:bookworm-slim

# Install only runtime dependencies needed for the application
# ca-certificates is required for HTTPS connections if tools make external API calls
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
# Running as non-root reduces the attack surface if the container is compromised
RUN groupadd -r mcpuser && useradd -r -g mcpuser mcpuser

# Set working directory
WORKDIR /app

# Copy compiled binary and configuration from builder stage
COPY --from=builder /app/target/release/mcp-server /app/mcp-server
COPY --from=builder /app/kmcp.yaml /app/kmcp.yaml

# Change ownership of application files to non-root user
RUN chown -R mcpuser:mcpuser /app

# Switch to non-root user
USER mcpuser

# Expose port for HTTP transport mode
# Default port 3000, can be overridden via PORT environment variable
EXPOSE 3000

# Health check configuration
# Checks the /health endpoint every 30 seconds with 10 second timeout
# Allows 5 seconds for initial startup before starting health checks
# Retries up to 3 times before marking container as unhealthy
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD sh -c 'exec 3<>/dev/tcp/localhost/3000 && echo -e "GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n" >&3 && cat <&3 | grep -q "ok" || exit 1' || exit 1

# Set default environment variables
# RUST_LOG: Logging level (can be overridden)
# MCP_TRANSPORT_MODE: Default to HTTP mode in Docker (can be overridden to stdio)
# HOST: Bind to all interfaces to accept external connections
# PORT: Default port number
ENV RUST_LOG=info
ENV MCP_TRANSPORT_MODE=http
ENV HOST=0.0.0.0
ENV PORT=3000

# Default command runs the server
# The server will use HTTP mode by default due to MCP_TRANSPORT_MODE=http
# Override CMD to use stdio mode: CMD ["./mcp-server"] with MCP_TRANSPORT_MODE=stdio
CMD ["./mcp-server"]
