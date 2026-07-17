# ABE Headless Test Server
# Two-stage build: Rust extension, then mod deploy to Arma 3 DS container

ARG STEAM_USERNAME=anonymous
ARG STEAM_PASSWORD=

FROM rust:1.88-slim-bookworm AS builder

WORKDIR /build
COPY ext/ ./ext/
COPY data/ ./data/

RUN cd ext && cargo test && cargo build --release

# Final stage: SteamCMD base with Arma 3 DS
FROM steamcmd/steamcmd:ubuntu-22 AS arma3server
ARG STEAM_USERNAME
ARG STEAM_PASSWORD

RUN if [ "${STEAM_USERNAME}" != "anonymous" ] && [ -n "${STEAM_PASSWORD}" ]; then \
        steamcmd +@sSteamCmdForcePlatformType linux +force_install_dir /arma3 +login "${STEAM_USERNAME}" "${STEAM_PASSWORD}" +app_update 233780 validate +quit; \
    elif [ "${STEAM_USERNAME}" = "anonymous" ]; then \
        echo "[INFO] Anonymous login — may fail for app 233780"; \
        steamcmd +@sSteamCmdForcePlatformType linux +force_install_dir /arma3 +login anonymous +app_update 233780 validate +quit || \
        echo "[INFO] SteamCMD non-zero exit — continuing anyway"; \
    else \
        echo "[WARN] STEAM_USERNAME set but STEAM_PASSWORD missing — skipping Arma 3 DS download"; \
    fi

FROM arma3server

LABEL description="ABE Headless Test Server"

# Runtime dependencies
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends \
    curl tar ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Install HEMTT
RUN curl -sSfL -o /usr/local/bin/hemtt \
    https://github.com/BrettMayson/HEMTT/releases/latest/download/linux-x64 && \
    chmod +x /usr/local/bin/hemtt

WORKDIR /abe
COPY . /abe/
COPY --from=builder /build/ext/target/release/libabe_ballistics_ext.so /abe/ext/

COPY docker/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
