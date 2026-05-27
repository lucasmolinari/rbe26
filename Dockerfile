FROM rust:1.87-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --bin api --bin preprocessor --bin lb

FROM builder AS vectors
WORKDIR /app
COPY resources/references.json.gz ./resources/
RUN ./target/release/preprocessor

FROM debian:bookworm-slim AS api
WORKDIR /app
COPY --from=builder /app/target/release/api ./api
COPY --from=vectors /app/resources/vectors.bin ./resources/
COPY resources/normalization.json ./resources/
COPY resources/mcc_risk.json ./resources/
CMD ["./api"]

FROM debian:bookworm-slim AS lb
WORKDIR /app
COPY --from=builder /app/target/release/lb ./lb
CMD ["./lb"]
