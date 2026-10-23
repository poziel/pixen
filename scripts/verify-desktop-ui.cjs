// Run through playwright-cli run-code --filename=scripts/verify-desktop-ui.cjs
// after attaching it to the Tauri WebView's local CDP endpoint.
async (page) => {
  // Start the test desktop with the repository root as its working directory.
  const root = ".";
  const check = (condition, message) => {
    if (!condition) throw new Error(message);
  };
  const invoke = (command, args = {}) =>
    page.evaluate(
      ({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args),
      { command, args },
    );
  check(
    (await page
      .getByRole("navigation", { name: "Primary view bar" })
      .count()) === 1,
    "one primary view bar",
  );
  check((await page.getByRole("tab").count()) === 0, "no document tabs");
  await invoke("open_document", {
    path: `${root}/examples/mcp-animation/output/bounce.pixloom.json`,
  });
  await page.getByRole("button", { name: "Timeline", exact: true }).click();
  await page.getByRole("button", { name: /^Frame 6 / }).waitFor();
  if (
    await page.getByRole("button", { name: "Pause", exact: true }).isVisible()
  )
    await page.getByRole("button", { name: "Pause", exact: true }).click();
  await page.getByRole("button", { name: "Stop", exact: true }).click();
  const snapshot = await invoke("snapshot");
  check(snapshot.document.frames.length === 6, "six animation frames");
  await page.getByRole("button", { name: "Next frame", exact: true }).click();
  await page.getByText(/Frame 2 \/ 6/).waitFor();
  const canvas = page.locator("canvas.artwork");
  await canvas.waitFor({ state: "visible" });
  check(
    await canvas.evaluate((c) => c.width === 24 && c.height === 24),
    "native render reaches Vue canvas",
  );
  await page.getByRole("button", { name: "Layers", exact: true }).click();
  check(
    (await page.getByRole("button", { name: /^Solo / }).count()) === 2,
    "layer controls render",
  );
  const before = await invoke("snapshot");
  await page
    .getByRole("button", { name: /^Solo / })
    .first()
    .click();
  const after = await invoke("snapshot");
  check(
    before.document.revision === after.document.revision,
    "solo stays preview-only",
  );
  const report = await invoke("review_document", {
    kind: "analyze",
    frameId: snapshot.document.frames[0].id,
    layerId: null,
  });
  check(report.report.kind === "sprite_analysis", "native review engine");
  await page
    .getByRole("heading", { name: "Art review", exact: true })
    .waitFor();
  await page.getByRole("button", { name: "Close modal", exact: true }).click();
  await page.getByRole("button", { name: "AI chat", exact: true }).click();
  check(
    await page
      .getByText("AI chat is not available in this version.", { exact: true })
      .isVisible(),
    "AI placeholder",
  );
  await page.getByRole("button", { name: "Layers", exact: true }).click();
  await page.screenshot({ path: "target/frame-native-verified.png" });
  const result = {
    frames: snapshot.document.frames.length,
    layers: snapshot.document.layers.length,
    canvas: "24x24",
    tabs: 0,
    review: report.report.kind,
  };
  const closed=page.waitForEvent('close',{timeout:10000});
  await page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('plugin:window|close'));
  await closed;
  return result;
}
