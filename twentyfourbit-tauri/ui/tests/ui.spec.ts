import { expect, test } from "@playwright/test";

test("window stays inside the viewport at desktop and compact sizes", async ({ page }) => {
  for (const width of [1120, 900, 800]) {
    await page.setViewportSize({ width, height: 600 });
    await page.goto("/");
    await expect(page.locator(".row").first()).toBeVisible();
    const layout = await page.evaluate(() => {
      const windowRect = document.querySelector(".window")!.getBoundingClientRect();
      const sidebarRect = document.querySelector(".sidebar")!.getBoundingClientRect();
      const contentRect = document.querySelector(".content")!.getBoundingClientRect();
      return {
        viewport: innerWidth,
        windowWidth: windowRect.width,
        windowLeft: windowRect.left,
        sidebarLeft: sidebarRect.left,
        contentRight: contentRect.right,
      };
    });
    expect(layout, `layout at ${width}px`).toEqual({
      viewport: width,
      windowWidth: width,
      windowLeft: 0,
      sidebarLeft: 0,
      contentRight: width,
    });
  }
});

test("chart selection performs a controlled React search", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "榜单", exact: true }).click();
  await page.locator(".chart").nth(1).locator(".chart-row").first().click();
  await expect(page.locator("#q")).toHaveValue("周杰伦");
  await expect(page.locator(".row-title")).toHaveText(["晴天", "青花瓷", "夜曲"]);
});

test("slash shortcut focuses search after navigating from another page", async ({ page }) => {
  await page.goto("/?page=charts");
  await expect(page.locator("h1")).toHaveText("本周榜单");
  await page.keyboard.press("/");
  await expect(page.locator("h1")).toHaveText("搜索");
  await expect(page.locator("#q")).toBeFocused();
});

test("clearing search invalidates an in-flight response", async ({ page }) => {
  await page.clock.install();
  await page.goto("/");
  await page.locator("#q").fill("roses");
  await page.clock.runFor(330); // debounce elapsed; demo response is still pending
  await page.locator("#q").fill("");
  await page.clock.runFor(100); // deliver the stale response deterministically
  await expect(page.locator("#q")).toHaveValue("");
  await expect(page.locator(".row")).toHaveCount(0);
  await expect(page.locator(".empty h2")).toHaveText("输入关键词开始搜索");
});

test("system theme is followed until the user makes an explicit choice", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  expect(await page.evaluate(() => localStorage.getItem("24bit.theme"))).toBeNull();
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
});

test("chart entries and quality options are keyboard operable", async ({ page }) => {
  await page.goto("/?page=charts");
  const artist = page.getByRole("button", { name: /周杰伦/ });
  await artist.press("Enter");
  await expect(page.locator("#q")).toHaveValue("周杰伦");

  await page.getByRole("button", { name: "设置", exact: true }).click();
  const surround = page.getByRole("radio", { name: /高清环绕声/ });
  await surround.press("Space");
  await expect(surround).toHaveAttribute("aria-checked", "true");
});

test("row actions remain visible when keyboard focus is inside", async ({ page }) => {
  await page.goto("/");
  const row = page.locator(".row").nth(1);
  await row.getByRole("button", { name: "下载到本地" }).focus();
  await expect(row.locator(".row-actions")).toHaveCSS("opacity", "1");
});

test("transport gives immediate feedback and supports keyboard seeking", async ({ page }) => {
  await page.goto("/");
  const toggle = page.getByRole("button", { name: "暂停（空格）", exact: true });
  await toggle.click();
  await expect(page.getByRole("button", { name: "播放（空格）", exact: true })).toBeVisible();
  const progress = page.getByRole("slider", { name: "播放进度" });
  await expect(progress).toHaveValue("26");
  await progress.press("ArrowRight");
  await expect(progress).toHaveValue("31");
});

test("download progress is separate from disabled playback seeking", async ({ page }) => {
  await page.goto("/?pending=1");
  const download = page.getByRole("progressbar", { name: "下载进度" });
  await expect(download).toHaveAttribute("aria-valuenow", "40");
  await expect(page.getByRole("slider", { name: "播放进度" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "暂停（空格）", exact: true })).toBeDisabled();
});

test("pause freezes progress and resume advances it again", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "暂停（空格）", exact: true }).click();
  const resume = page.getByRole("button", { name: "播放（空格）", exact: true });
  await expect(resume).toBeVisible();
  const progress = page.getByRole("slider", { name: "播放进度" });
  const pausedAt = Number(await progress.inputValue());
  await page.waitForTimeout(300);
  expect(Number(await progress.inputValue())).toBe(pausedAt);
  await resume.click();
  await expect(page.getByRole("button", { name: "暂停（空格）", exact: true })).toBeVisible();
  await page.waitForTimeout(1200);
  expect(Number(await progress.inputValue())).toBeGreaterThan(pausedAt);
});

test("scrubbing previews smoothly and commits the destination on release", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "暂停（空格）", exact: true }).click();
  const progress = page.getByRole("slider", { name: "播放进度" });
  const box = await progress.boundingBox();
  if (!box) throw new Error("progress bar has no bounds");
  await page.mouse.move(box.x + box.width * 0.1, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.75, box.y + box.height / 2, { steps: 12 });
  await expect(page.locator(".bar")).toHaveClass(/is-dragging/);
  const previewSeconds = Number(await progress.inputValue());
  expect(previewSeconds).toBeGreaterThan(185);
  expect(previewSeconds).toBeLessThan(200);
  await page.mouse.up();
  await expect(page.locator(".bar")).not.toHaveClass(/is-dragging/);
  await expect(progress).toHaveValue(String(previewSeconds));
  await page.mouse.move(box.x + box.width * 0.2, box.y + box.height / 2);
  await expect(progress).toHaveValue(String(previewSeconds));
});

test("playback mode control exposes all four modes", async ({ page }) => {
  await page.goto("/");
  for (const label of ["列表循环", "单曲循环", "随机播放", "顺序播放"]) {
    await page.locator(".playback-mode").click();
    await expect(page.locator(".playback-mode")).toHaveAttribute(
      "aria-label",
      `播放模式：${label}，点击切换`,
    );
  }
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await page.getByRole("button", { name: "顺序播放", exact: true }).click();
  await expect(page.locator(".playback-mode")).toHaveAttribute(
    "aria-label",
    "播放模式：列表循环，点击切换",
  );
});

test("single-repeat restarts the finished track", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("24bit.playback-mode", "repeat-one"));
  await page.goto("/?ended=1&track=last");
  await expect(page.locator(".playing-title")).toHaveText("如愿");
  await expect(page.getByRole("button", { name: "暂停（空格）", exact: true })).toBeVisible();
  const progress = page.getByRole("slider", { name: "播放进度" });
  await expect(progress).toHaveValue("0");
});

test("list-repeat wraps the final track and sequential stops there", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("24bit.playback-mode", "repeat-all"));
  await page.goto("/?ended=1&track=last");
  await expect(page.locator(".playing-title")).toHaveText("富士山下");
  await expect(page.getByRole("button", { name: "暂停（空格）", exact: true })).toBeVisible();

  await page.addInitScript(() => localStorage.setItem("24bit.playback-mode", "sequential"));
  await page.goto("/?ended=1&track=last");
  await expect(page.locator(".playing-title")).toHaveText("如愿");
  await expect(page.getByRole("button", { name: "播放（空格）", exact: true })).toBeVisible();
  await expect(page.getByRole("slider", { name: "播放进度" })).toHaveValue("258");
});
