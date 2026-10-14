FROM node:24-alpine AS web-build
WORKDIR /src
COPY package.json package-lock.json ./
RUN npm ci
COPY svelte.config.js vite.config.ts tsconfig.json ./
COPY src ./src
RUN npm run build

FROM golang:1.25-alpine AS go-build
WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY . .
COPY --from=web-build /src/web/build ./web/build
RUN CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o /out/learnvault .

FROM alpine:3.22
RUN apk add --no-cache ca-certificates wget && addgroup -S -g 10001 learnvault && adduser -S -D -H -u 10001 -G learnvault learnvault
WORKDIR /app
COPY --from=go-build --chown=10001:10001 /out/learnvault /app/learnvault
ENV LISTEN_ADDR=0.0.0.0 PORT=8090 DATA_DIR=/data CACHE_DIR=/cache LIBRARY_DIR=/library COOKIE_SECURE=true
USER 10001:10001
EXPOSE 8090
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 CMD wget -q -O - http://127.0.0.1:${PORT}/api/healthz >/dev/null || exit 1
ENTRYPOINT ["/app/learnvault"]
