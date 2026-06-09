# Obsidian Plugin Implementation Reference

This document provides code snippets and implementation guidance for the zpic Obsidian plugin.

## Project Structure

```
obsidian-zpic-plugin/
├── src/
│   ├── main.ts           # Plugin entry point
│   ├── settings.ts       # Settings interface and tab
│   ├── uploader.ts       # HTTP client for zpic server
│   ├── utils.ts          # Utility functions
│   └── types.ts          # TypeScript type definitions
├── manifest.json         # Plugin metadata
├── package.json          # Dependencies
├── tsconfig.json         # TypeScript configuration
├── rollup.config.js      # Build configuration
├── styles.css            # Plugin styles (optional)
└── README.md             # User documentation
```

## Core Implementation

### types.ts - Type Definitions

```typescript
export interface ZpicSettings {
  serverUrl: string;
  uploadOnPaste: boolean;
  uploadOnDrop: boolean;
  imageDesc: 'origin' | 'none';
  deleteLocalAfterUpload: boolean;
  timeout: number;
}

export interface UploadResponse {
  success: boolean;
  result?: string[];
  fullResult?: Array<{
    imgUrl: string;
    delete?: string;
  }>;
  msg?: string;
  code?: string;
}

export const DEFAULT_SETTINGS: ZpicSettings = {
  serverUrl: 'http://127.0.0.1:36677',
  uploadOnPaste: true,
  uploadOnDrop: true,
  imageDesc: 'origin',
  deleteLocalAfterUpload: false,
  timeout: 30000,
};
```

### uploader.ts - HTTP Client

```typescript
import { requestUrl, Notice } from 'obsidian';
import type { ZpicSettings, UploadResponse } from './types';

export class ZpicUploader {
  constructor(private settings: ZpicSettings) {}

  /**
   * Upload files to zpic server
   * @param files - Array of File objects or file paths
   */
  async upload(files: File[] | string[]): Promise<UploadResponse> {
    try {
      // Check if we're uploading File objects or paths
      const isFileObjects = files.length > 0 && files[0] instanceof File;

      if (isFileObjects) {
        return await this.uploadMultipart(files as File[]);
      } else {
        return await this.uploadPaths(files as string[]);
      }
    } catch (error) {
      console.error('Upload error:', error);
      return {
        success: false,
        msg: `Upload failed: ${error.message}`,
      };
    }
  }

  /**
   * Upload via JSON (for file paths)
   */
  private async uploadPaths(paths: string[]): Promise<UploadResponse> {
    const response = await requestUrl({
      url: `${this.settings.serverUrl}/upload`,
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ list: paths }),
      throw: false,
    });

    return this.handleResponse(response);
  }

  /**
   * Upload via multipart (for File objects)
   */
  private async uploadMultipart(files: File[]): Promise<UploadResponse> {
    const formData = new FormData();
    
    files.forEach((file) => {
      formData.append('list', file);
    });

    const response = await requestUrl({
      url: `${this.settings.serverUrl}/upload`,
      method: 'POST',
      body: formData,
      throw: false,
    });

    return this.handleResponse(response);
  }

  /**
   * Parse and validate response
   */
  private handleResponse(response: any): UploadResponse {
    if (response.status !== 200) {
      return {
        success: false,
        msg: `Server returned ${response.status}`,
      };
    }

    const data = response.json as UploadResponse;

    if (!data.success) {
      return {
        success: false,
        msg: data.msg || 'Upload failed',
        code: data.code,
      };
    }

    return data;
  }

  /**
   * Check if server is reachable
   */
  async checkHealth(): Promise<boolean> {
    try {
      const response = await requestUrl({
        url: `${this.settings.serverUrl}/health`,
        method: 'GET',
        throw: false,
      });

      return response.status === 200;
    } catch {
      return false;
    }
  }
}
```

### utils.ts - Utility Functions

