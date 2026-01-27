#!/bin/bash
set -e

# Load Docker secrets into environment variables
for secret in VENICE_API_KEY OPENAI_API_KEY ANTHROPIC_API_KEY BRAINPRO_GATEWAY_TOKEN; do
    file="/run/secrets/$(echo $secret | tr '[:upper:]' '[:lower:]')"
    [ -f "$file" ] && export "$secret"="$(cat $file)"
done

# Fix permissions for brainpro user
chown -R brainpro:brainpro /app /run /var/log/supervisor 2>/dev/null || true

exec "$@"
