#!/bin/bash
# Launches an MCP filesystem server scoped to ~/.opnble/ so the agent can
# read state.json, VM logs, and project clones without spawning a shell.
# Used by .cursor/mcp.json.

set -e

OPNBLE_DATA="${HOME}/.opnble"

if [[ ! -d "$OPNBLE_DATA" ]]; then
  echo "opnble data dir not found at $OPNBLE_DATA; refusing to start MCP server" >&2
  exit 1
fi

exec npx --yes @modelcontextprotocol/server-filesystem "$OPNBLE_DATA"