```typescript
import { TFile } from 'obsidian';

/**
 * Check if file is an image
 */
export function isImageFile(file: File | TFile): boolean {
  const imageExtensions = [
    '.png', '.jpg', '.jpeg', '.gif', '.webp',
    '.bmp', '.tiff', '.tif', '.svg', '.avif'
  ];
  
  const fileName = file.name.toLowerCase();
  return imageExtensions.some(ext => fileName.endsWith(ext));
}

/**
 * Generate unique placeholder ID
 */
export function generatePlaceholderId(): string {
  return (Math.random() + 1).toString(36).substring(2, 7);
}

/**
 * Format image as markdown
 */
export function formatImageMarkdown(url: string, name: string, desc: 'origin' | 'none'): string {
  const altText = desc === 'origin' ? name : '';
  return `![${altText}](${url})`;
}

/**
 * Get placeholder text
 */
export function getPlaceholderText(id: string): string {
  return `![Uploading...${id}]()`;
}
```

### main.ts - Plugin Entry Point

```typescript
import {
  Editor,
  MarkdownView,
  Notice,
  Plugin,
  PluginSettingTab,
  Setting,
} from 'obsidian';
import { ZpicUploader } from './uploader';
import { DEFAULT_SETTINGS, type ZpicSettings } from './types';
import {
  isImageFile,
  generatePlaceholderId,
  formatImageMarkdown,
  getPlaceholderText,
} from './utils';

export default class ZpicPlugin extends Plugin {
  settings: ZpicSettings;
  uploader: ZpicUploader;

  async onload() {
    await this.loadSettings();
    this.uploader = new ZpicUploader(this.settings);

    // Add settings tab
    this.addSettingTab(new ZpicSettingTab(this.app, this));

    // Register paste handler
    this.registerEvent(
      this.app.workspace.on(
        'editor-paste',
        this.handlePaste.bind(this)
      )
    );

    // Register drop handler
    this.registerEvent(
      this.app.workspace.on(
        'editor-drop',
        this.handleDrop.bind(this)
      )
    );

    // Add manual upload command
    this.addCommand({
      id: 'upload-image',
      name: 'Upload image from clipboard',
      editorCallback: async (editor: Editor) => {
        // Trigger clipboard upload
        navigator.clipboard.read().then(async (items) => {
          for (const item of items) {
            for (const type of item.types) {
              if (type.startsWith('image/')) {
                const blob = await item.getType(type);
                const file = new File([blob], 'clipboard.png', { type });
                await this.uploadAndInsert(editor, [file]);
                return;
              }
            }
          }
          new Notice('No image in clipboard');
        });
      },
    });

    console.log('Zpic plugin loaded');
  }

  async loadSettings() {
    this.settings = Object.assign({}, DEFAULT_SETTINGS, await this.loadData());
  }

  async saveSettings() {
    await this.saveData(this.settings);
    this.uploader = new ZpicUploader(this.settings);
  }

  /**
   * Handle paste event
   */
  async handlePaste(
    evt: ClipboardEvent,
    editor: Editor,
    view: MarkdownView
  ) {
    if (!this.settings.uploadOnPaste) return;

    const files = evt.clipboardData?.files;
    if (!files || files.length === 0) return;

    // Check if any file is an image
    const imageFiles = Array.from(files).filter(isImageFile);
    if (imageFiles.length === 0) return;

    // Prevent default paste behavior
    evt.preventDefault();

    await this.uploadAndInsert(editor, imageFiles);
  }

  /**
   * Handle drop event
   */
  async handleDrop(
    evt: DragEvent,
    editor: Editor,
    view: MarkdownView
  ) {
    if (!this.settings.uploadOnDrop) return;

    // Skip if Ctrl/Cmd is pressed (preserve local file behavior)
    if (evt.ctrlKey || evt.metaKey) return;

    const files = evt.dataTransfer?.files;
    if (!files || files.length === 0) return;

    // Check if any file is an image
    const imageFiles = Array.from(files).filter(isImageFile);
    if (imageFiles.length === 0) return;

    // Prevent default drop behavior
    evt.preventDefault();

    await this.uploadAndInsert(editor, imageFiles);
  }

  /**
   * Upload files and insert markdown links
   */
  async uploadAndInsert(editor: Editor, files: File[]) {
    // Check server health first
    const isHealthy = await this.uploader.checkHealth();
    if (!isHealthy) {
      new Notice(
        'Cannot connect to zpic server. Please run: zpic server start',
        5000
      );
      return;
    }

    // Generate placeholder for each file
    const placeholders = files.map((file) => ({
      id: generatePlaceholderId(),
      name: file.name,
    }));

    // Insert placeholders
    const placeholderTexts = placeholders
      .map((p) => getPlaceholderText(p.id))
      .join('\n');
    editor.replaceSelection(placeholderTexts + '\n');

    try {
      // Upload files
      const response = await this.uploader.upload(files);

      if (!response.success) {
        // Replace all placeholders with error
        placeholders.forEach((p) => {
          this.replaceText(
            editor,
            getPlaceholderText(p.id),
            `⚠️ Upload failed: ${response.msg}`
          );
        });
        new Notice(`Upload failed: ${response.msg}`);
        return;
      }

      // Replace placeholders with actual URLs
      response.result?.forEach((url, index) => {
        const placeholder = placeholders[index];
        if (!placeholder) return;

        const markdown = formatImageMarkdown(
          url,
          placeholder.name,
          this.settings.imageDesc
        );

        this.replaceText(
          editor,
          getPlaceholderText(placeholder.id),
          markdown
        );
      });

      new Notice(
        `Successfully uploaded ${response.result?.length || 0} image(s)`
      );
    } catch (error) {
      console.error('Upload error:', error);
      placeholders.forEach((p) => {
        this.replaceText(
          editor,
          getPlaceholderText(p.id),
          '⚠️ Upload error'
        );
      });
      new Notice(`Upload error: ${error.message}`);
    }
  }

  /**
   * Replace first occurrence of text in editor
   */
  replaceText(editor: Editor, target: string, replacement: string) {
    const content = editor.getValue();
    const lines = content.split('\n');

    for (let i = 0; i < lines.length; i++) {
      const index = lines[i].indexOf(target);
      if (index !== -1) {
        const from = { line: i, ch: index };
        const to = { line: i, ch: index + target.length };
        editor.replaceRange(replacement, from, to);
        return;
      }
    }
  }
}

/**
 * Settings tab
 */
class ZpicSettingTab extends PluginSettingTab {
  plugin: ZpicPlugin;

  constructor(app: any, plugin: ZpicPlugin) {
    super(app, plugin);
    this.plugin = plugin;
  }

  display(): void {
    const { containerEl } = this;
    containerEl.empty();

    containerEl.createEl('h2', { text: 'Zpic Image Upload Settings' });

    new Setting(containerEl)
      .setName('Server URL')
      .setDesc('zpic server address (default: http://127.0.0.1:36677)')
      .addText((text) =>
        text
          .setPlaceholder('http://127.0.0.1:36677')
          .setValue(this.plugin.settings.serverUrl)
          .onChange(async (value) => {
            this.plugin.settings.serverUrl = value;
            await this.plugin.saveSettings();
          })
      );

    new Setting(containerEl)
      .setName('Upload on paste')
      .setDesc('Automatically upload when pasting images')
      .addToggle((toggle) =>
        toggle
          .setValue(this.plugin.settings.uploadOnPaste)
          .onChange(async (value) => {
            this.plugin.settings.uploadOnPaste = value;
            await this.plugin.saveSettings();
          })
      );

    new Setting(containerEl)
      .setName('Upload on drop')
      .setDesc('Automatically upload when dragging images into editor')
      .addToggle((toggle) =>
        toggle
          .setValue(this.plugin.settings.uploadOnDrop)
          .onChange(async (value) => {
            this.plugin.settings.uploadOnDrop = value;
            await this.plugin.saveSettings();
          })
      );

    new Setting(containerEl)
      .setName('Image description')
      .setDesc('How to generate alt text for uploaded images')
      .addDropdown((dropdown) =>
        dropdown
          .addOption('origin', 'Original filename')
          .addOption('none', 'No description')
          .setValue(this.plugin.settings.imageDesc)
          .onChange(async (value: 'origin' | 'none') => {
            this.plugin.settings.imageDesc = value;
            await this.plugin.saveSettings();
          })
      );

    new Setting(containerEl)
      .setName('Request timeout')
      .setDesc('Upload request timeout in milliseconds (default: 30000)')
      .addText((text) =>
        text
          .setPlaceholder('30000')
          .setValue(String(this.plugin.settings.timeout))
          .onChange(async (value) => {
            const timeout = parseInt(value, 10);
            if (!isNaN(timeout) && timeout > 0) {
              this.plugin.settings.timeout = timeout;
              await this.plugin.saveSettings();
            }
          })
      );
  }
}
```

