#!/usr/bin/env bash
# The three smokes a release runs before anything is published (publish.yml
# `smoke-binary`, `smoke-install`, `smoke-network`). Each starts a container of
# an image pinned by digest, mounts what it tests, and runs the commands in it;
# the container gets no tool the image does not have except `git` (canon calls
# it) and `ca-certificates` (the S3 path reads the system CA file), which the
# smoke installs itself. Run it where docker runs, on a binary built anywhere
# for the container's architecture:
#
#   scripts/release-smoke.sh binary  --binary PATH --image IMAGE --version X.Y.Z [--repo DIR]
#       The binary in the container: `--version` (must be X.Y.Z), `skills show canon`,
#       and `search` and `gate check` on the checkout DIR (SQLite FTS, the allocator,
#       git). debian:12 (glibc 2.36, where 0.41 died) and alpine (musl) both.
#
#   scripts/release-smoke.sh install --launcher TGZ --platform TGZ --image NODE_IMAGE --version X.Y.Z
#       The two tarballs a release publishes (the launcher and this architecture's
#       platform package), installed into a clean prefix of a Node image;
#       the launcher's `canon --version` must be X.Y.Z.
#
#   scripts/release-smoke.sh network --binary PATH --image IMAGE --postgres-image IMAGE [--repo DIR] [--s3-endpoint URL]
#       The binary's own network path, in a static binary that resolves names with
#       musl's resolver and has no NSS: (a) the repo's hot tier (postgres) is attached by
#       the HOST NAME `postgres` on a user-defined docker network, with a one-time
#       password, and the schema is created; (b) the cold tier (S3) is read through
#       HTTPS (default https://s3.us-east-1.amazonaws.com) with a fake key and a bucket
#       that does not exist: an authentication rejection is the pass, a name
#       resolution, connection or certificate failure is the failure. No real secret is used.
#       A host whose certificate nobody trusts (https://untrusted-root.badssl.com as
#       --s3-endpoint) must fail with a certificate error: that is the negative control.
#
# Every smoke exits 0 when all its commands passed and 1 on the first that did not,
# printing what it ran and what came back.
set -euo pipefail

die() {
  echo "release-smoke: FAILED: $*" >&2
  exit 1
}

usage() {
  sed -n '2,/^set -euo/p' "$0" | sed '$d' | sed 's/^# \{0,1\}//' >&2
  exit 2
}

[ $# -ge 1 ] || usage
MODE=$1
shift

BINARY="" LAUNCHER="" PLATFORM="" IMAGE="" VERSION="" REPO="" POSTGRES_IMAGE=""
S3_ENDPOINT="https://s3.us-east-1.amazonaws.com"
while [ $# -gt 0 ]; do
  case "$1" in
    --binary) BINARY=${2:?--binary needs a value}; shift 2 ;;
    --launcher) LAUNCHER=${2:?--launcher needs a value}; shift 2 ;;
    --platform) PLATFORM=${2:?--platform needs a value}; shift 2 ;;
    --image) IMAGE=${2:?--image needs a value}; shift 2 ;;
    --version) VERSION=${2:?--version needs a value}; shift 2 ;;
    --repo) REPO=${2:?--repo needs a value}; shift 2 ;;
    --postgres-image) POSTGRES_IMAGE=${2:?--postgres-image needs a value}; shift 2 ;;
    --s3-endpoint) S3_ENDPOINT=${2:?--s3-endpoint needs a value}; shift 2 ;;
    *) echo "release-smoke: unknown argument $1" >&2; usage ;;
  esac
done
[ -n "$IMAGE" ] || die "--image is required"
REPO=${REPO:-$PWD}
REPO=$(cd "$REPO" && pwd)

# The commands run in the container are a script file mounted read-only, not stdin: a command that reads stdin
# (canon, apt) must not be able to swallow the rest of the script.
WORK=$(mktemp -d)
cleanup_work() { rm -rf "$WORK"; }
trap cleanup_work EXIT

abspath() { (cd "$(dirname "$1")" && printf '%s/%s\n' "$PWD" "$(basename "$1")"); }

# A build row's artifact does not keep the executable bit; the checksum handoff travels beside the binary.
prepare_binary() {
  [ -f "$BINARY" ] || die "no binary at $BINARY"
  BINARY=$(abspath "$BINARY")
  BIN_DIR=$(dirname "$BINARY")
  BIN_NAME=$(basename "$BINARY")
  if [ -f "$BINARY.sha256" ]; then
    (cd "$BIN_DIR" && sha256sum -c "$BIN_NAME.sha256") || die "the checksum of $BINARY does not match its handoff"
  fi
  chmod +x "$BINARY"
}

