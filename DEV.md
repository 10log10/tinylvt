# Development

Start a docker container runtime, such as [colima](https://github.com/abiosoft/colima).

Spin up a local postgres database and migrate it:

```
docker pull postgres
docker run --name postgres -e POSTGRES_PASSWORD=password -e POSTGRES_USER=user -e POSTGRES_DB=tinylvt -p 5432:5432 -d postgres
cd api
DATABASE_URL=postgresql://user:password@localhost:5432/tinylvt sqlx migrate run
```

To restart container, first do: `docker stop postgres && docker rm postgres`

## Development with Hot Reloading

For the best development experience with hot reloading:

Run the backend (API server):

```
cd api
DATABASE_URL=postgresql://user:password@localhost:5432/tinylvt \
IP_ADDRESS=127.0.0.1 \
PORT=8000 \
ALLOWED_ORIGINS=* \
cargo run
```

Run the frontend with hot reloading:

```
cd ui
BACKEND_URL=http://localhost:8000 trunk serve
```

This will:
- Backend runs on `http://localhost:8000` 
- Frontend runs on `http://localhost:8080` with hot reloading
- CORS allows any origin (using `*`)

## Production-Style Development

To test production-like CORS restrictions:

```
cd api
DATABASE_URL=postgresql://user:password@localhost:5432/tinylvt \
IP_ADDRESS=127.0.0.1 \
PORT=8000 \
ALLOWED_ORIGINS=http://localhost:8080,http://localhost:3000 \
cargo run
```

```
cd ui
trunk build --release
```

Then serve the built files from `ui/dist/` with a proper web server.

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

- `DATABASE_URL`: PostgreSQL connection string
- `IP_ADDRESS`: Server bind address (`127.0.0.1` for local, `0.0.0.0` for public)
- `PORT`: Server port
- `ALLOWED_ORIGINS`: 
  - Use `*` to allow any origin (development only)
  - Or comma-separated list of specific origins (e.g., `https://app.tinylvt.com,https://tinylvt.com`)

Use `cargo watch -x ...` in place of `cargo ...` to watch for filesystem changes.

TODO: Generate data for testing:

```
DATABASE_URL=postgresql://user:password@localhost:5432/database IP_ADDRESS=127.0.0.1 cargo run --bin gen_test_data
```

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
