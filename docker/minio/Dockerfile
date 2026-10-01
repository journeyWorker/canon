# The required official manifest routes
# (registry-1.docker.io/v2/minio/minio/manifests/* and
# quay.io/v2/minio/minio/manifests/*) return 401/404 here. Build the
# official release source instead of inventing an unverifiable image digest or
# weakening the compose pin policy.
FROM mirror.gcr.io/library/golang@sha256:3d78beb141d98f42337f1252ecf2a5f20374109929a4c3f6817f9e4179cc0ae5 AS build
# golang:1.24.8-alpine3.22; no implicit toolchain downloads.
ENV GOTOOLCHAIN=local

ARG TARGETOS
ARG TARGETARCH
WORKDIR /src
RUN apk add --no-cache ca-certificates curl
RUN curl -fsSL -o /tmp/minio.tar.gz \
      https://codeload.github.com/minio/minio/tar.gz/9e49d5e7a648f00e26f2246f4dc28e6b07f8c84a \
    && printf '%s  %s\n' \
      45521908307306e925c98d629e1c17d78c8b72b6ee242b1bfb1409f7d8ee5841 \
      /tmp/minio.tar.gz | sha256sum -c - \
    && tar -xzf /tmp/minio.tar.gz --strip-components=1 -C /src \
    && rm /tmp/minio.tar.gz

RUN CGO_ENABLED=0 GOOS="$TARGETOS" GOARCH="$TARGETARCH" \
    go build -mod=readonly -tags kqueue -trimpath -o /out/minio .

FROM mirror.gcr.io/library/alpine@sha256:a8560b36e8b8210634f77d9f7f9efd7ffa463e380b75e2e74aff4511df3ef88c
RUN apk add --no-cache ca-certificates
COPY --from=build /out/minio /usr/bin/minio

VOLUME ["/data"]
ENTRYPOINT ["/usr/bin/minio"]
CMD ["server", "/data", "--console-address", ":9001"]
