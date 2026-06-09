# zpic HTTP Server API Specification

Version: 0.1.0

## Base URL

```
http://127.0.0.1:36677
```

Default port: `36677` (PicGo compatible)

## Authentication

None required for localhost deployment. Future versions may add API key support for remote access.

## Endpoints

### POST /upload

Upload one or more images to the configured uploader.

#### Request Format 1: JSON Path List (Desktop)

For local files already on disk:

```http
POST /upload HTTP/1.1
Content-Type: application/json
Host: 127.0.0.1:36677

{
  "list": [
    "/Users/user/Pictures/image1.png",
    "/Users/user/Pictures/image2.jpg"
  ]
}
```

**Request Body:**
- `list` (string[]): Array of absolute file paths to upload

#### Request Format 2: Multipart File Upload (Mobile/Clipboard)

For clipboard images or files not yet on disk:

```http
POST /upload HTTP/1.1
Content-Type: multipart/form-data; boundary=----WebKitFormBoundary7MA4YWxkTrZu0gW
Host: 127.0.0.1:36677

------WebKitFormBoundary7MA4YWxkTrZu0gW
Content-Disposition: form-data; name="list"; filename="1718123456.png"
Content-Type: image/png

<binary image data>
------WebKitFormBoundary7MA4YWxkTrZu0gW--
```

**Form Fields:**
- `list` (file): One or more image files

#### Success Response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "success": true,
  "result": [
    "https://cdn.example.com/image1.png",
    "https://cdn.example.com/image2.jpg"
  ],
  "fullResult": [
    {
      "imgUrl": "https://cdn.example.com/image1.png",
      "delete": "https://cdn.example.com/delete/abc123"
    },
    {
      "imgUrl": "https://cdn.example.com/image2.jpg",
      "delete": "https://cdn.example.com/delete/def456"
    }
  ]
}
```

**Response Fields:**
- `success` (boolean): Always `true` on success
- `result` (string[]): Array of uploaded image URLs
- `fullResult` (object[], optional): Extended metadata including delete URLs if supported by uploader

#### Error Response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "success": false,
  "msg": "Upload failed: invalid image format",
  "code": "INVALID_FILE_TYPE"
}
```

**Response Fields:**
- `success` (boolean): Always `false` on error
- `msg` (string): Human-readable error message
- `code` (string, optional): Machine-readable error code for programmatic handling

#### Error Codes

| Code | Description |
|------|-------------|
| `INVALID_FILE_TYPE` | Unsupported image format (not PNG, JPG, GIF, etc.) |
| `FILE_NOT_FOUND` | File path in JSON request does not exist |
| `UPLOAD_FAILED` | Uploader returned an error (network, auth, etc.) |
| `CONFIG_ERROR` | zpic configuration is missing or invalid |
| `SERVER_ERROR` | Internal server error |

#### Supported Image Formats

- PNG (`.png`)
- JPEG (`.jpg`, `.jpeg`)
- GIF (`.gif`)
- WebP (`.webp`)
- BMP (`.bmp`)
- TIFF (`.tiff`, `.tif`)
- SVG (`.svg`)
- AVIF (`.avif`)

---

### GET /health

Health check endpoint.

#### Request

```http
GET /health HTTP/1.1
Host: 127.0.0.1:36677
```

#### Success Response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "status": "ok",
  "version": "0.1.0",
  "uptime": 3600
}
```

**Response Fields:**
- `status` (string): Always `"ok"` if server is running
- `version` (string): zpic version number
- `uptime` (number): Server uptime in seconds

---

### GET /config

Get current zpic configuration overview (non-sensitive data only).

#### Request

```http
GET /config HTTP/1.1
Host: 127.0.0.1:36677
```

#### Success Response

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "currentUploader": "github",
  "uploaders": ["github", "local", "s3", "oss"],
  "version": "0.1.0"
}
```

**Response Fields:**
- `currentUploader` (string): Currently active uploader type
- `uploaders` (string[]): List of available uploader types
- `version` (string): zpic version number

**Security Note:** This endpoint does NOT expose secrets (tokens, passwords, API keys). Only metadata is returned.

---

## CORS Configuration

The server allows cross-origin requests from any origin with the following configuration:

- **Allowed Origins:** `*` (any)
- **Allowed Methods:** `GET`, `POST`
- **Allowed Headers:** `*` (any)

This permissive configuration is safe because the server binds to localhost only (`127.0.0.1`).

---

## Rate Limiting

No rate limiting is applied in the initial version. Future versions may add rate limiting for remote deployment scenarios.

---

## Timeout

Default request timeout: **60 seconds**

For large image uploads, consider increasing the timeout in the Obsidian plugin settings.

---

## Examples

### Example 1: Upload local file (desktop)

```bash
curl -X POST http://127.0.0.1:36677/upload \
  -H "Content-Type: application/json" \
  -d '{"list": ["/Users/user/Desktop/screenshot.png"]}'
```

Response:
```json
{
  "success": true,
  "result": ["https://raw.githubusercontent.com/user/repo/main/screenshot.png"]
}
```

### Example 2: Upload file via multipart (mobile/clipboard)

```bash
curl -X POST http://127.0.0.1:36677/upload \
  -F "list=@/Users/user/Desktop/image.png"
```

Response:
```json
{
  "success": true,
  "result": ["https://cdn.example.com/image.png"]
}
```

### Example 3: Health check

```bash
curl http://127.0.0.1:36677/health
```

Response:
```json
{
  "status": "ok",
  "version": "0.1.0",
  "uptime": 7200
}
```

### Example 4: Get configuration

```bash
curl http://127.0.0.1:36677/config
```

Response:
```json
{
  "currentUploader": "github",
  "uploaders": ["github", "local", "s3", "oss"],
  "version": "0.1.0"
}
```

---

## Error Handling Best Practices

### For Clients (Obsidian Plugin)

1. **Check server availability** with `/health` before attempting uploads.
2. **Parse error responses** and show actionable messages to users.
3. **Implement timeout handling** for slow uploads or network issues.
4. **Retry transient failures** (network errors) but not permanent failures (auth errors).
5. **Log errors** with context for debugging.

### Common Error Scenarios

| Scenario | Error | User Action |
|----------|-------|-------------|
| Server not running | Connection refused | Start zpic server with `zpic server start` |
| Invalid image format | `INVALID_FILE_TYPE` | Only upload supported image types |
| Uploader auth failure | `UPLOAD_FAILED` | Check zpic configuration with `zpic config show` |
| File too large | `UPLOAD_FAILED` | Check uploader's file size limits |
| Network timeout | Connection timeout | Increase timeout in plugin settings |

---

## Versioning

API version is tied to zpic version. Breaking changes will be documented in release notes.

Current API version: **v0.1.0**

Future versions may introduce:
- API key authentication (`Authorization` header)
- WebSocket support for real-time progress
- Batch upload optimization
- Image compression before upload