## manifest.json

```json
{
  "id": "zpic-image-upload",
  "name": "Zpic Image Upload",
  "version": "0.1.0",
  "minAppVersion": "0.15.0",
  "description": "Automatically upload images to zpic server on paste and drag-and-drop",
  "author": "Zpic Team",
  "authorUrl": "https://github.com/xtcel/zpic",
  "isDesktopOnly": false
}
```

## package.json

```json
{
  "name": "obsidian-zpic-plugin",
  "version": "0.1.0",
  "description": "Zpic image upload plugin for Obsidian",
  "main": "main.js",
  "scripts": {
    "dev": "rollup -c -w",
    "build": "rollup -c"
  },
  "keywords": ["obsidian", "plugin", "image", "upload", "zpic"],
  "author": "Zpic Team",
  "license": "MIT",
  "devDependencies": {
    "@rollup/plugin-commonjs": "^25.0.0",
    "@rollup/plugin-node-resolve": "^15.0.0",
    "@rollup/plugin-typescript": "^11.0.0",
    "@types/node": "^20.0.0",
    "obsidian": "^1.4.0",
    "rollup": "^4.0.0",
    "tslib": "^2.6.0",
    "typescript": "^5.0.0"
  }
}
```

## tsconfig.json

```json
{
  "compilerOptions": {
    "baseUrl": ".",
    "inlineSourceMap": true,
    "inlineSources": true,
    "module": "ESNext",
    "target": "ES6",
    "allowJs": true,
    "noImplicitAny": true,
    "moduleResolution": "node",
    "importHelpers": true,
    "isolatedModules": true,
    "strictNullChecks": true,
    "lib": ["DOM", "ES5", "ES6", "ES7"],
    "skipLibCheck": true
  },
  "include": ["src/**/*.ts"]
}
```

