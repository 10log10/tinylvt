#!/bin/bash
set -e

# Stop and remove existing container (ignore errors if not running)
docker stop postgres 2>/dev/null || true
docker rm postgres 2>/dev/null || true

# Prevent running out of disk space after many restarts
docker system prune -f

# Start the container with a high number of max connections, since we have
# plenty resources to run many tests in parallel.
# Note: we previously hit stochastic database slowdowns and pool timeouts when
# the connection limit was the default of 100, and we were running 55 api tests
# in parallel.
docker run --name postgres \
  -e POSTGRES_PASSWORD=password \
  -e POSTGRES_USER=user \
  -e POSTGRES_DB=tinylvt \
  -p 5433:5432 \
  -d postgres:17-alpine \
  -c max_connections=500

# Wait for postgres to be ready
echo "Waiting for PostgreSQL to start..."
until docker exec postgres pg_isready -U user -d tinylvt > /dev/null 2>&1; do
  sleep 0.5
done
echo "PostgreSQL is ready"

# Run migrations
cd "$(dirname "$0")/api"
DATABASE_URL=postgresql://user:password@localhost:5433/tinylvt sqlx migrate run
echo "Migrations complete"
