# The required official manifest routes
# (registry-1.docker.io/v2/minio/mc/manifests/* and
# quay.io/v2/minio/mc/manifests/*) return 401/404 here. Build the
# official release source instead of inventing an unverifiable image digest or
# weakening the compose pin policy.
FROM mirror.gcr.io/library/golang@sha256:3d78beb141d98f42337f1252ecf2a5f20374109929a4c3f6817f9e4179cc0ae5 AS build
# golang:1.24.8-alpine3.22; no implicit toolchain downloads.
ENV GOTOOLCHAIN=local

ARG TARGETOS
ARG TARGETARCH
WORKDIR /src
RUN apk add --no-cache ca-certificates curl
RUN curl -fsSL -o /tmp/mc.tar.gz \
      https://codeload.github.com/minio/mc/tar.gz/7394ce0dd2a80935aded936b09fa12cbb3cb8096 \
    && printf '%s  %s\n' \
      95cd293c7119f16921a6dc515a1fb74a2227f19fd994b9c8b770a154e802ac44 \
      /tmp/mc.tar.gz | sha256sum -c - \
    && tar -xzf /tmp/mc.tar.gz --strip-components=1 -C /src \
    && rm /tmp/mc.tar.gz

RUN CGO_ENABLED=0 GOOS="$TARGETOS" GOARCH="$TARGETARCH" \
    go build -mod=readonly -tags kqueue -trimpath -o /out/mc .

FROM mirror.gcr.io/library/alpine@sha256:a8560b36e8b8210634f77d9f7f9efd7ffa463e380b75e2e74aff4511df3ef88c
RUN apk add --no-cache ca-certificates
COPY --from=build /out/mc /usr/bin/mc
ENTRYPOINT ["/usr/bin/mc"]
