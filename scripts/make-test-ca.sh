#!/usr/bin/env bash
# Generate a throwaway self-signed CA certificate for exercising `lcm` during
# development. Writes to ./scratch by default.
#
#   ./scripts/make-test-ca.sh            # -> scratch/test-ca.crt (+ .key)
#   ./scripts/make-test-ca.sh my-ca out  # -> out/my-ca.crt (+ .key)
set -euo pipefail

name="${1:-test-ca}"
outdir="${2:-scratch}"
mkdir -p "$outdir"

openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
    -nodes -days 365 \
    -keyout "$outdir/$name.key" \
    -out "$outdir/$name.crt" \
    -subj "/CN=LCM Test Root CA ($name)/O=LCM Dev" \
    -addext "basicConstraints=critical,CA:TRUE" \
    -addext "keyUsage=critical,keyCertSign,cRLSign"

echo "Wrote $outdir/$name.crt and $outdir/$name.key"
