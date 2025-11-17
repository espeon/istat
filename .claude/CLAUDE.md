# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

istat is an ATProto application with three main components:

1. **jacquard-oatproxy** - Rust library implementing an OAuth 2.1 authorization server/proxy for ATProto
2. **server** - Backend Rust server that runs the oatproxy and provides XRPC endpoints
3. **frontend** - React/TypeScript frontend using TanStack Router

The core innovation is the OAuth proxy that sits between ATProto clients and Personal Data Servers (PDSes), enabling confidential client mode with longer session times (1 year vs 1 week).

## Architecture

### jacquard-oatproxy Library

Located in `jacquard-oatproxy/`, this is a reusable OAuth proxy implementation:

- **auth.rs** - JWT validation and bearer token extraction
- **server.rs** - Main OAuth server implementation with PAR, authorize, token, and revoke endpoints
- **token.rs** - Token issuance and DPoP proof generation
- **session.rs** - Session state management
- **store.rs** - Storage traits for sessions, keys, and nonces
- **handlers.rs** - Optional Axum HTTP handlers (behind `axum` feature flag)

The proxy acts as both an OAuth server (downstream to clients) and OAuth client (upstream to PDSes). It handles DPoP proof generation, token refresh, and request proxying transparently.

### Server Application

Located in `server/src/`:

- **main.rs** - Entry point, database initialization, router assembly
- **oauth.rs** - OAuth flow handling with SQLite-backed stores
- **oatproxy/** - Integration of jacquard-oatproxy with the server
- **xrpc/** - XRPC endpoint implementations
- **jetstream.rs** - Jetstream event ingestion for real-time data

Database migrations are in `server/migrations/` and run sequentially on startup.

### Frontend Application

Located in `frontend/`, built with:

- TanStack Router for file-based routing (routes in `src/routes/`)
- Atcute SDK for ATProto communication
- Tailwind CSS for styling
- Vitest for testing

The frontend uses a custom `ProxyIdentityResolver` to rewrite PDS endpoints to point to the OAuth proxy.

### Lexicons

Located in `lexicons/`, this workspace member contains generated ATProto lexicon types. The lexicon JSON definitions are in `lex/` at the project root. These are auto-generated - manual edits will be overwritten.

## Common Commands

### Rust Server

```bash
# Run the server (development)
cargo run --bin server

# Build the server
cargo build --bin server

# Build release
cargo build --release --bin server

# Run tests
cargo test

# Check without building
cargo check

# Check the oatproxy library specifically
cargo check -p jacquard-oatproxy

# Run oatproxy example
cargo run --example simple_server
```

### Frontend

```bash
# Install dependencies
pnpm install

# Run development server (on port 3000)
pnpm dev

# Build for production
pnpm build

# Run tests
pnpm test

# Generate lexicon types from JSON schemas
pnpm codegen
```

### Database

The server uses SQLite with migrations in `server/migrations/`. Migrations are applied automatically on startup in numerical order (001, 002, 003, etc.).

Database URL defaults to `sqlite://istat.db` in the current directory.

## Development Notes

### Working with Lexicons

Lexicon schemas are defined as JSON files in `lex/`. After modifying lexicons:

1. Backend: Run `jacquard-codegen --input lex --output lexicons` to regenerate Rust types
2. Frontend: Run `pnpm codegen` to regenerate TypeScript types

Never manually edit generated files in `lexicons/` or frontend lexicon outputs.

### OAuth Proxy Flow

When debugging OAuth issues, understand the flow:

1. Client initiates OAuth with proxy
2. Proxy performs PAR with upstream PDS using confidential client credentials
3. User authorizes at PDS
4. Proxy exchanges code for long-lived upstream tokens (1 year)
5. Proxy issues short-lived JWTs to client (1 hour)
6. Client makes XRPC requests with JWT
7. Proxy validates JWT, looks up upstream session, creates DPoP proof, forwards to PDS

Key files: `jacquard-oatproxy/src/server.rs` for OAuth endpoints, `jacquard-oatproxy/src/token.rs` for DPoP proofs.

### Adding XRPC Endpoints

1. Define lexicon in `lex/` directory
2. Regenerate types (`pnpm codegen` for frontend, `cargo build` for backend)
3. Implement handler in `server/src/xrpc/`
4. Wire up in `server/src/main.rs` router

### Jetstream Integration

The server uses the `rocketman` crate to consume Jetstream events. Event handlers are implemented as `LexiconIngestor` traits in `server/src/jetstream.rs`. This allows real-time indexing of ATProto records.

## Dependencies

Key Rust dependencies:

- `jacquard-*` - ATProto utilities (from tangled.org git repo)
- `axum` - Web framework
- `sqlx` - Database with SQLite
- `rocketman` - Jetstream client
- `tokio` - Async runtime
- `jose-jwk`, `jose-jws` - JWT/JWK handling
- `dpop-verifier` - DPoP proof validation

Key frontend dependencies:

- `@tanstack/react-router` - File-based routing
- `@atcute/*` - ATProto client libraries
- `tailwindcss` - Styling

## Testing

Rust: `cargo test`
Frontend: `pnpm test`

The oatproxy library has example usage in `jacquard-oatproxy/examples/simple_server/`.



    <frontend_aesthetics>
    You tend to converge toward generic, “on distribution” outputs. In frontend design, this creates what users call the “AI slop” aesthetic. Avoid this: make creative, distinctive frontends that surprise and delight. Focus on:

    Typography: Choose fonts that are beautiful, unique, and interesting. Avoid generic fonts like Arial and Inter; opt instead for distinctive choices that elevate the frontend’s aesthetics.

    Color & Theme: Commit to a cohesive aesthetic. Use CSS variables for consistency. Dominant colors with sharp accents outperform timid, evenly-distributed palettes. Draw from IDE themes and cultural aesthetics for inspiration.

    Motion: Use animations for effects and micro-interactions. Prioritize CSS-only solutions for HTML. Use Motion library for React when available. Focus on high-impact moments: one well-orchestrated page load with staggered reveals (animation-delay) creates more delight than scattered micro-interactions.

    Backgrounds: Solid colors can go a long way. Consider subtle gradients, patterns, or textures to add depth without overwhelming content. Be as simple as possible while still being visually engaging.

    Avoid generic AI-generated aesthetics:

        Overused font families (Inter, Roboto, Arial, system fonts)
        Clichéd color schemes (particularly purple gradients on white backgrounds)
        Predictable layouts and component patterns
        Cookie-cutter design that lacks context-specific character

    Interpret creatively and make unexpected choices that feel genuinely designed for the context. Vary between light and dark themes, different fonts, different aesthetics. You still tend to converge on common choices (Space Grotesk, for example) across generations. Avoid this: it is critical that you think outside the box!
    </frontend_aesthetics>
