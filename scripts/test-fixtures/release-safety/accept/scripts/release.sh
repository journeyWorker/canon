#!/usr/bin/env bash
set -euo pipefail
curl -fsSL -o /tmp/tool.zip https://example.invalid/tool.zip
echo "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  /tmp/tool.zip" | sha256sum -c -
unzip -q /tmp/tool.zip -d /tmp/tool
npm install -g npm@11.5.1
npm ci
