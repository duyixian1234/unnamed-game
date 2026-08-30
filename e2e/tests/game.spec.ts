import { expect, test } from "@playwright/test";

test("starts a run without a browser panic", async ({ page }) => {
  const browserErrors: string[] = [];
  const states: string[] = [];
  let sawWebGpuAdapter = false;
  page.on("console", (message) => {
    const text = message.text();
    console.log(`[browser:${message.type()}] ${text}`);
    sawWebGpuAdapter ||= text.includes("backend: BrowserWebGpu");
    const state = text.match(/\[game-state\] (\w+)/)?.[1];
    if (state) {
      states.push(state);
    }
    if (message.type() === "error") {
      browserErrors.push(text);
    }
  });
  page.on("pageerror", (error) => {
    const text = String(error);
    console.log(`[browser:pageerror] ${text}`);
    browserErrors.push(text);
  });

  await page.goto("/");
  await expect(page.locator("canvas")).toHaveCount(1, { timeout: 20_000 });
  await expect.poll(() => sawWebGpuAdapter).toBe(true);
  await expect.poll(() => states).toContain("MainMenu");

  const canvas = page.locator("canvas");
  const bounds = await canvas.boundingBox();
  expect(bounds).not.toBeNull();
  await page.waitForTimeout(300);
  for (const height of [0.58, 0.55, 0.61]) {
    if (states.includes("StartingWeaponChoice")) {
      break;
    }
    await page.mouse.click(
      bounds!.x + bounds!.width / 2,
      bounds!.y + bounds!.height * height
    );
    await page.waitForTimeout(300);
  }
  await expect.poll(() => states).toContain("StartingWeaponChoice");

  await page.waitForTimeout(300);
  await page.keyboard.press("Enter");
  await page.waitForTimeout(100);
  await page.keyboard.press("Enter");
  await expect.poll(() => states).toContain("InGame");

  await page.keyboard.down("KeyD");
  await page.waitForTimeout(300);
  await page.keyboard.up("KeyD");
  await page.waitForTimeout(500);

  const fatalErrors = browserErrors.filter(
    (error) =>
      /panicked at|error\[B0001\]|RuntimeError: unreachable/i.test(error) &&
      !/\.well-known\/trunk\/ws/i.test(error)
  );
  expect(fatalErrors, browserErrors.join("\n")).toEqual([]);
});
