import { test, expect, Page } from "@playwright/test";
import { provisionTaskFixture } from "./helpers";

const email = process.env.E2E_USER_EMAIL ?? "codex-e2e@example.com";
const password = process.env.E2E_USER_PASSWORD ?? "codex-e2e";

async function performLogin(page: Page) {
  await page.goto("/");
  await page.getByRole("tab", { name: "登录" }).click();
  await page.getByPlaceholder("admin@example.com").fill(email);
  await page.getByPlaceholder("******").fill(password);
  await page.getByRole("button", { name: "登录" }).click();
  await expect(page).toHaveURL(/\/tasks$/);
  await expect(page.getByRole("heading", { name: "任务列表" })).toBeVisible();
}

test.describe("Codex Cloud task lifecycle", () => {
  test("allows login and end-to-end task lifecycle", async ({ page, request }) => {
    const fixture = await provisionTaskFixture(request);

    await performLogin(page);

    const taskLink = page.getByRole("link", { name: fixture.taskTitle });
    await expect(taskLink).toBeVisible();
    await taskLink.click();

    await expect(page.getByRole("heading", { name: fixture.taskTitle })).toBeVisible();
    const statusTag = page.getByText("pending", { exact: true });
    await expect(statusTag).toBeVisible();

    const claimButton = page.getByRole("button", { name: "认领任务" });
    await expect(claimButton).toBeEnabled();
    await claimButton.click();
    await expect(page.getByText("claimed", { exact: true })).toBeVisible();

    const startAttemptButton = page.getByRole("button", { name: "启动尝试" });
    await expect(startAttemptButton).toBeEnabled();
    await startAttemptButton.click();
    await expect(page.getByText("running", { exact: true })).toBeVisible();

    await expect(page.getByRole("heading", { name: "完成当前尝试" })).toBeVisible();
    await page.getByLabel("Diff 内容").fill("diff --git a/file b/file");
    await page.getByLabel("执行日志").fill("Test execution log");
    await page.getByRole("button", { name: "提交结果" }).click();

    await expect(page.getByText("review", { exact: true })).toBeVisible();
    await expect(page.getByText("成功", { exact: true })).toBeVisible();
  });
});
