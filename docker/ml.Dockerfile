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
#
# Intel GPU (Arc, OpenVINO) build: ONNX Runtime with the OpenVINO execution
# provider is taken from the onnxruntime-openvino wheel (it must match the ONNX
# Runtime version of the `ort` crate) and loaded at runtime. The runtime image
# needs the Intel compute runtime (OpenCL / Level Zero):
#   docker build -f docker/ml.Dockerfile --build-arg FEATURES=openvino \
#     --build-arg ORT_WHEEL_URL=https://files.pythonhosted.org/packages/58/79/7ca125aaa651ed4084f755d2f2e10f949a7eaad6d01aaf9a9f8b9a153248/onnxruntime_openvino-1.22.0-cp312-cp312-manylinux_2_28_x86_64.whl \
#     --build-arg ORT_WHEEL_SHA256=000811fd42b027663cd15fcbbde049871164f7353ed28cab0219a1c10eef8e6d \
#     --build-arg RUNTIME_IMAGE=ubuntu:26.04 \
#     --build-arg "RUNTIME_PACKAGES=intel-opencl-icd libze-intel-gpu1 libze1 ocl-icd-libopencl1" -t pixelhort-ml:openvino .
#   docker run --device /dev/dri -p 3003:3003 -v pixelhort-ml-cache:/cache pixelhort-ml:openvino

ARG RUNTIME_IMAGE=debian:bookworm-slim

FROM rust:1-bookworm AS build
ARG FEATURES=""
ARG ORT_WHEEL_URL=""
ARG ORT_WHEEL_SHA256=""
WORKDIR /src
COPY crates/ml/ ./
COPY docker/clear-execstack.py /usr/local/bin/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked ${FEATURES:+--features "$FEATURES"} \
 && mkdir -p /out/lib \
 && cp target/release/pixelhort-ml /out/ \
 # Dynamically linked ONNX Runtime builds (e.g. CUDA) ship shared libs next to the binary.
 && (find target/release -maxdepth 1 -name 'libonnxruntime*.so*' -exec cp -L {} /out/lib/ \; || true) \
 # Prebuilt ONNX Runtime from a Python wheel (OpenVINO): the shared libs replace any of the above.
 && if [ -n "$ORT_WHEEL_URL" ]; then \
      curl -fsSL -o /tmp/ort.whl "$ORT_WHEEL_URL" \
   && echo "$ORT_WHEEL_SHA256  /tmp/ort.whl" | sha256sum -c - \
   && unzip -q /tmp/ort.whl 'onnxruntime/capi/*.so*' -d /tmp/ort \
   && rm -rf /out/lib/* /tmp/ort/onnxruntime/capi/onnxruntime_pybind11_state*.so \
   && cp /tmp/ort/onnxruntime/capi/*.so* /out/lib/ \
   && mv /out/lib/libonnxruntime.so.* /out/lib/libonnxruntime.so \
   # The wheel's libonnxruntime.so requests an executable stack, which glibc >= 2.41
   # refuses to dlopen. Clear PF_X in its PT_GNU_STACK program header.
   && python3 /usr/local/bin/clear-execstack.py /out/lib/libonnxruntime.so; \
    fi

FROM ${RUNTIME_IMAGE}
ARG RUNTIME_PACKAGES=""
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libstdc++6 ${RUNTIME_PACKAGES} \
 && rm -rf /var/lib/apt/lists/* \
 && mkdir -p /cache
COPY --from=build /out/lib/ /usr/local/lib/pixelhort-ml/
COPY --from=build /out/pixelhort-ml /usr/local/bin/pixelhort-ml
COPY --chmod=755 docker/entrypoint.sh /usr/local/bin/entrypoint.sh
# LD_LIBRARY_PATH also lets builds with ort's load-dynamic (OpenVINO) find libonnxruntime.so.
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
