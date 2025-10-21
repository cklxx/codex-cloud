const defaultBaseUrl = process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:8000";

export const API_BASE_URL = defaultBaseUrl.replace(/\/$/, "");

export async function apiFetch(path: string, init?: RequestInit, token?: string | null) {
  const headers = new Headers(init?.headers);
  headers.set("Content-Type", "application/json");
  const resolvedToken = token ?? headers.get("Authorization")?.replace(/^Bearer\s+/i, "");
  if (resolvedToken) {
    headers.set("Authorization", `Bearer ${resolvedToken}`);
  }

  return fetch(`${API_BASE_URL}${path}`, {
    ...init,
    headers
  });
}

export async function apiFetchAuthed(path: string, token: string, init?: RequestInit) {
  return apiFetch(path, init, token);
}
