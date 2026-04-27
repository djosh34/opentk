# syntax=docker/dockerfile:1.7

ARG RUST_IMAGE=rust:1-bookworm

FROM --platform=$BUILDPLATFORM ${RUST_IMAGE} AS build

ARG TARGETPLATFORM
ARG ZIG_VERSION=0.13.0

SHELL ["/bin/bash", "-euo", "pipefail", "-c"]

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt/lists,sharing=locked \
    apt-get update && \
    apt-get install --no-install-recommends -y \
      build-essential \
      ca-certificates \
      curl \
      xz-utils && \
    rm -rf /var/lib/apt/lists/*

RUN case "$(uname -m)" in \
      x86_64) zig_arch="x86_64" ;; \
      aarch64) zig_arch="aarch64" ;; \
      *) echo "unsupported build architecture: $(uname -m)" >&2; exit 1 ;; \
    esac && \
    curl -fsSL "https://ziglang.org/download/${ZIG_VERSION}/zig-linux-${zig_arch}-${ZIG_VERSION}.tar.xz" \
      | tar -xJ -C /usr/local --strip-components=1 && \
    ln -s /usr/local/zig /usr/local/bin/zig

RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo-install-target,sharing=locked \
    cargo install --locked cargo-zigbuild --target-dir /usr/local/cargo-install-target

RUN rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl

WORKDIR /src

COPY Cargo.toml Cargo.lock rustfmt.toml ./
COPY crates ./crates
COPY migrations ./migrations

RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/src/target,sharing=locked \
    case "${TARGETPLATFORM}" in \
      linux/amd64) rust_target="x86_64-unknown-linux-musl" ;; \
      linux/arm64) rust_target="aarch64-unknown-linux-musl" ;; \
      *) echo "unsupported target platform: ${TARGETPLATFORM}" >&2; exit 1 ;; \
    esac && \
    cargo zigbuild --release --target "${rust_target}" \
      --bin opentk-api \
      --bin opentk-sync && \
    mkdir -p /out/bin && \
    cp "target/${rust_target}/release/opentk-api" /out/bin/opentk-api && \
    cp "target/${rust_target}/release/opentk-sync" /out/bin/opentk-sync

FROM scratch

COPY --from=build /out/bin/opentk-api /bin/opentk-api
COPY --from=build /out/bin/opentk-sync /bin/opentk-sync

CMD ["/bin/opentk-api"]
