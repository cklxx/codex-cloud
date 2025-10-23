import type { APIRequestContext } from "@playwright/test";
import { expect } from "@playwright/test";
import { randomUUID } from "node:crypto";

export type TaskFixture = {
  taskId: string;
  taskTitle: string;
  repositoryId: string;
  accessToken: string;
};

const defaultEmail = process.env.E2E_USER_EMAIL ?? "codex-e2e@example.com";
const defaultPassword = process.env.E2E_USER_PASSWORD ?? "codex-e2e";
const defaultName = process.env.E2E_USER_NAME ?? "Codex E2E";
const apiBase = process.env.E2E_API_BASE_URL ?? "http://localhost:8000";

async function ensureTestUser(request: APIRequestContext, email: string, password: string, name: string) {
  const response = await request.post(`${apiBase}/auth/users`, {
    data: { email, password, name },
    failOnStatusCode: false
  });

  expect(response.ok() || response.status() === 409).toBeTruthy();
}

async function login(request: APIRequestContext, email: string, password: string) {
  const response = await request.post(`${apiBase}/auth/session`, {
    data: { email, password }
  });

  expect(response.ok()).toBeTruthy();
  const json = await response.json();
  const token = json["access_token"] as string | undefined;
  expect(token).toBeTruthy();
  return token!;
}

async function createRepository(request: APIRequestContext, token: string) {
  const repoId = randomUUID();
  const payload = {
    name: `e2e-repo-${repoId.slice(0, 8)}`,
    git_url: `https://example.com/${repoId}.git`,
    default_branch: "main"
  };
  const response = await request.post(`${apiBase}/repositories`, {
    data: payload,
    headers: { Authorization: `Bearer ${token}` }
  });
  expect(response.ok()).toBeTruthy();
  const json = await response.json();
  return json["id"] as string;
}

async function createTask(request: APIRequestContext, token: string, repositoryId: string) {
  const taskTitle = `E2E Task ${new Date().toISOString()}`;
  const response = await request.post(`${apiBase}/tasks`, {
    data: {
      title: taskTitle,
      description: "Automated end-to-end scenario",
      repository_id: repositoryId
    },
    headers: { Authorization: `Bearer ${token}` }
  });
  expect(response.ok()).toBeTruthy();
  const json = await response.json();
  return { taskId: json["id"] as string, taskTitle };
}

export async function provisionTaskFixture(request: APIRequestContext): Promise<TaskFixture> {
  await ensureTestUser(request, defaultEmail, defaultPassword, defaultName);
  const token = await login(request, defaultEmail, defaultPassword);
  const repositoryId = await createRepository(request, token);
  const { taskId, taskTitle } = await createTask(request, token, repositoryId);

  return { taskId, taskTitle, repositoryId, accessToken: token };
}
