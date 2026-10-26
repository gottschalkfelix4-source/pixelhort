# syntax=docker/dockerfile:1
# Pixelhort server (API + web app + CLI binary).
#
#   docker build -f docker/server.Dockerfile -t pixelhort-server .

FROM node:24-bookworm-slim AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci
COPY web/ ./
RUN npm run build

FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/common crates/common
COPY crates/server crates/server
COPY crates/cli crates/cli
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p pixelhort-server -p pixelhort-cli \
 && mkdir -p /out && cp target/release/pixelhort-server target/release/pixelhort /out/

FROM debian:bookworm-slim
# ffmpeg: video frames + transcoding, exiftool: metadata, heif-convert: HEIC/AVIF,
# postgresql-client-16: pg_dump for automatic database backups.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl gnupg tini \
      ffmpeg libimage-exiftool-perl libheif-examples \
 && install -d /usr/share/postgresql-common/pgdg \
 && curl -fsSL -o /usr/share/postgresql-common/pgdg/apt.postgresql.org.asc https://www.postgresql.org/media/keys/ACCC4CF8.asc \
 && echo "deb [signed-by=/usr/share/postgresql-common/pgdg/apt.postgresql.org.asc] https://apt.postgresql.org/pub/repos/apt bookworm-pgdg main" \
      > /etc/apt/sources.list.d/pgdg.list \
 && apt-get update && apt-get install -y --no-install-recommends postgresql-client-16 \
 && apt-get purge -y gnupg && apt-get autoremove -y && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /data pixelhort \
 && mkdir -p /data && chown pixelhort /data
COPY --from=build /out/pixelhort-server /usr/local/bin/pixelhort-server
COPY --from=build /out/pixelhort /usr/local/bin/pixelhort
COPY --from=web /web/dist /app/web
ENV PH_DATA_DIR=/data \
    PH_WEB_DIR=/app/web \
    PH_LISTEN=0.0.0.0:2342 \
    PH_METRICS_LISTEN=0.0.0.0:9464
USER pixelhort
VOLUME ["/data"]
EXPOSE 2342 9464
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s \
  CMD ["curl", "-fsS", "http://127.0.0.1:2342/api/server/ping"]
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/pixelhort-server"]
