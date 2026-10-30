# Frost backend API (contract used by `cosmetics.rs`)

The launcher only needs a small HTTPS service. It is not implemented in this repository yet; this is the contract.
Base URL is configurable (`Settings.frost_api`).

## Authentication (Mojang session handshake)

1. `POST /v1/auth/challenge`  body `{ "uuid": "<undashed uuid>" }` -> `{ "server_id": "<random 32+ hex>" }`
   Server stores `server_id` for ~60 s.
2. Client calls Mojang `POST https://sessionserver.mojang.com/session/minecraft/join`
   with `{accessToken, selectedProfile, serverId}`.
3. `POST /v1/auth/login`  body `{ "username": "...", "server_id": "..." }`
   Server verifies with `GET https://sessionserver.mojang.com/session/minecraft/hasJoined?username=…&serverId=…`,
   checks the returned id matches, and answers `{ "token": "<opaque session token>" }`.

All following calls send `Authorization: Bearer <token>`.

## Cape

* `PUT /v1/cape` multipart field `file` (PNG, ≤ 1 MiB, 64x32 / 22x17 or integer multiples). Re-validate server side, re-encode, store under content hash.
* `DELETE /v1/cape`

## Presence + lookup (tab snowflake, cape visibility)

* `POST /v1/presence` – heartbeat every 60 s; entry expires after ~150 s.
* `POST /v1/cosmetics/lookup` body `{ "uuids": ["…"] }` (≤ 100) ->
  ```json
  { "<uuid>": { "frost": true, "cape_url": "https://cdn…/<hash>.png", "cape_hash": "<hash>" } }
  ```
  Only users with a live heartbeat get `frost: true`.
  No authentication needed, rate-limit per IP.

Players without a Frost heartbeat never receive a `cape_url`, which is what keeps capes Frost-exclusive.
