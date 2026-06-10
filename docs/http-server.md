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
