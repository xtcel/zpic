# zpic HTTP server

`zpic` can run a local HTTP server that exposes its upload pipeline
through a PicGo-compatible API. The primary consumer is the
[Obsidian plugin](../extensions/obsidian/), but any client that
speaks the same wire format (curl, custom scripts, ...) can drive
uploads the same way.

The server binds to `127.0.0.1` by default so it never opens the
upload pipeline to the wider network. Run it on a trusted host only.

## Quick start

```bash
# Configure a uploader (one-time)
zpic config init
zpic set uploader github MyBlog \
  --field repo=user/images \
  --field branch=main \
  --field token=$GITHUB_TOKEN

# Start the server in the foreground
zpic server start

# In a different shell, verify it's up
curl http://127.0.0.1:36677/health
```

By default the server binds to `127.0.0.1:36677` — the same address
the PicGo and PicList clients target.

## Endpoints

| Method | Path     | Purpose                            |
|--------|----------|------------------------------------|
| GET    | `/health` | Liveness probe (status, version, uptime) |
| GET    | `/config` | Active uploader + available types (no secrets) |
| POST   | `/upload` | Upload one or more images (JSON or multipart) |

### `POST /upload`

The upload endpoint accepts two content types:

#### 1. JSON path list (desktop)

```http
POST /upload
Content-Type: application/json

{ "list": ["/path/to/image1.png", "/path/to/image2.jpg"] }
```

Used when the client already has file paths on the local filesystem
(desktop Obsidian, scripts that know the path layout).

#### 2. Multipart form data (mobile, clipboard)

```http
POST /upload
Content-Type: multipart/form-data; boundary=----abc

------abc
Content-Disposition: form-data; name="list"; filename="clipboard.png"
Content-Type: image/png

<binary image data>
------abc--
```

Used by mobile Obsidian and any client that holds the bytes
in-memory rather than on disk. The server writes the part to a temp
file under the system temp dir, runs the standard pipeline, and
deletes the temp file when the upload finishes (success or failure).

#### Upload size limits

- HTTP request body limit: `1 GiB`.
- Single multipart file limit: `512 MiB`.

If either limit is exceeded, the server rejects the request with a
payload-too-large style error (typically `HTTP 413` or a structured
`success: false` response, depending on which layer triggers first).

#### Response

```json
{
  "success": true,
  "result": ["https://cdn.example.com/image.png"],
  "fullResult": [
    { "imgUrl": "https://cdn.example.com/image.png" }
  ]
}
```

`result` lists the public URLs in the same order as the request.
`fullResult` carries the same URLs with optional `delete` tokens
when the active uploader exposes them (e.g. S3, GitHub).

Errors are returned with HTTP 200 and a `success: false` body, in
the same shape PicGo uses. See
[`specs/api-specification.md`](../openspec/changes/add-http-server-for-obsidian/specs/api-specification.md)
for the full error-code list.

## Configuration

```bash
# Default values
zpic server start

# Custom bind
zpic server start --host 0.0.0.0 --port 8080

# Explicit config (overrides the standard search path)
zpic server start --config /path/to/config.toml
```

`--host` and `--port` are global flags, so they can also be placed
before the `start` subcommand: `zpic server --host 0.0.0.0 start` is
equivalent.

When the server is bound to `0.0.0.0` (all interfaces), the startup
banner additionally prints every local IPv4 address that is
plausibly reachable from another device on the same network — RFC
1918 (10/8, 172.16/12, 192.168/16) and public IPs. Loopback,
link-local (169.254/16), CGNAT (100.64/10), multicast, and
benchmarking ranges are filtered out as not useful for this purpose.

```text
✓ zpic server listening on http://0.0.0.0:36677
  ➜  Local:   http://127.0.0.1:36677/
  ➜  Network: http://192.168.1.42:36677/
  uploader: github (MyBlog)
  Press Ctrl+C to stop.
```

On a multi-homed host (e.g. WiFi + ethernet) you may see more than
one `Network:` line — one per reachable interface. Pick the one that
matches the network your phone or laptop is on.

When the server is bound to a specific address (the default
`127.0.0.1`, or a particular interface IP), the banner stays compact
and only prints the address you asked for.

The server reads the same zpic config the `upload` and `migrate`
commands use, so the active uploader and the rename template are
shared with the rest of the CLI.

## Security

- The server binds to `127.0.0.1` by default. Expose it to a
  wider network only on a host you control and trust.
- CORS is wide open (`*`) for both origin and headers. This is
  safe for a loopback-only deployment but should be tightened
  before exposing the server on a LAN.
- No authentication is built in. If you need to expose the server
  beyond loopback, front it with a reverse proxy that enforces
  auth and TLS.
- The `/config` endpoint intentionally omits secrets (tokens,
  passwords, API keys). Only metadata is exposed.

## See also

- [Obsidian plugin](../extensions/obsidian/) — the main client
- [API specification](../openspec/changes/add-http-server-for-obsidian/specs/api-specification.md) — wire contract
- [zpic proposal overview](../openspec/changes/add-http-server-for-obsidian/) — design and decisions
