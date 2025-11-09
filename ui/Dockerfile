# ============================================================================
# TinyLVT UI Docker Build
# ============================================================================
#
# ENVIRONMENT VARIABLES:
#
# Build-time (--build-arg):
#   BACKEND_URL (optional)
#     - The backend API URL embedded in the WASM bundle at compile time
#     - Used by the application code for API calls
#     - Default: Falls back to same-origin (browser's current origin)
#     - Example: --build-arg BACKEND_URL=https://api.tinylvt.com
#
#   SUPPORT_EMAIL (required)
#     - The support email address displayed in the UI help page
#     - Embedded in the WASM bundle at compile time
#     - Example: --build-arg SUPPORT_EMAIL=support@example.com
#
# Runtime (-e or --env):
#   BACKEND_URL (required for production)
#     - The backend API URL injected into nginx CSP header at container start
#     - Allows the frontend to connect to the API (Content-Security-Policy)
#     - Default: http://localhost:8000 (suitable for local development only)
#     - Example: -e BACKEND_URL=https://api.tinylvt.com
#
# USAGE:
#   Build:
#     docker build -f ui/Dockerfile \
#       --build-arg BACKEND_URL=https://api.example.com \
#       --build-arg SUPPORT_EMAIL=support@example.com \
#       -t tinylvt-ui .
#
#   Run:
#     docker run -d -p 80:80 \
#       -e BACKEND_URL=https://api.example.com \
#       tinylvt-ui
#
# NOTE: This Dockerfile must be run from the repository root (not ui/)
# ============================================================================

# Stage 1: Build the UI with Trunk
FROM rust:1.90-bookworm AS builder

# Install dependencies for Trunk and WASM compilation
RUN apt-get update && \
    apt-get install -y binaryen && \
    rm -rf /var/lib/apt/lists/*

# Install cargo-binstall for faster binary installation
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

# Install Trunk using binstall (downloads pre-compiled binary)
# Pin to 0.21.14 for version consistency
RUN cargo binstall --no-confirm trunk@0.21.14

# Add wasm32 target for Rust
RUN rustup target add wasm32-unknown-unknown

# Create working directory
WORKDIR /build

# Copy workspace configuration files from parent directory
COPY Cargo.toml Cargo.lock ./

# Copy all workspace members (required by Cargo workspace)
COPY ui ./ui
COPY payloads ./payloads
COPY api ./api
COPY test-helpers ./test-helpers
COPY email-hash ./email-hash
COPY ui-tests ./ui-tests
COPY dev-server ./dev-server

# Build-time arguments: BACKEND_URL and SUPPORT_EMAIL embedded in WASM bundle
ARG BACKEND_URL
ARG SUPPORT_EMAIL

# Build the UI in release mode
# Note: data-wasm-opt="0" in index.html skips wasm-opt (binaryen 108 compatibility)
WORKDIR /build/ui
RUN BACKEND_URL=${BACKEND_URL} SUPPORT_EMAIL=${SUPPORT_EMAIL} trunk build --release

# Stage 2: Serve with nginx
FROM nginx:alpine

# Copy the built static files from the builder stage
COPY --from=builder /build/ui/dist /usr/share/nginx/html

# Copy nginx configuration template
COPY ui/nginx.conf.template /etc/nginx/nginx.conf.template

# Runtime environment variable: BACKEND_URL for CSP header
ENV BACKEND_URL=http://localhost:8000

# Expose HTTP port
EXPOSE 80

# Generate nginx config from template with envsubst, then start nginx
CMD ["/bin/sh", "-c", "envsubst '${BACKEND_URL}' < /etc/nginx/nginx.conf.template > /etc/nginx/nginx.conf && exec nginx -g 'daemon off;'"]
