# syntax=docker/dockerfile:1

# The frontend is the same for every architecture: build it once, natively.
FROM --platform=$BUILDPLATFORM node:26.13.0-alpine AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci --no-audit --no-fund
COPY web/ ./
RUN npm run build

# A static musl binary, built natively for each target platform.
FROM rust:1.99.1-alpine AS build
RUN apk add --no-cache musl-dev gcc ca-certificates
WORKDIR /src
COPY Cargo.toml Cargo.lock build.rs ./
COPY .cargo .cargo
COPY .sqlx .sqlx
COPY migrations migrations
COPY src src
COPY --from=web /src/web/dist web/dist
# Appended to the version (`--version`), e.g. -dev+abc1234 for a build of a branch.
ARG GRABARR_VERSION_SUFFIX=""
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    set -eux; \
    cargo build --locked --release; \
    mkdir -p /rootfs/etc/ssl/certs /rootfs/config; \
    cp target/release/grabarr /rootfs/grabarr; \
    cp /etc/ssl/certs/ca-certificates.crt /rootfs/etc/ssl/certs/; \
    # The database directory. A new named volume mounted here copies this mode, so it is writable whichever user
    # the container runs as.
    mkdir -m 1777 /rootfs/data

FROM scratch
# Copying the directory's contents keeps the mode of /data, which copying the directory itself would not.
COPY --from=build /rootfs/ /
# nobody:nogroup. Any other uid:gid works too (`user:` in compose).
LABEL org.opencontainers.image.source="https://github.com/nikarh/grabarr" \
      org.opencontainers.image.description="Search Prowlarr, grab to qBittorrent, watch the downloads" \
      org.opencontainers.image.licenses="MIT OR Apache-2.0"
USER 65534:65534
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s CMD ["/grabarr", "healthcheck"]
ENTRYPOINT ["/grabarr"]
