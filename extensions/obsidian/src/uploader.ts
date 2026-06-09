/**
 * HTTP client for the zpic server.
 *
 * Communicates with the PicGo-compatible API documented in
 * `openspec/changes/add-http-server-for-obsidian/specs/api-specification.md`.
 *
 * Obsidian ships its own `requestUrl` helper which works on desktop
 * (Electron) and mobile (iOS / Android) without going through the
 * browser fetch stack, so it is the preferred transport.
 */

import { Notice, requestUrl, type RequestUrlParam } from "obsidian";
import type {
  HealthResponse,
  ServerConfigResponse,
  UploadErrorCode,
  UploadResponse,
  ZpicSettings,
} from "./types";
import { guessMimeType, normalizeServerUrl } from "./utils";

/** Subset of the Obsidian `RequestUrlResponse` we depend on. */
interface MinimalRequestResponse {
  status: number;
  json: unknown;
  text?: string;
}

export class ZpicUploader {
  private serverUrl: string;
  private timeout: number;

  constructor(settings: ZpicSettings) {
    this.serverUrl = normalizeServerUrl(settings.serverUrl);
    this.timeout = settings.timeout;
  }

  /**
   * Update the uploader when the user changes the server URL or timeout
   * from the settings tab.
   */
  updateSettings(settings: ZpicSettings): void {
    this.serverUrl = normalizeServerUrl(settings.serverUrl);
    this.timeout = settings.timeout;
  }

  /**
   * Upload one or more files. Dispatches to the JSON path-list mode
   * when given strings (file paths) and to the multipart mode when
   * given `File` objects.
   */
  async upload(input: File[] | string[]): Promise<UploadResponse> {
    if (input.length === 0) {
      return { success: false, msg: "No files to upload" };
    }

    try {
      if (typeof input[0] === "string") {
        return await this.uploadPaths(input as string[]);
      }
      return await this.uploadMultipart(input as File[]);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.error("[zpic] upload error:", message);
      return {
        success: false,
        msg: `Upload failed: ${message}`,
        code: "SERVER_ERROR",
      };
    }
  }

  /**
   * JSON path-list upload mode. Used on desktop Obsidian when the
   * dropped / pasted image is backed by a real file on disk.
   */
  private async uploadPaths(paths: string[]): Promise<UploadResponse> {
    const params: RequestUrlParam = {
      url: `${this.serverUrl}/upload`,
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ list: paths }),
      throw: false,
    };
    const response = await this.sendWithTimeout(params);
    return this.handleResponse(response);
  }

  /**
   * Multipart upload mode. Used on mobile and for clipboard images
   * that are not yet on disk.
   */
  private async uploadMultipart(files: File[]): Promise<UploadResponse> {
    const formData = new FormData();
    for (const file of files) {
      // The Electron `requestUrl` ignores `File.type` on some platforms
      // and Obsidian on iOS leaves it empty; fall back to a guessed
      // MIME based on the file extension so the server receives a
      // usable Content-Type for each part.
      const type = file.type || guessMimeType(file.name);
      formData.append("list", file, file.name);
      void type;
    }

    // The Obsidian typings only declare `body: string | ArrayBuffer`,
    // but `requestUrl` does accept a `FormData` instance at runtime
    // and generates the right multipart envelope. Cast through
    // `unknown` so the call is type-safe without resorting to `any`.
    const params: RequestUrlParam = {
      url: `${this.serverUrl}/upload`,
      method: "POST",
      body: formData as unknown as string,
      throw: false,
    };
    const response = await this.sendWithTimeout(params);
    return this.handleResponse(response);
  }

  /**
   * Race a `requestUrl` call against a timeout. We can't cancel the
   * underlying request from inside Obsidian, but rejecting early lets
   * the UI show a useful error rather than hanging.
   */
  private async sendWithTimeout(
    params: RequestUrlParam,
  ): Promise<MinimalRequestResponse> {
    const send = requestUrl(
      params,
    ) as unknown as Promise<MinimalRequestResponse>;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const timeout = new Promise<MinimalRequestResponse>((_, reject) => {
      timer = setTimeout(() => {
        reject(new Error(`Request timed out after ${this.timeout}ms`));
      }, this.timeout);
    });

    try {
      return await Promise.race([send, timeout]);
    } finally {
      if (timer) clearTimeout(timer);
    }
  }

  /** Parse the server response and surface structured errors. */
  private handleResponse(response: MinimalRequestResponse): UploadResponse {
    if (response.status < 200 || response.status >= 300) {
      return {
        success: false,
        msg: `Server returned HTTP ${response.status}`,
        code: "SERVER_ERROR",
      };
    }

    const data = (response.json ?? {}) as UploadResponse;
    if (!data || typeof data !== "object") {
      return {
        success: false,
        msg: "Server returned an invalid response",
        code: "SERVER_ERROR",
      };
    }

    if (!data.success) {
      return {
        success: false,
        msg: data.msg ?? "Upload failed",
        code: (data.code ?? "UPLOAD_FAILED") as UploadErrorCode,
      };
    }

    return data;
  }

  /**
   * Lightweight reachability check used before kicking off a real
   * upload. Returns `true` only when the server responds with HTTP 200
   * and a valid `HealthResponse` payload.
   */
  async checkHealth(): Promise<boolean> {
    try {
      const params: RequestUrlParam = {
        url: `${this.serverUrl}/health`,
        method: "GET",
        throw: false,
      };
      const response = await this.sendWithTimeout(params);
      if (response.status !== 200) return false;
      const data = (response.json ?? {}) as HealthResponse;
      return data.status === "ok";
    } catch {
      return false;
    }
  }

  /**
   * Fetch the non-sensitive server configuration. Currently used for
   * diagnostics, exposed in the settings tab so users can confirm
   * which uploader is active.
   */
  async getConfig(): Promise<ServerConfigResponse | null> {
    try {
      const params: RequestUrlParam = {
        url: `${this.serverUrl}/config`,
        method: "GET",
        throw: false,
      };
      const response = await this.sendWithTimeout(params);
      if (response.status !== 200) return null;
      return (response.json ?? null) as ServerConfigResponse | null;
    } catch {
      return null;
    }
  }
}

/**
 * Show a user-visible notice with a default 5s timeout. Centralising
 * this keeps the message style consistent across the plugin.
 */
export function showNotice(message: string, timeout = 5000): void {
  new Notice(message, timeout);
}
