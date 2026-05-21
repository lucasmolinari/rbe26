FROM rust:1.87-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

# seed Qdrant
FROM qdrant/qdrant:latest AS qdrant-seeded
COPY --from=builder /app/target/release/preprocessor /app/preprocessor
COPY resources /app/resources
ENV QDRANT__STORAGE__STORAGE_PATH=/qdrant/storage
RUN bash -c '\
    /qdrant/qdrant &>/tmp/qdrant.log & \
    until echo > /dev/tcp/localhost/6333 2>/dev/null; do sleep 1; done && \
    cd /app && QDRANT_URL=http://localhost:6334 ./preprocessor'

# Qdrant loads what is already on disk
FROM qdrant/qdrant:latest AS qdrant-runtime
COPY --from=qdrant-seeded /qdrant/storage /qdrant/storage

FROM debian:bookworm-slim AS api
WORKDIR /app
COPY --from=builder /app/target/release/api ./api
COPY resources/normalization.json ./resources/
COPY resources/mcc_risk.json ./resources/
CMD ["./api"]