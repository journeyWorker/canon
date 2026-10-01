#!/usr/bin/env bash
set -euo pipefail
curl -fsSL -o /tmp/tool.zip https://example.invalid/tool.zip
unzip -q /tmp/tool.zip -d /tmp/tool