# What a container runs first: `git` (canon calls it), the CA file when NEED_CA=1, and the safe.directory
# setting that keeps git from refusing a checkout the container's root does not own.
write_setup() {
  cat >"$WORK/setup.sh" <<'SH'
set -eu
need_ca="${NEED_CA:-0}"
pkgs=""
command -v git >/dev/null 2>&1 || pkgs="$pkgs git"
if [ "$need_ca" = 1 ] && [ ! -e /etc/ssl/certs/ca-certificates.crt ]; then pkgs="$pkgs ca-certificates"; fi
if [ -n "$pkgs" ]; then
  echo "+ install:$pkgs"
  if [ -f /etc/alpine-release ]; then
    apk add --no-cache --quiet $pkgs
  else
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq >/dev/null
    apt-get install -y -qq --no-install-recommends $pkgs >/dev/null
  fi
fi
git config --global --add safe.directory '*'
echo "image: $(. /etc/os-release && echo "$PRETTY_NAME")"
# The libc in the log: glibc prints its version here, musl prints its banner on stderr (and exits 1).
(ldd --version 2>&1 || true) | head -n 1
SH
}

case "$MODE" in
  binary)
    [ -n "$BINARY" ] && [ -n "$VERSION" ] || die "binary needs --binary and --version"
    prepare_binary
    write_setup
    cat >"$WORK/inner.sh" <<'SH'
set -eu
. /smoke/setup.sh
canon=/canon-bin/$BIN_NAME
fail() { echo "release-smoke: FAILED: $*" >&2; exit 1; }
echo "+ canon --version"
out=$("$canon" --version)
echo "$out"
[ "$out" = "canon $EXPECTED_VERSION" ] || fail "--version is '$out', the workspace version is $EXPECTED_VERSION"
echo "+ canon skills show canon"
size=$("$canon" skills show canon | wc -c)
echo "$size bytes"
[ "$size" -gt 1000 ] || fail "skills show canon printed almost nothing"
echo "+ canon search approval --json"
"$canon" search approval --json >/tmp/search.json || { head -c 2000 /tmp/search.json; fail "canon search exited non-zero"; }
echo "$(wc -c </tmp/search.json) bytes"
echo "+ canon gate check"
"$canon" gate check || fail "canon gate check exited non-zero"
echo "release-smoke binary: OK"
SH
    echo "release-smoke binary: $BINARY in $IMAGE (checkout $REPO)"
    docker run --rm \
      -e "EXPECTED_VERSION=$VERSION" -e "BIN_NAME=$BIN_NAME" \
      -v "$BIN_DIR:/canon-bin:ro" -v "$REPO:/repo" -v "$WORK:/smoke:ro" -w /repo \
      "$IMAGE" sh /smoke/inner.sh
    ;;

  install)
    [ -n "$LAUNCHER" ] && [ -n "$PLATFORM" ] && [ -n "$VERSION" ] || die "install needs --launcher, --platform and --version"
    [ -f "$LAUNCHER" ] || die "no launcher tarball at $LAUNCHER"
    [ -f "$PLATFORM" ] || die "no platform tarball at $PLATFORM"
    LAUNCHER=$(abspath "$LAUNCHER")
    PLATFORM=$(abspath "$PLATFORM")
    # Both tarballs in one install and no registry: the launcher's optionalDependencies name platform packages that are
    # not on npm yet in a first release, and the platform package it needs is the other tarball.
    cat >"$WORK/inner.sh" <<'SH'
set -eu
echo "image: $(. /etc/os-release && echo "$PRETTY_NAME"); node $(node --version); npm $(npm --version)"
prefix=/tmp/canon-prefix
echo "+ npm install -g --prefix $prefix --offline launcher.tgz platform.tgz"
npm install -g --prefix "$prefix" --offline --no-audit --no-fund --loglevel=error /tarballs/launcher.tgz /tarballs/platform.tgz
ls "$prefix/lib/node_modules/@journeykit"
echo "+ canon --version"
out=$("$prefix/bin/canon" --version)
echo "$out"
[ "$out" = "canon $EXPECTED_VERSION" ] || { echo "release-smoke: FAILED: the installed canon says '$out', the workspace version is $EXPECTED_VERSION" >&2; exit 1; }
echo "release-smoke install: OK"
SH
    echo "release-smoke install: $(basename "$LAUNCHER") + $(basename "$PLATFORM") in $IMAGE"
    docker run --rm \
      -e "EXPECTED_VERSION=$VERSION" \
      -v "$LAUNCHER:/tarballs/launcher.tgz:ro" -v "$PLATFORM:/tarballs/platform.tgz:ro" -v "$WORK:/smoke:ro" \
      "$IMAGE" sh /smoke/inner.sh
    ;;

  network)
    [ -n "$BINARY" ] && [ -n "$POSTGRES_IMAGE" ] || die "network needs --binary and --postgres-image"
    prepare_binary
    write_setup
    cat >"$WORK/inner.sh" <<'SH'
set -eu
. /smoke/setup.sh
canon=/canon-bin/$BIN_NAME
echo "+ canon query --kind task --json   (hot tier: postgres at the host name postgres)"
if ! "$canon" query --kind task --json >/tmp/hot.json 2>&1; then
  cat /tmp/hot.json
  echo "NETWORK-SMOKE-RESULT: hot-tier-failed"
  exit 0
