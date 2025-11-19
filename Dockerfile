FROM node:24-slim AS frontend-builder

WORKDIR /app/frontend

# install pnpm
RUN npm install -g pnpm@10.11.0

# copy frontend source
COPY frontend/package.json frontend/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile

COPY frontend/ ./
COPY lex/ ../lex/

# build frontend
RUN pnpm build

# rust builder stage
FROM rust:1-bullseye AS rust-builder

WORKDIR /app

# install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY server/ ./server/
COPY jacquard-oatproxy/ ./jacquard-oatproxy/
COPY lexicons/ ./lexicons/

RUN cargo build --release --bin server

# final stage
FROM debian:bullseye

WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# copy server binary
COPY --from=rust-builder /app/target/release/server /usr/local/bin/server

# copy frontend dist
COPY --from=frontend-builder /app/frontend/dist /app/dist

# set environment variables
ENV STATIC_DIR=/app/dist
ENV DATABASE_URL=sqlite:/data/istat.db
ENV BIND_ADDR=0.0.0.0:8080

# create data directory
RUN mkdir -p /data

EXPOSE 8080

VOLUME ["/data"]

CMD ["/usr/local/bin/server"]
