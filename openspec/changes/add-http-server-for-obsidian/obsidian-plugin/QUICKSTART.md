# Obsidian Plugin Quick Start Guide

This guide walks you through setting up the zpic Obsidian plugin for automatic image uploads.

## Prerequisites

1. **Install zpic CLI**

   ```bash
   # From crates.io (after release)
   cargo install zpic
   
   # Or from GitHub
   cargo install --git https://github.com/xtcel/zpic zpic
   ```

2. **Configure zpic uploader**

   ```bash
   # Initialize configuration
   zpic config init
   
   # Set up your preferred uploader (example: GitHub)
   zpic set uploader github MyBlog \
     --field repo=username/image-repo \
     --field branch=main \
     --field token=ghp_your_token_here
   
   # Verify configuration
   zpic config show
   ```

## Installation

### Method 1: Manual Installation (Recommended for testing)

1. Download the latest release from [GitHub Releases](https://github.com/xtcel/obsidian-zpic-plugin/releases)
2. Extract the zip file
3. Copy `main.js`, `manifest.json`, and `styles.css` to your Obsidian vault:
   ```
   YourVault/.obsidian/plugins/zpic/
   ```
4. Restart Obsidian
5. Go to Settings → Community plugins
6. Enable "Zpic Image Upload"

### Method 2: Obsidian Community Plugins (Coming soon)

1. Open Settings → Community plugins
2. Click "Browse"
3. Search for "Zpic"
4. Click "Install" then "Enable"

## Configuration

### 1. Start zpic server

Open a terminal and run:

```bash
zpic server start
```

You should see:
```
✓ zpic server listening on http://127.0.0.1:36677
✓ Using uploader: github (MyBlog)
```

**Keep this terminal open** or set up auto-start (see below).

### 2. Configure plugin settings

1. Open Obsidian Settings
2. Go to Community plugins → Zpic Image Upload
3. Configure settings:

   | Setting | Default | Description |
   |---------|---------|-------------|
   | Server URL | `http://127.0.0.1:36677` | zpic server address |
   | Upload on paste | ✓ | Auto-upload when pasting images |
   | Upload on drop | ✓ | Auto-upload when dragging images |
   | Image description | origin | Use original filename in markdown |
   | Delete local after upload | ✗ | Remove local file after successful upload |
   | Timeout | 30000ms | Upload request timeout |

4. Click "Save"

## Usage

### Paste Image

1. Copy an image to clipboard (screenshot, browser image, etc.)
2. Paste in Obsidian editor (`Cmd+V` / `Ctrl+V`)
3. Plugin automatically uploads and inserts markdown link

**Before:**
```
<cursor>
```

**After paste:**
```markdown
![Uploading...abc123]()
```

**After upload:**
```markdown
![screenshot.png](https://cdn.example.com/screenshot.png)
```

### Drag and Drop

1. Drag an image file from Finder/Explorer
2. Drop into Obsidian editor
3. Plugin automatically uploads and inserts markdown link

**Tip:** Hold `Ctrl/Cmd` while dropping to preserve local file behavior (skip upload).

### Control via Frontmatter (Optional)

Disable auto-upload for specific notes:

```yaml
---
zpic-upload: false
---

# My Note

Images pasted here won't auto-upload
```

## Auto-Start zpic Server

### macOS (launchd)

```bash
# Install auto-start service
zpic server install

# Check status
zpic server status

# Uninstall (if needed)
zpic server uninstall
```

The server will now start automatically when you log in.

### Linux (systemd)

```bash
# Install auto-start service
zpic server install

# Check status
zpic server status

# Manually start/stop
systemctl --user start zpic-server
systemctl --user stop zpic-server

# Uninstall (if needed)
zpic server uninstall
```

### Windows

Windows service support is coming soon. For now, use Task Scheduler:

1. Open Task Scheduler
2. Create Basic Task
3. Trigger: "When I log on"
4. Action: Start a program
5. Program: `C:\path\to\zpic.exe`
6. Arguments: `server start`

## Troubleshooting

### Error: "Could not connect to zpic server"

**Cause:** zpic server is not running.

**Solution:**
1. Start the server: `zpic server start`
2. Verify it's running: `curl http://127.0.0.1:36677/health`
3. Check plugin settings has correct Server URL

---

### Error: "Upload failed: authentication error"

**Cause:** Uploader credentials are invalid or expired.

**Solution:**
1. Check configuration: `zpic config show`
2. Update credentials: `zpic set uploader github MyBlog --field token=NEW_TOKEN`
3. Test upload: `zpic upload test.png`

---

### Error: "Upload timeout"

**Cause:** Large image or slow network.

**Solution:**
1. Increase timeout in plugin settings (e.g., 60000ms)
2. Check network connection
3. Try uploading a smaller image first

---

### Images paste as local files instead of uploading

**Cause:** Plugin settings or frontmatter disabled auto-upload.

**Solution:**
1. Check Settings → Zpic → "Upload on paste" is enabled
2. Check note frontmatter doesn't have `zpic-upload: false`
3. Verify server is running

---

### Server crashes or stops unexpectedly

**Cause:** Configuration error or disk space issues.

**Solution:**
1. Check logs: `zpic server start` (see error output)
2. Verify configuration: `zpic doctor`
3. Check disk space for temp directory
4. Report issue on GitHub with logs

---

### Mobile (iOS/Android) upload not working

**Cause:** Server URL unreachable from mobile device.

**Solution:**
1. Ensure server is running on the same network
2. Use computer's local IP instead of 127.0.0.1
3. Example: `http://192.168.1.100:36677`
4. Ensure firewall allows port 36677

**Note:** For security, only expose on trusted networks.

---

## Advanced Usage

### Multiple Uploader Profiles

Switch between different upload configurations:

```bash
# List available uploaders
zpic uploader list

# Switch to different profile
zpic use uploader github WorkBlog

# Restart server to apply changes
zpic server stop
zpic server start
```

### Custom Server Port

```bash
# Start on custom port
zpic server start --port 8080

# Update plugin settings
# Server URL: http://127.0.0.1:8080
```

### Debug Mode

```bash
# Start server with verbose logging
RUST_LOG=debug zpic server start
```

Check logs for detailed request/response information.

---

## Uninstallation

### Remove Plugin

1. Go to Settings → Community plugins
2. Find "Zpic Image Upload"
3. Click "Uninstall"
4. Delete plugin folder: `.obsidian/plugins/zpic/`

### Remove Server Auto-Start

```bash
zpic server uninstall
```

### Remove zpic CLI (optional)

```bash
cargo uninstall zpic
```

---

## Support

- **Documentation:** [zpic GitHub](https://github.com/xtcel/zpic)
- **Issues:** [Report a bug](https://github.com/xtcel/zpic/issues)
- **Discussions:** [GitHub Discussions](https://github.com/xtcel/zpic/discussions)

---

## License

MIT License - see LICENSE file for details
