# syntax=docker/dockerfile:1

# ---- Frontend: Astro static build (HTML, CSS, JS, fonts, precompressed) ----
FROM node:22-bookworm-slim AS web
WORKDIR /web
RUN npm install -g bun@1.3
COPY web/package.json web/bun.lock ./
RUN bun install --frozen-lockfile
COPY web/ ./
# With a Sentry token (CI passes it as a BuildKit secret, never a layer), hidden source maps
# are uploaded to Sentry for this release. Maps are never shipped: any the upload left behind
# are deleted here. Without a token the build makes none.
ARG SENTRY_ORG SENTRY_PROJECT SENTRY_RELEASE
RUN --mount=type=secret,id=sentry_auth_token,env=SENTRY_AUTH_TOKEN \
    bun run build \
    && find dist -name '*.map' -delete

# ---- Server: Rust ----
FROM rust:1-slim-bookworm AS server
WORKDIR /app
# wreq builds BoringSSL from source: CMake, make and a C++ compiler for the library, libclang for
# its bindings, git to apply its patches. Build stage only; the binary links it statically.
RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake g++ git libclang-dev make \
    && rm -rf /var/lib/apt/lists/*
# Build dependencies against stub sources first, so code-only changes reuse this layer
COPY Cargo.toml Cargo.lock ./
COPY crates/crumb-core/Cargo.toml crates/crumb-core/Cargo.toml
COPY crates/crumb-client/Cargo.toml crates/crumb-client/Cargo.toml
COPY crates/crumb-ffi/Cargo.toml crates/crumb-ffi/Cargo.toml
COPY crates/crumb-fetch/Cargo.toml crates/crumb-fetch/Cargo.toml
COPY crates/crumb-relay/Cargo.toml crates/crumb-relay/Cargo.toml
# Only the server is built here; the other members get stubs so Cargo can load the workspace
RUN mkdir -p src crates/crumb-core/src crates/crumb-client/src crates/crumb-ffi/src/bin crates/crumb-fetch/src crates/crumb-relay/src \
    && echo 'fn main() {}' > src/main.rs \
    && echo 'fn main() {}' > crates/crumb-relay/src/main.rs \
    && touch src/lib.rs crates/crumb-core/src/lib.rs crates/crumb-client/src/lib.rs crates/crumb-ffi/src/lib.rs crates/crumb-fetch/src/lib.rs crates/crumb-relay/src/lib.rs \
    && echo 'fn main() {}' > crates/crumb-ffi/src/bin/uniffi-bindgen.rs \
    && cargo build --release --locked && rm -rf src crates
COPY src ./src
COPY crates ./crates
# Reviewed lists compiled into the server (include_str!)
COPY data ./data
RUN find src crates -name '*.rs' -exec touch {} + && cargo build --release --locked

# ---- Video tools: whisper.cpp (speech to text, on the CPU), its model, and yt-dlp ----
FROM debian:bookworm-slim AS video
ARG WHISPER_VERSION=v1.9.4
# ggml-base.en: English, 142 MB, a few seconds per minute of speech. SHA-1 from whisper.cpp's
# models/README.md
ARG WHISPER_MODEL=base.en
ARG WHISPER_MODEL_SHA1=137c40403d78fd54d454da0f9bd998f78703390c
# yt-dlp follows TikTok's and Instagram's changes; bump this when video imports start failing.
# SHA-256 from the release's SHA2-256SUMS
ARG YT_DLP_VERSION=2026.08.19
ARG YT_DLP_SHA256=58162f9bfdc27458ea47bfcb311cf47028f17d8154a8bf7d689861d46399230a
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates cmake curl g++ git make \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
# GGML_NATIVE=OFF: the build machine's CPU isn't the one it runs on
RUN git clone --depth 1 --branch "$WHISPER_VERSION" https://github.com/ggml-org/whisper.cpp \
    && cmake -S whisper.cpp -B out -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
       -DGGML_NATIVE=OFF -DWHISPER_BUILD_TESTS=OFF -DWHISPER_BUILD_SERVER=OFF -DWHISPER_SDL2=OFF \
    && cmake --build out --target whisper-cli -j "$(nproc)" \
    && install -D out/bin/whisper-cli /out/bin/whisper-cli
RUN curl -fsSL -o /out/models/ggml-$WHISPER_MODEL.bin --create-dirs \
       "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-$WHISPER_MODEL.bin" \
    && echo "$WHISPER_MODEL_SHA1  /out/models/ggml-$WHISPER_MODEL.bin" | sha1sum -c - \
    && curl -fsSL -o /out/bin/yt-dlp \
       "https://github.com/yt-dlp/yt-dlp/releases/download/$YT_DLP_VERSION/yt-dlp_linux" \
    && echo "$YT_DLP_SHA256  /out/bin/yt-dlp" | sha256sum -c - \
    && chmod 755 /out/bin/yt-dlp

# ---- Run ----
FROM debian:bookworm-slim
WORKDIR /app

# Headless Chromium for sites that block plain HTTP scraping (adds ~250 MB).
# Build with --build-arg WITH_CHROMIUM=false for a slimmer image without the fallback.
ARG WITH_CHROMIUM=true
# Recipes from cooking videos (src/video.rs): ffmpeg for audio and stills, whisper.cpp (needs
# libgomp) and yt-dlp from the video stage. --build-arg WITH_VIDEO=false leaves them out;
# captions are still read then.
ARG WITH_VIDEO=true
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tini \
       $(if [ "$WITH_CHROMIUM" = "true" ]; then echo chromium fonts-liberation; fi) \
       $(if [ "$WITH_VIDEO" = "true" ]; then echo ffmpeg libgomp1; fi) \
    && rm -rf /var/lib/apt/lists/*
COPY --from=video /out /opt/video
RUN if [ "$WITH_VIDEO" = "true" ]; then \
      ln -s /opt/video/bin/whisper-cli /opt/video/bin/yt-dlp /usr/local/bin/ \
      && whisper-cli --help > /dev/null 2>&1 && yt-dlp --version && ffmpeg -hide_banner -version | head -1; \
    else rm -rf /opt/video; fi

# Railway injects PORT at runtime; 3000 is the local default
ENV HOST=0.0.0.0 \
    PORT=3000 \
    WEB_DIST=/app/web \
    CHROMIUM_PATH=/usr/bin/chromium \
    WHISPER_MODEL=/opt/video/models/ggml-base.en.bin \
    RUST_LOG=info
# glibc otherwise keeps up to 8 arenas per core of freed resize buffers (~220 MB that never
# shrinks); these bring steady-state RSS to ~20 MB with no measurable speed cost
ENV MALLOC_ARENA_MAX=2 \
    MALLOC_MMAP_THRESHOLD_=131072 \
    MALLOC_TRIM_THRESHOLD_=131072
# The version CI built (Sentry's release name); error reporting stays off unless SENTRY_DSN is set
ARG CRUMB_VERSION=""
ENV SENTRY_RELEASE=$CRUMB_VERSION
COPY --from=server /app/target/release/crumb /usr/local/bin/crumb
COPY --from=web /web/dist /app/web
EXPOSE 3000
# tini as PID 1 reaps Chromium's orphaned helper processes
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["crumb"]
