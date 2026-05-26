FROM rust:1-bookworm AS builder

WORKDIR /src
COPY . .
RUN cargo build --release -p server --bin server --bin generate_invite

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /src/target/release/server /app/server
COPY --from=builder /src/target/release/generate_invite /app/generate_invite

ENV SERVER_ADDR=0.0.0.0:3000
EXPOSE 3000
CMD ["/app/server"]
