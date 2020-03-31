FROM node:12.13.1 AS builder

WORKDIR /build
RUN \
  git clone \
    https://github.com/paritytech/substrate-telemetry.git \
    --branch master \
    --single-branch \
    . && \
  git checkout 0a89382127b9fb1b95d144cae816c46582975e93
RUN yarn install --frozen-lockfile
ADD ./feed-address.patch .
RUN patch -p1 < feed-address.patch && \
    yarn build:common && \
    yarn build:frontend

FROM python:3.8.0
WORKDIR /htdocs
EXPOSE 8000
COPY --from=builder /build/htdocs /htdocs
CMD ["python", "-m", "http.server"]
