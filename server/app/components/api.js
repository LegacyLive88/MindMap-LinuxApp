"use client";

import { compactLibrary } from "../../lib/library";

export function clientId() {
  const key = "mindmap-client";
  let id = window.localStorage.getItem(key);
  if (!id) {
    id = crypto.randomUUID();
    window.localStorage.setItem(key, id);
  }
  return id;
}

export async function api(path, options = {}) {
  const headers = { Accept: "application/json", ...(options.headers || {}) };
  if (options.body) headers["Content-Type"] = "application/json";
  const response = await fetch(path, { ...options, headers, cache: "no-store" });
  const data = await response.json().catch(() => ({}));
  if (response.status === 401 && !path.endsWith("/login") && !path.endsWith("/setup")) {
    window.location.assign("/login");
  }
  if (!response.ok) {
    const error = new Error(data.error || "The cloud refused that request.");
    error.status = response.status;
    error.data = data;
    throw error;
  }
  return data;
}

export async function saveLibrary(library, revision, force = false) {
  return api("/api/v1/sync", {
    method: "POST",
    body: JSON.stringify({
      base_revision: revision,
      client_id: clientId(),
      force,
      library: compactLibrary(library),
    }),
  });
}
