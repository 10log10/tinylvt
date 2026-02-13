# Stage 1: Build the application
FROM rust:1.90-bookworm AS builder

# Install PostgreSQL development libraries needed for SQLx
RUN apt-get update && apt-get install -y libpq-dev && rm -rf /var/lib/apt/lists/*

# Create a working directory
WORKDIR /build

# Copy workspace configuration files from parent directory
# NOTE: This Dockerfile must be run from the repository root:
#   docker build -f api/Dockerfile .
COPY Cargo.toml Cargo.lock ./

# Copy all workspace members (required by Cargo workspace)
COPY api ./api
COPY payloads ./payloads
COPY test-helpers ./test-helpers
COPY ui ./ui
COPY email-hash ./email-hash
COPY ui-tests ./ui-tests
COPY dev-server ./dev-server
COPY markdown-html ./markdown-html

# Build the api binary in release mode
# The sqlx::migrate!() macro will embed migrations at compile time
RUN cargo build --release --package api

# Stage 2: Create minimal runtime image
FROM debian:bookworm-slim

# Install runtime dependencies:
# - libpq5: PostgreSQL client library (required by SQLx at runtime)
# - ca-certificates: Required for HTTPS connections (email API, etc.)
RUN apt-get update && \
    apt-get install -y libpq5 ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Create a non-root user for security
RUN useradd -m -u 1000 tinylvt

# Copy the compiled binary from builder stage
COPY --from=builder /build/target/release/api /usr/local/bin/api

# Set ownership
RUN chown tinylvt:tinylvt /usr/local/bin/api

# Switch to non-root user
USER tinylvt

# Expose the default port
EXPOSE 8000

# Run the API server
# Migrations will run automatically on startup via sqlx::migrate!()
CMD ["api"]
