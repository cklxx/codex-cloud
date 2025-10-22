FROM node:22-alpine AS base
WORKDIR /app
RUN corepack enable

FROM base AS deps
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY cloud/frontend/package.json ./cloud/frontend/package.json
RUN pnpm install --frozen-lockfile --filter codex-cloud-frontend...

FROM base AS builder
COPY --from=deps /app/node_modules ./node_modules
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY cloud/frontend ./cloud/frontend
RUN pnpm --filter codex-cloud-frontend build

FROM base AS runner
ENV NODE_ENV=production
COPY --from=builder /app/node_modules ./node_modules
COPY --from=builder /app/pnpm-workspace.yaml ./pnpm-workspace.yaml
COPY --from=builder /app/package.json ./package.json
COPY --from=builder /app/pnpm-lock.yaml ./pnpm-lock.yaml
COPY --from=builder /app/cloud/frontend ./cloud/frontend
EXPOSE 3000
CMD ["pnpm", "--filter", "codex-cloud-frontend", "start"]
