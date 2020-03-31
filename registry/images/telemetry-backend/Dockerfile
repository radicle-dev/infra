FROM rust:1.39.0-stretch AS builder

WORKDIR /build
RUN \
  git clone \
    https://github.com/paritytech/substrate-telemetry.git \
    --branch master \
    --single-branch \
    . && \
  git checkout 0a89382127b9fb1b95d144cae816c46582975e93
RUN cd backend && cargo build --release

FROM debian:stretch
COPY --from=builder /build/backend/target/release/telemetry /polkadot-telemetry
RUN apt-get update && apt-get install libssl1.1
CMD ["/polkadot-telemetry"]
