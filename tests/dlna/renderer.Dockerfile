# Real DLNA media renderer (gmediarender, as used on Raspberry Pi players) for casting tests.
# docker build -f tests/dlna/renderer.Dockerfile -t dlna-renderer tests/dlna
# docker run -d --name tv-renderer --network <pixelhort network> dlna-renderer
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends gmediarender gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav ca-certificates && rm -rf /var/lib/apt/lists/*
ENTRYPOINT ["gmediarender", "-f", "Wohnzimmer-TV", "--gstout-videosink=fakesink", "--gstout-audiosink=fakesink", "--logfile=/dev/stdout"]
