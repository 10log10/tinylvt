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

## Stripe webhook endpoints (production)

The API requires two webhook endpoints, created via the Stripe API
rather than the dashboard so their `api_version` matches the version
pinned by async-stripe (`async-stripe-shared`'s `version::VERSION`,
currently `2026-04-22.dahlia`). `api_version` is a creation-time
property: changing it (e.g. after a crate upgrade) means creating a
replacement endpoint and deleting the old one. Overlap while both
exist is safe — duplicate deliveries are absorbed by the handlers'
upsert guards.

The event lists below mirror the handlers in `store/billing.rs`
(platform) and `store/connect.rs` (Connect). A new webhook-driven
feature must add its event types here and to the endpoint, and to
the local-forwarding filter below.

Platform endpoint:

```
curl https://api.stripe.com/v1/webhook_endpoints \
  -u "$STRIPE_API_KEY:" \
  -d url="https://api.tinylvt.com/api/stripe_webhook" \
  -d api_version="2026-04-22.dahlia" \
  -d "enabled_events[]=customer.subscription.created" \
  -d "enabled_events[]=customer.subscription.updated" \
  -d "enabled_events[]=customer.subscription.deleted" \
  -d "enabled_events[]=checkout.session.completed" \
  -d "enabled_events[]=payment_method.detached"
```

Connect endpoint (`connect=true` makes it receive events from
connected accounts):

```
curl https://api.stripe.com/v1/webhook_endpoints \
  -u "$STRIPE_API_KEY:" \
  -d url="https://api.tinylvt.com/api/stripe_connect_webhook" \
  -d connect=true \
  -d api_version="2026-04-22.dahlia" \
  -d "enabled_events[]=account.updated" \
  -d "enabled_events[]=account.application.deauthorized" \
  -d "enabled_events[]=payment_intent.amount_capturable_updated" \
  -d "enabled_events[]=payment_intent.canceled" \
  -d "enabled_events[]=payment_intent.payment_failed" \
  -d "enabled_events[]=payment_intent.processing" \
  -d "enabled_events[]=payment_intent.succeeded" \
  -d "enabled_events[]=checkout.session.completed" \
  -d "enabled_events[]=checkout.session.expired"
```

Each create response contains the signing secret (`"secret":
"whsec_..."`), returned only at creation. Set it as
`STRIPE_WEBHOOK_SECRET` (platform) or
`STRIPE_CONNECT_WEBHOOK_SECRET` (Connect).

To list existing endpoints and delete a superseded one:

```
curl https://api.stripe.com/v1/webhook_endpoints -u "$STRIPE_API_KEY:"
curl -X DELETE "https://api.stripe.com/v1/webhook_endpoints/we_..." \
  -u "$STRIPE_API_KEY:"
```

## Local Stripe testing

Real Stripe calls require the API binary (`cd api && cargo run`), not
dev-server: dev-server builds with the `mock-stripe` feature via
test-helpers' `spawn_app`, which also hardcodes its config (mock keys,
fresh throwaway database) and never reads the Stripe entries in `.env`.

```
stripe listen --forward-to localhost:8000/api/stripe_webhook
```

Then add the printed key to .env (`STRIPE_WEBHOOK_SECRET`). For Connect
events (account status, funding PaymentIntents, and credit-purchase
Checkout sessions), forward separately and put that session's key in
`STRIPE_CONNECT_WEBHOOK_SECRET`:

```
stripe listen --events account.updated,\
account.application.deauthorized,\
payment_intent.amount_capturable_updated,\
payment_intent.canceled,payment_intent.payment_failed,\
payment_intent.processing,payment_intent.succeeded,\
checkout.session.completed,checkout.session.expired \
    --forward-to localhost:8000/api/stripe_connect_webhook
```

Note the first session (no filter, no `--forward-connect-to`) also
receives Connect events and delivers them to the platform endpoint,
which ignores them — harmless, but it makes its log look like Connect
events are being handled when they aren't. Only the filtered session
feeds the Connect handler, so new webhook-driven features must add
their event types to the filter above.

Without forwarding, Connect state still converges: the status endpoint
re-reads the live account on each fetch and reconciles
`stripe_charges_enabled`.

### Testing with the Stripe API

```
export STRIPE_SANDBOX_SECRET_KEY=$(rg -o '^STRIPE_API_KEY=(.*)' -r '$1' .env)
export TEST_CONNECT_ACCOUNT_ID=$(rg -o '^TEST_CONNECT_ACCOUNT_ID=(.*)' -r '$1' .env)
cargo test --test api stripe_sandbox -- --ignored --test-threads=4 --nocapture
```

### Creating a charge-ready connected account via the API

Accounts our code creates must be onboarded through the Stripe-hosted flow, gauntlet included (phone OTP, SSN, captcha), because `create_connected_account` sets `controller.requirement_collection = stripe`, which makes the platform forbidden from writing `business_profile`, `external_account`, `tos_acceptance`, and the person fields.

To skip it for a sandbox fixture, create the account with all four controller properties application-controlled: `requirement_collection = application`, `fees.payer = application`, `losses.payments = application`, `stripe_dashboard.type = none`. Stripe rejects the first unless the other three accompany it. Then prefill everything in that one create call (business profile, individual, external account, ToS, requested capabilities) using the magic values from [Stripe's testing guide](https://docs.stripe.com/connect/testing); a follow-up update hits the same permission rules. `business_profile[url]` rejects `example.com`. Finally, clear the pending document requirement by uploading any image with `purpose=identity_document` and attaching its file id to `individual[verification][document][front]` — the `file_identity_document_success` token isn't accepted there. The account should then report `charges_enabled: true` and empty `currently_due`.

Point a community at it by setting `stripe_account_id` and `stripe_charges_enabled = true` on its row. The account's `default_currency` must match the community's `currency_name`.

Because fees and losses fall on the platform and there's no connected dashboard, these accounts are fine for exercising payments but misleading for onboarding, account standing, or fee accounting — use the hosted flow to test those.

Deauthorization in the sandbox is one-way since it involves deleting the connected account.

### Test cards

- Always succeeds: 4242 4242 4242 4242
- 3DS required: 4000 0027 6000 3184
