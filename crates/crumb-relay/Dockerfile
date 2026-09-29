# syntax=docker/dockerfile:1
#
# crumb-relay for linux/amd64, linux/arm64 and linux/arm/v7 (see docs/RELAY.md).
# Build context: the repository root.
#
#   A relay that also works for the server (Chromium, cooking videos: docs/RELAY.md, "Relays as
#   workers"), built on the machine that runs it (whisper.cpp is compiled for its CPU):
#     docker build -f crates/crumb-relay/Dockerfile --target worker -t crumb-relay-worker .
#
#   Build it here (on the machine that will run it; slow the first time, BoringSSL is compiled):
#     docker build -f crates/crumb-relay/Dockerfile -t crumb-relay .
#   Or, as CI does, around binaries built beforehand (nothing is compiled or emulated):
#     docker buildx build -f crates/crumb-relay/Dockerfile --build-arg BINARY_SOURCE=prebuilt \
#       --platform linux/amd64,linux/arm64,linux/arm/v7 .   # with dist/crumb-relay-linux-{amd64,arm64,arm}
ARG BINARY_SOURCE=build

# ---- Compile ----
FROM rust:1-slim-bookworm AS build
WORKDIR /app
# wreq builds BoringSSL from source: CMake, make and a C++ compiler for the library, libclang for
# its bindings, git to apply its patches. Build stage only.
RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake g++ git libclang-dev make \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates
# The sites whose terms forbid automated fetching (include_str!)
COPY data ./data
# The desktop app is a workspace member: a stub, so Cargo can load the workspace
COPY desktop/linux/Cargo.toml desktop/linux/Cargo.toml
RUN mkdir -p desktop/linux/src && echo 'fn main() {}' > desktop/linux/src/main.rs \
    && touch desktop/linux/src/lib.rs
RUN cargo build --release --locked -p crumb-relay \
    && install -D target/release/crumb-relay /out/crumb-relay

# ---- Binaries built beforehand ----
FROM scratch AS prebuilt
ARG TARGETARCH
COPY dist/crumb-relay-linux-${TARGETARCH} /out/crumb-relay

FROM ${BINARY_SOURCE} AS binary

# ---- A relay that works for the server: headless Chromium and the video tools ----
# Not a default target: `--target worker`. whisper.cpp is compiled here, for this machine's
# CPU, and yt-dlp is its Python build (every architecture), so a slow first build on a Pi.
FROM debian:bookworm-slim AS worker-tools
ARG WHISPER_VERSION=v1.9.4
ARG WHISPER_MODEL=base.en
ARG WHISPER_MODEL_SHA1=137c40403d78fd54d454da0f9bd998f78703390c
# Keep in step with the server's Dockerfile; SHA-256 of the release's `yt-dlp` (SHA2-256SUMS)
ARG YT_DLP_VERSION=2026.08.19
ARG YT_DLP_SHA256=1fa6733c37ea6fb51c99ad8fe785e7b7e5f3246c9b980230329d4fb72ed8d4d6
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates cmake curl g++ git make \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
RUN git clone --depth 1 --branch "$WHISPER_VERSION" https://github.com/ggml-org/whisper.cpp \
    && cmake -S whisper.cpp -B out -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
       -DWHISPER_BUILD_TESTS=OFF -DWHISPER_BUILD_SERVER=OFF -DWHISPER_SDL2=OFF \
    && cmake --build out --target whisper-cli -j "$(nproc)" \
    && install -D out/bin/whisper-cli /out/bin/whisper-cli
RUN curl -fsSL -o /out/models/ggml-$WHISPER_MODEL.bin --create-dirs \
       "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-$WHISPER_MODEL.bin" \
    && echo "$WHISPER_MODEL_SHA1  /out/models/ggml-$WHISPER_MODEL.bin" | sha1sum -c - \
    && curl -fsSL -o /out/bin/yt-dlp \
       "https://github.com/yt-dlp/yt-dlp/releases/download/$YT_DLP_VERSION/yt-dlp" \
    && echo "$YT_DLP_SHA256  /out/bin/yt-dlp" | sha256sum -c - \
    && chmod 755 /out/bin/yt-dlp

FROM debian:bookworm-slim AS worker
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tini chromium fonts-liberation \
       ffmpeg libgomp1 python3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=worker-tools /out /opt/video
COPY --from=binary /out/crumb-relay /usr/local/bin/crumb-relay
RUN ln -s /opt/video/bin/whisper-cli /opt/video/bin/yt-dlp /usr/local/bin/ \
    && useradd --system --uid 10001 --create-home --home-dir /home/relay --shell /usr/sbin/nologin relay
USER relay
ENV HOME=/home/relay \
    RELAY_LISTEN=0.0.0.0:8787 \
    CHROMIUM_PATH=/usr/bin/chromium \
    WHISPER_MODEL=/opt/video/models/ggml-base.en.bin \
    RUST_LOG=info
EXPOSE 8787
# tini reaps Chromium's helper processes
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/crumb-relay"]

# ---- Run ----
# glibc, libstdc++ and the CA certificates, no shell; runs as a non-root user
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=binary /out/crumb-relay /usr/local/bin/crumb-relay
# The port is reached through Docker's port mapping: publish it on the Tailscale address only,
# e.g. -p 100.101.102.103:8787:8787
ENV RELAY_LISTEN=0.0.0.0:8787 \
    RUST_LOG=info
EXPOSE 8787
ENTRYPOINT ["/usr/local/bin/crumb-relay"]
