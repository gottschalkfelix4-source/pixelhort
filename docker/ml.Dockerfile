# syntax=docker/dockerfile:1
# Pixelhort ML service.
#
#   docker build -f docker/ml.Dockerfile -t pixelhort-ml .
#   docker run -p 3003:3003 -v pixelhort-ml-cache:/cache pixelhort-ml
#
# GPU (CUDA) build: the CUDA build of ONNX Runtime is linked dynamically and
# needs CUDA 12 + cuDNN 9 at runtime, e.g.
#   docker build -f docker/ml.Dockerfile --build-arg FEATURES=cuda \
#     --build-arg RUNTIME_IMAGE=nvidia/cuda:12.8.1-cudnn-runtime-ubuntu22.04 -t pixelhort-ml:cuda .
#   docker run --gpus all -p 3003:3003 -v pixelhort-ml-cache:/cache pixelhort-ml:cuda

ARG RUNTIME_IMAGE=debian:bookworm-slim

FROM rust:1-bookworm AS build
ARG FEATURES=""
WORKDIR /src
COPY crates/ml/ ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked ${FEATURES:+--features "$FEATURES"} \
 && mkdir -p /out/lib \
 && cp target/release/pixelhort-ml /out/ \
 # Dynamically linked ONNX Runtime builds (e.g. CUDA) ship shared libs next to the binary.
 && (find target/release -maxdepth 1 -name 'libonnxruntime*.so*' -exec cp -L {} /out/lib/ \; || true)

FROM ${RUNTIME_IMAGE}
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libstdc++6 \
 && rm -rf /var/lib/apt/lists/* \
 && mkdir -p /cache
COPY --from=build /out/lib/ /usr/local/lib/pixelhort-ml/
COPY --from=build /out/pixelhort-ml /usr/local/bin/pixelhort-ml
COPY --chmod=755 docker/entrypoint.sh /usr/local/bin/entrypoint.sh
# The entrypoint starts as root only to fix ownership of /cache, then runs the
# service as PUID:PGID (Unraid: 99:100).
ENV LD_LIBRARY_PATH=/usr/local/lib/pixelhort-ml \
    PH_ML_LISTEN=0.0.0.0:3003 \
    PH_ML_CACHE_DIR=/cache \
    RUST_LOG=info,ort=warn \
    PH_WRITABLE_DIRS=/cache \
    PUID=10001 \
    PGID=10001
VOLUME ["/cache"]
EXPOSE 3003
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
  CMD ["/bin/sh", "-c", "exec 3<>/dev/tcp/127.0.0.1/3003 && printf 'GET /ping HTTP/1.0\r\n\r\n' >&3 && grep -q pong <&3"]
ENTRYPOINT ["/usr/local/bin/entrypoint.sh", "/usr/local/bin/pixelhort-ml"]
