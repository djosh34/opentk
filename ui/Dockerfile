# syntax=docker/dockerfile:1.7

ARG NODE_IMAGE=node:24-bookworm-slim
ARG CADDY_BUILDER_IMAGE=caddy:2.10.2-builder

FROM ${NODE_IMAGE} AS build

ENV PNPM_HOME=/pnpm
ENV PATH="${PNPM_HOME}:${PATH}"
ENV ASTRO_TELEMETRY_DISABLED=1

WORKDIR /src/ui

RUN corepack enable && corepack prepare pnpm@10.24.0 --activate

COPY ui/package.json ui/pnpm-lock.yaml ./

RUN --mount=type=cache,id=opentk-ui-pnpm-store,target=/pnpm/store,sharing=locked \
    pnpm config set store-dir /pnpm/store && \
    pnpm install --frozen-lockfile

COPY ui ./

RUN --mount=type=cache,id=opentk-ui-pnpm-store,target=/pnpm/store,sharing=locked \
    pnpm build

RUN mkdir -p /tmp/caddy

FROM ${CADDY_BUILDER_IMAGE} AS caddy

RUN xcaddy build v2.10.2 --output /tmp/caddy && \
    setcap -r /tmp/caddy

FROM scratch AS runtime

ENV HOME=/tmp
ENV XDG_CONFIG_HOME=/tmp
ENV XDG_DATA_HOME=/tmp

USER 1003:1003

COPY --from=caddy /tmp/caddy /usr/bin/caddy
COPY --from=build --chown=1003:1003 /tmp/caddy /tmp
COPY --from=build /src/ui/dist /srv
COPY ui/Caddyfile /etc/caddy/Caddyfile

EXPOSE 8080

CMD ["/usr/bin/caddy", "run", "--config", "/etc/caddy/Caddyfile", "--adapter", "caddyfile"]
