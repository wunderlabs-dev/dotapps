FROM node:20-alpine

# Install pnpm
RUN corepack enable && corepack prepare pnpm@latest --activate

# Install common build dependencies
RUN apk add --no-cache git python3 make g++

# Set working directory
WORKDIR /app

# Default command (will be overridden)
CMD ["sh"]
