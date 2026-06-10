/**
 * Utility helpers shared across the plugin.
 *
 * Keep this module dependency-free (other than the type-only `obsidian`
 * import) so it can be exercised from tests in the future.
 */

import { TFile } from 'obsidian';
import type { ImageDescMode } from './types';

/** Lower-case extensions accepted by the plugin and zpic server.
 *  Covers images, audio, and video. `webm` is shared by both audio
 *  and video upstream but is only listed once here. */
const MEDIA_EXTENSIONS = [
  // images
  '.png',
  '.jpg',
  '.jpeg',
  '.gif',
  '.webp',
  '.bmp',
  '.tiff',
  '.tif',
  '.svg',
  '.avif',
  // audio
  '.mp3',
  '.flac',
  '.wav',
  '.ogg',
  '.oga',
  '.m4a',
  '.3gp',
  // video
  '.mp4',
  '.webm',
  '.ogv',
];

/**
 * Check whether a file (browser `File` or Obsidian `TFile`) is uploadable
 * media by looking at its extension. The MIME type on `File` objects is
 * not reliable across mobile platforms, so we fall back to the extension.
 */
export function isImageFile(file: File | TFile): boolean {
  if (!file || !file.name) return false;
  const fileName = file.name.toLowerCase();
  return MEDIA_EXTENSIONS.some((ext) => fileName.endsWith(ext));
}

/**
 * Generate a short, unique placeholder id used inside the inserted
 * `![Uploading...{id}]()` markdown. Cryptographic strength is not needed
 * here; we just want to avoid clashes between concurrent uploads.
 */
export function generatePlaceholderId(): string {
  return Math.random().toString(36).slice(2, 9);
}

/**
 * Build the markdown image tag for an uploaded URL. The `desc` mode
 * controls whether the original filename is preserved as alt text.
 */
export function formatImageMarkdown(
  url: string,
  name: string,
  desc: ImageDescMode
): string {
  const altText = desc === 'origin' ? name : '';
  return `![${altText}](${url})`;
}

/**
 * Build the placeholder markdown inserted while an upload is in flight.
 * The id is embedded so the post-upload replacement can locate the row.
 */
export function getPlaceholderText(id: string): string {
  return `![Uploading ${id}…]()`;
}

/**
 * Normalize a user-provided server URL: trim whitespace and strip a
 * trailing slash so we can safely concatenate `/upload`, `/health`, etc.
 */
export function normalizeServerUrl(url: string): string {
  return url.trim().replace(/\/+$/, '');
}

/**
 * Heuristic MIME guess from a filename. Used as a fallback when the
 * platform-provided `File.type` is empty (common on iOS). Mirrors
 * the server-side extension table in `crates/zpic-media/src/detect.rs`.
 */
export function guessMimeType(fileName: string): string {
  const ext = fileName.toLowerCase().split('.').pop() ?? '';
  switch (ext) {
    // images
    case 'png':
      return 'image/png';
    case 'jpg':
    case 'jpeg':
      return 'image/jpeg';
    case 'gif':
      return 'image/gif';
    case 'webp':
      return 'image/webp';
    case 'bmp':
      return 'image/bmp';
    case 'svg':
      return 'image/svg+xml';
    case 'avif':
      return 'image/avif';
    case 'tif':
    case 'tiff':
      return 'image/tiff';
    // audio
    case 'mp3':
      return 'audio/mpeg';
    case 'flac':
      return 'audio/flac';
    case 'wav':
      return 'audio/wav';
    case 'ogg':
    case 'oga':
      return 'audio/ogg';
    case 'm4a':
      return 'audio/mp4';
    case '3gp':
      return 'audio/3gpp';
    // video
    case 'mp4':
      return 'video/mp4';
    case 'webm':
      return 'video/webm';
    case 'ogv':
      return 'video/ogg';
    default:
      return 'application/octet-stream';
  }
}
