# Development

Start a docker container runtime, such as [colima](https://github.com/abiosoft/colima).

Spin up a local postgres database and migrate it, or reset it:

```
# Requires an active docker runtime
./reset-dev-db.sh
```

Note that it uses the `postgres` container name, attached to port 5433, and that the script will also cleanup the docker volume with `docker system prune -f`.

## Environment Configuration

Copy the example environment file and customize it:

```bash
cp env.example .env
# Edit .env with your values
```

The `.env` file is used by both `dev-server` and `api` binaries.

## Development with Hot Reloading

Run the development server (creates test data automatically):

```bash
cargo run -p dev-server
```

In a separate terminal, run the frontend:

```bash
cd ui && BACKEND_URL=http://localhost:8000 TRUNK_WATCH_ENABLE_COOLDOWN=true trunk serve
```

The dev-server:
- Creates comprehensive test data (users, communities, auctions)
- Runs the auction scheduler automatically
- Syncs mocked time with real time for browser compatibility
- Prints login credentials for test accounts on startup

### Using the API binary directly

For production-like testing or when you don't need test data:

```bash
cd api && cargo run
```

This requires configuring all environment variables in `.env` (see `env.example`).

## Production Deployment

Frontend and backend are deployed as separate services:

### Backend (API Server)
```bash
# API server - only serves API endpoints
DATABASE_URL=postgresql://... \
IP_ADDRESS=0.0.0.0 \
PORT=8000 \
ALLOWED_ORIGINS=https://app.tinylvt.com \
cargo run --release
```

### Frontend (Static Files)
```bash
# Build static files
cd ui
BACKEND_URL=https://api.tinylvt.com trunk build --release

# Serve with nginx, Apache, or static hosting service
# Files are built to ui/dist/
```

**Production serving options:**
- **Static hosting**: Deploy `ui/dist/` to Vercel, Netlify, Cloudflare Pages
- **CDN**: Upload to S3 + CloudFront, or similar
- **Web server**: Serve `ui/dist/` with nginx/Apache
- **Container**: Package `ui/dist/` in nginx container

**Benefits of separate services:**
- Frontend can be served from CDN (faster, cheaper)
- Backend can scale independently
- Frontend deploys don't affect backend
- Better security isolation

## Environment Variables

### Backend (API Server)
- `DATABASE_URL`: PostgreSQL connection string (port 5433 for local dev)
- `IP_ADDRESS`: Server bind address (`127.0.0.1` for local, `0.0.0.0` for public)
- `PORT`: Server port
- `ALLOWED_ORIGINS`: Comma-separated list of allowed origins (e.g., `http://localhost:8080` for development, `https://tinylvt.com` for production)
- `EMAIL_API_KEY`: API key for email service (e.g., Resend)
- `EMAIL_FROM_ADDRESS`: From address for outgoing emails
- `BASE_URL`: Base URL for email links (optional, defaults to http://localhost:8080)

### Frontend (UI Build)
- `BACKEND_URL`: Backend API URL (optional, defaults to same-origin)

Use `cargo watch -x ...` in place of `cargo ...` to watch for filesystem changes.

## Linting SQL

```
pip install sqlfluff
sqlfluff lint migrations --dialect postgres
```

## Too many open files

Probably need to raise the socket/file descriptor limit. Do so temporarily with:

```
ulimit -n 65535
```

## Viewing logs

```
RUST_LOG=api=info cargo test long_community_name_rejected -- --nocapture
```

## Tracing

Remember that [care must be taken](https://docs.rs/tracing/latest/tracing/struct.Span.html#in-asynchronous-code) when using tracing spans in async code. [Instrument attribute macros](https://docs.rs/tracing/latest/tracing/attr.instrument.html) are the preferred path.

## Compressing PNG files

Using imagemagick:

```
for f in orig/*.png; do
  magick "$f" -quality 50 "$(basename "${f%.png}").jpg"
done
```

Quality depends on the image content. 50 seems fine for screenshots that lack any subtle color gradients. [JPEG quality examples.](https://regex.info/blog/lightroom-goodies/jpeg-quality)

## Running ui Docker Container Locally

```
docker build -f ui/Dockerfile \
    --build-arg BACKEND_URL=http://localhost:8000 \
    -t tinylvt-ui .
```

```
docker run -d -p 8080:80 \
    -e BACKEND_URL=http://localhost:8000 \
    --name tinylvt-ui \
    tinylvt-ui
```

## Local Stripe testing

```
stripe listen --forward-to localhost:8000/api/stripe_webhook
```

Then add the printed key to .env
