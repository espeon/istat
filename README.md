# istat

an ATProto application with OAuth 2.1 proxy support for extended session management.

## what is this?

istat provides an OAuth proxy layer between ATProto clients and Personal Data Servers (PDSes), enabling:

- **longer-lived sessions** - 1 year tokens upstream vs 1 week without proxy
- **confidential client mode** - secure OAuth flow with client secrets
- **transparent proxying** - drop-in replacement for direct PDS communication

## architecture

three main components:

1. **jacquard-oatproxy** - reusable Rust library implementing OAuth 2.1 authorization server/proxy
2. **server** - backend running the proxy and XRPC endpoints
3. **frontend** - React/TypeScript UI with TanStack Router

the proxy acts as both OAuth server (downstream to clients) and OAuth client (upstream to PDSes), handling DPoP proof generation and token refresh automatically.

## quick start

### server

```bash
# run development server
cargo run --bin server

# build release
cargo build --release --bin server

# server runs on port 8080 by default
# database: sqlite://istat.db
```

### frontend

```bash
# install dependencies
pnpm install

# run dev server (port 3000)
pnpm dev

# build for production
pnpm build
```

## docker

multiarch images (x86_64 + arm64) published to GitHub Container Registry:

```bash
# latest release
docker pull ghcr.io/<username>/istat:latest

# specific version
docker pull ghcr.io/<username>/istat:v1.0.0

# latest main branch
docker pull ghcr.io/<username>/istat:main-<sha>
```

run with frontend:

```bash
docker run -p 8080:8080 -v $(pwd)/data:/data ghcr.io/<username>/istat:latest
```

run API-only (frontend served separately):

```bash
docker run -p 8080:8080 -v $(pwd)/data:/data \
  -e ISTAT_DISABLE_FRONTEND=true \
  ghcr.io/<username>/istat:latest
```

### environment variables

- `DATABASE_URL` - database connection string (default: `sqlite:/data/istat.db`)
- `PUBLIC_URL` - public-facing URL for OAuth redirects (default: `http://localhost:3000`)
- `BIND_ADDR` - address to bind server (default: `0.0.0.0:8080`)
- `DEV_MODE` - proxy to Vite dev server on localhost:3001 (default: `false`)
- `ISTAT_DISABLE_FRONTEND` - disable frontend serving, API/OAuth only (default: `false`)
- `STATIC_DIR` - directory to serve static files from (default: `dist`)

## development

### working with lexicons

lexicon schemas live in `lex/`. after modifying:

```bash
# backend (regenerate Rust types)
jacquard-codegen --input lex --output lexicons

# frontend (regenerate TypeScript types)
pnpm codegen
```

### database migrations

migrations in `server/migrations/` run automatically on startup (001, 002, 003...). SQLite backend with default path `sqlite://istat.db`.

### testing

```bash
# rust
cargo test

# frontend
pnpm test
```

## oauth flow

1. client initiates OAuth with proxy
2. proxy performs PAR with upstream PDS using confidential credentials
3. user authorizes at PDS
4. proxy exchanges code for long-lived upstream tokens (1 year)
5. proxy issues short-lived JWTs to client (1 hour)
6. client makes XRPC requests with JWT
7. proxy validates JWT, creates DPoP proof, forwards to PDS

## license

dual licensed under MIT or Apache 2.0
