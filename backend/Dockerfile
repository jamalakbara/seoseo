# syntax=docker/dockerfile:1
# SeoSeo backend: one binary (`seoseo serve | worker | migrate | admin`).

FROM rust:1.99-slim-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /src

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /src/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release --locked -p app \
 && cp target/release/seoseo /usr/local/bin/seoseo

FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
COPY --from=builder /usr/local/bin/seoseo /usr/local/bin/seoseo
ENV BIND_ADDR=0.0.0.0:8080 LOG_FORMAT=json RUST_LOG=info
EXPOSE 8080
USER nonroot
ENTRYPOINT ["/usr/local/bin/seoseo"]
CMD ["serve"]
