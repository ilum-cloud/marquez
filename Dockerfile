FROM rust:slim-bookworm AS builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/src/app
COPY api-rs/ ./api-rs/
COPY api/src/main/resources/marquez/db/migration/ ./api-rs/migrations/
WORKDIR /usr/src/app/api-rs
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /usr/src/app
COPY --from=builder /usr/src/app/api-rs/target/release/marquez-api .
COPY --from=builder /usr/src/app/api-rs/migrations/ ./migrations/
COPY marquez-rs.dev.yml marquez.dev.yml
COPY docker/wait-for-it.sh wait-for-it.sh
COPY docker/entrypoint-rs.sh entrypoint.sh
RUN chmod +x entrypoint.sh wait-for-it.sh
EXPOSE 5000 5001
ENTRYPOINT ["/usr/src/app/entrypoint.sh"]
