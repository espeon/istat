# identity ingestor requirements

## goal
track identity changes and account lifecycle events from the atproto jetstream to maintain accurate user profiles

## current state
- basic ProfileIngestor exists in server/src/jetstream.rs
- tracks app.bsky.actor.profile updates (display name, description, avatar, banner)
- fetches handle from plc.directory on profile create/update
- profiles table schema in 005_profiles_schema.sql

## needed improvements

### 1. identity event handling
listen for and process these atproto identity events:
- `#identity` events from jetstream (account-level changes)
- handle changes (via com.atproto.identity.updateHandle or plc operations)
- account status changes (active, deactivated, deleted, suspended)
- tombstone records for deleted accounts

### 2. enhanced profile tracking
- track profile deletion separately from account deletion
- detect when profiles are recreated after deletion
- handle partial updates vs full replacements
- store only current state, no history

### 3. account status tracking
add columns to profiles table:
- `account_status` TEXT: active, deactivated, deleted, suspended, tombstoned
- `account_status_updated_at` TEXT timestamp of last status change
- `last_seen_at` TEXT timestamp (updated on any activity)

### 4. implementation approach
- extend existing ProfileIngestor with identity event handling
- add new IdentityIngestor for account-level events
- implement proper error handling for PLC directory failures
- add retry logic for transient network failures
- respect rate limits when fetching from external services

## technical notes
- rocketman now supports identity events (assumed based on "updated rocketman")
- need to handle race conditions between profile and identity events
- consider caching PLC lookups to reduce external API calls
- ensure idempotent operations for event replay scenarios

## testing requirements
- unit tests for each ingestor operation
- integration tests with mock jetstream events
- test handle change propagation
- test account deactivation and reactivation flows
- test tombstone handling