fi
cat /tmp/hot.json
echo
echo "+ canon query --kind trajectory --json   (cold tier: S3 at $CANON_S3_ENDPOINT, a fake key, a bucket that does not exist)"
set +e
"$canon" query --kind trajectory --json >/tmp/cold.json 2>&1
cold=$?
set -e
cat /tmp/cold.json
echo
echo "cold tier exit status: $cold"
if grep -Eq 'InvalidAccessKeyId|SignatureDoesNotMatch|AccessDenied|InvalidToken|403 Forbidden|401 Unauthorized' /tmp/cold.json; then
  echo "NETWORK-SMOKE-RESULT: s3-rejected-authentication"
elif grep -Eiq 'dns error|failed to lookup|name or service not known|temporary failure in name resolution|no address associated' /tmp/cold.json; then
  echo "NETWORK-SMOKE-RESULT: name-resolution-failed"
elif grep -Eiq 'certificate|unknownissuer|unknown issuer|invalid peer' /tmp/cold.json; then
  echo "NETWORK-SMOKE-RESULT: certificate-error"
elif grep -Eiq 'error trying to connect|connection refused|connect error|timed out|connection reset|error sending request' /tmp/cold.json; then
  echo "NETWORK-SMOKE-RESULT: connection-failed"
else
  echo "NETWORK-SMOKE-RESULT: unexpected"
fi
SH
    SUFFIX="$$-$(od -An -N3 -tx1 /dev/urandom | tr -d ' \n')"
    NET="canon-smoke-net-$SUFFIX"
    PGC="canon-smoke-pg-$SUFFIX"
    # The password lives for this run only; nothing real is used.
    PGPASS=$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n')
    cleanup_network() {
      docker rm -f "$PGC" >/dev/null 2>&1 || true
      docker network rm "$NET" >/dev/null 2>&1 || true
      cleanup_work
    }
    trap cleanup_network EXIT
    echo "release-smoke network: $BINARY in $IMAGE, postgres $POSTGRES_IMAGE, S3 $S3_ENDPOINT"
    docker network create "$NET" >/dev/null
    # `--network-alias postgres`: the binary connects to the HOST NAME `postgres` (musl's resolver), never to an IP.
    docker run -d --name "$PGC" --network "$NET" --network-alias postgres \
      -e POSTGRES_USER=canon -e "POSTGRES_PASSWORD=$PGPASS" -e POSTGRES_DB=canon_v1 \
      "$POSTGRES_IMAGE" >/dev/null
    ready=""
    for _ in $(seq 1 60); do
      if docker exec "$PGC" pg_isready -h 127.0.0.1 -U canon -d canon_v1 >/dev/null 2>&1; then ready=1; break; fi
      sleep 1
    done
    [ -n "$ready" ] || { docker logs "$PGC" >&2 || true; die "the postgres container did not become ready in 60 s"; }
    LOG="$WORK/network.log"
    if ! docker run --rm --network "$NET" \
      -e NEED_CA=1 -e "BIN_NAME=$BIN_NAME" \
      -e "CANON_PG_DSN=postgres://canon:$PGPASS@postgres:5432/canon_v1" \
      -e "CANON_S3_ENDPOINT=$S3_ENDPOINT" \
      -e CANON_S3_ACCESS_KEY=AKIAFAKEFAKEFAKEFAKE \
      -e CANON_S3_SECRET_KEY=fakefakefakefakefakefakefakefakefake0000 \
      -e CANON_R2_BUCKET=canon-smoke-no-such-bucket-0f3a9c \
      -v "$BIN_DIR:/canon-bin:ro" -v "$REPO:/repo" -v "$WORK:/smoke:ro" -w /repo \
      "$IMAGE" sh /smoke/inner.sh 2>&1 | tee "$LOG" | sed -e "s/$PGPASS/<password>/g"; then
      die "the container of the network smoke failed before the cold tier answered (the output above)"
    fi
    result=$(sed -n 's/^NETWORK-SMOKE-RESULT: //p' "$LOG" | tail -n 1)
    case "$result" in
      hot-tier-failed) die "the hot tier did not attach through the host name postgres (the query above failed)" ;;
      s3-rejected-authentication) ;;
      name-resolution-failed) die "the S3 endpoint's name did not resolve" ;;
      certificate-error) die "the S3 endpoint's certificate was rejected: the TLS trust store cannot verify it" ;;
      connection-failed) die "the request to the S3 endpoint failed before any answer: name resolution, connection or certificate (a host with an untrusted certificate ends here: the client's error does not say which)" ;;
      *) die "the cold tier answered neither an authentication rejection nor a known network failure (the output above)" ;;
    esac
    # The schema the hot tier creates on connect must be in the database the binary reached by host name.
    table=$(docker exec "$PGC" psql -U canon -d canon_v1 -tAc "select to_regclass('canon_v1.records_history')")
    [ "$table" = "canon_v1.records_history" ] || die "the hot tier answered but the schema was not created in postgres (to_regclass: '$table')"
    echo "release-smoke network: OK (postgres attached by host name and its schema exists; S3 over TLS rejected the fake key)"
    ;;

  *)
    echo "release-smoke: unknown smoke $MODE" >&2
    usage
    ;;
esac