## rollup.config.js

```javascript
import typescript from '@rollup/plugin-typescript';
import { nodeResolve } from '@rollup/plugin-node-resolve';
import commonjs from '@rollup/plugin-commonjs';

export default {
  input: 'src/main.ts',
  output: {
    dir: '.',
    sourcemap: 'inline',
    format: 'cjs',
    exports: 'default',
  },
  external: ['obsidian'],
  plugins: [
    typescript(),
    nodeResolve({ browser: true }),
    commonjs(),
  ],
};
```

## Development Workflow

1. **Install dependencies:**
   ```bash
   npm install
   ```

2. **Start development build:**
   ```bash
   npm run dev
   ```

3. **Link to Obsidian vault for testing:**
   ```bash
   ln -s $(pwd) /path/to/vault/.obsidian/plugins/zpic
   ```

4. **Make changes and reload plugin in Obsidian:**
   - Disable and re-enable plugin in settings
   - Or restart Obsidian

5. **Build for release:**
   ```bash
   npm run build
   ```

6. **Package for distribution:**
   ```bash
   zip -r zpic-image-upload-0.1.0.zip main.js manifest.json styles.css
   ```

## Testing Checklist

- [ ] Paste image from clipboard
- [ ] Drag image file into editor
- [ ] Multiple images in one paste/drop
- [ ] Error handling when server is down
- [ ] Error handling for upload failure
- [ ] Settings changes take effect
- [ ] Server URL validation
- [ ] Timeout handling for large uploads
- [ ] Mobile platform (iOS/Android) if applicable
- [ ] Placeholder replacement works correctly
- [ ] No duplicate uploads

## Deployment

### GitHub Release

1. Tag version: `git tag v0.1.0`
2. Push tag: `git push --tags`
3. Build release: `npm run build`
4. Create GitHub Release with `main.js`, `manifest.json`, `styles.css`
5. Write release notes

### Obsidian Community Plugins

Follow [Obsidian plugin submission guide](https://docs.obsidian.md/Plugins/Releasing/Submit+your+plugin):

1. Fork [obsidian-releases](https://github.com/obsidianmd/obsidian-releases)
2. Add plugin to `community-plugins.json`
3. Submit PR with plugin info and release
4. Wait for review and approval
