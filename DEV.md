# Development

Start a docker container runtime, such as [colima](https://github.com/abiosoft/colima).

Spin up a local postgres database and migrate it:

```
docker pull postgres
docker run --name postgres -e POSTGRES_PASSWORD=password -e POSTGRES_USER=user -e POSTGRES_DB=tinylvt -p 5432:5432 -d postgres
```

To restart container, first do: `docker stop postgres && docker rm postgres`

Run the backend:

```
cd api
DATABASE_URL=postgresql://user:password@localhost:5432/database IP_ADDRESS=127.0.0.1 cargo run
```

Use `cargo watch -x ...` in place of `cargo ...` to watch for filesystem changes.

Build the frontend:

```
cd ui
trunk watch
```

Generate data for testing:

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
