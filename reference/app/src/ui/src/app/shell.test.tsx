// The app shell's v1 leftovers: update check result, update notice, tray
// hide, firmware bar, error copy, capture source, HCI capture sign, the
// setup wizard's test tone.

import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App, hideToTray, isDiagnosticsShortcut } from "./App";
import { captureRunningText } from "./indicators";
import { errorCopyText, sourceText } from "../components/stream";
import { setLanguage, t } from "../i18n";
import { MockCore, sampleProfile, type MockOptions } from "../ipc/mock";
import { createCore } from "../ipc/store";
import { LINKS } from "../lib/links";
import { toneParams } from "../screens/Setup";

let mock: MockCore | null = null;

function start(hash: string, opts: MockOptions = {}) {
  location.hash = hash;
  const calls: { method: string; params: unknown }[] = [];
  mock = new MockCore({ speed: 0, ...opts, onCall: (method, params) => calls.push({ method, params }) });
  const core = createCore(mock);
  const view = render(() => <App core={core} />);
  return { core, calls, view, mock };
}

afterEach(async () => {
  cleanup();
  mock?.dispose();
  mock = null;
  vi.restoreAllMocks();
  await setLanguage("en");
});

describe("keyboard", () => {
  it("Ctrl+Shift+D opens the diagnostics", async () => {
    start("#/home");
    await screen.findByRole("button", { name: "Connect" });
    fireEvent.keyDown(window, { key: "D", code: "KeyD", ctrlKey: true, shiftKey: true });
    expect(await screen.findByRole("heading", { level: 1, name: "Diagnostics" })).toBeInTheDocument();
    expect(isDiagnosticsShortcut({ key: "d", code: "KeyD", ctrlKey: true, shiftKey: false, altKey: false, metaKey: false })).toBe(false);
    expect(isDiagnosticsShortcut({ key: "D", code: "KeyD", ctrlKey: true, shiftKey: true, altKey: true, metaKey: false })).toBe(false);
  });
});

describe("update check", () => {
  it("a failed check shows as failed with the releases page", async () => {
    const { mock } = start("#/settings", { scenario: "offline" });
    const open = vi.spyOn(mock, "openUrl").mockResolvedValue();
    fireEvent.click(await screen.findByRole("button", { name: "Check for Updates" }));
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Could not check for updates.");
    fireEvent.click(within(alert).getByRole("button", { name: t("ui.update.open_releases") }));
    expect(open).toHaveBeenCalledWith(LINKS.releases);
    // Not also as a transient notice
    expect(document.querySelector(".notice-error")).toBeNull();
  });

  it("a newer release shows on every screen, with the way to Settings", async () => {
    start("#/home", { scenario: "update" });
    const notice = await screen.findByText("Version 2.0.0-beta.1 is available (current: 2.0.0-alpha.1).");
    const banner = notice.closest(".banner") as HTMLElement;
    fireEvent.click(within(banner).getByRole("button", { name: t("ui.update.show") }));
    expect(await screen.findByRole("heading", { level: 1, name: "Settings" })).toBeInTheDocument();
    // Settings has its own banner with the install button: one, not two
    expect(screen.getAllByText(/Version 2\.0\.0-beta\.1 is available/)).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Update now" })).toBeInTheDocument();
  });

  it("the update notice can be closed", async () => {
    start("#/profiles", { scenario: "update" });
    const banner = (await screen.findByText(/is available \(current/)).closest(".banner") as HTMLElement;
    fireEvent.click(within(banner).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByText(/is available \(current/)).toBeNull());
  });
});

describe("tray", () => {
  it("hiding to the tray tells the core", async () => {
    const { core, calls, mock } = start("#/home");
    await screen.findByRole("heading", { level: 1, name: "Home" });
    const hide = vi.spyOn(mock, "hideWindow");
    await hideToTray(core);
    expect(hide).toHaveBeenCalled();
    expect(calls.some((c) => c.method === "app.hidden_to_tray")).toBe(true);
  });

  it("start in the tray applies only when starting with Windows", async () => {
    const { core } = start("#/settings");
    const toggle = await screen.findByRole("switch", { name: /Start in the tray/ });
    expect(toggle).not.toBeDisabled();
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, start_with_windows: false } });
    await waitFor(() => expect(toggle).toBeDisabled());
  });
});

describe("Home", () => {
  it("shows the firmware bar and leads to the Adapter screen", async () => {
    const { mock } = start("#/home");
    await screen.findByRole("heading", { level: 1, name: "Home" });
    const m = mock as unknown as { snap: { adapter: { adapters: { firmware_present: boolean | null }[] } }; emit: (e: unknown) => void };
    const adapter = { ...m.snap.adapter, adapters: [{ ...m.snap.adapter.adapters[0], firmware_present: false }] };
    m.emit({ kind: "adapter", value: adapter });
    const title = await screen.findByText("Firmware not found");
    const bar = title.closest(".banner") as HTMLElement;
    expect(bar).toHaveTextContent("rtl8761bu_fw.bin, rtl8761bu_config.bin");
    fireEvent.click(within(bar).getByRole("button", { name: t("ui.home.firmware_fix") }));
    expect(await screen.findByRole("heading", { level: 1, name: "Adapter" })).toBeInTheDocument();
  });

  it("copies the error text", async () => {
    start("#/home", { scenario: "error" });
    const write = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText: write }, configurable: true });
    const alert = await screen.findByRole("alert");
    fireEvent.click(within(alert).getByRole("button", { name: t("ui.error.copy") }));
    await waitFor(() => expect(write).toHaveBeenCalledWith(t("error.connect_no_answer")));
    expect(await screen.findByText(t("ui.error.copied"))).toBeInTheDocument();
  });

  it("shows the capture source, waiting for the application", async () => {
    const { mock } = start("#/home");
    await screen.findByRole("heading", { level: 1, name: "Home" });
    await mock.call("stream.start", { profile: "Desk speaker (AAC)" });
    await mock.call("profile.set_active", { name: "Desk speaker (AAC)" });
    expect(await screen.findByText(t("ui.source.waiting", "Spotify"))).toBeInTheDocument();
  });

  it("shows a running HCI capture app-wide", async () => {
    const { mock } = start("#/home");
    await screen.findByRole("heading", { level: 1, name: "Home" });
    await mock.call("diag.capture_start");
    const sign = await screen.findByRole("button", { name: "HCI capture running" });
    fireEvent.click(sign);
    expect(await screen.findByRole("heading", { level: 1, name: "Diagnostics" })).toBeInTheDocument();
    await mock.call("diag.capture_stop");
    await waitFor(() => expect(screen.queryByRole("button", { name: "HCI capture running" })).toBeNull());
  });
});

describe("flyout", () => {
  it("shows the capture source and the capture sign", async () => {
    const { mock } = start("", { window: "flyout" });
    await screen.findByRole("button", { name: "Connect" });
    await mock.call("diag.capture_start");
    await mock.call("stream.start_custom", { device: "AC:80:0A:12:34:56", codec: "sbc", quality: "hq", source: "tone" });
    expect(await screen.findByText("Test tone")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "HCI capture running" })).toBeInTheDocument();
  });
});

describe("setup test step", () => {
  async function toTestStep() {
    const r = start("#/setup");
    await screen.findByRole("heading", { level: 1, name: "Set up A2DPWB" });
    for (let i = 0; i < 3; i++) {
      const next = screen.getByRole("button", { name: "Next" });
      await waitFor(() => expect(next).not.toBeDisabled());
      fireEvent.click(next);
    }
    await screen.findByRole("heading", { level: 2, name: "Test" });
    return r;
  }

  it("connects with the chosen capture source; no test tone outside developer mode", async () => {
    const { calls } = await toTestStep();
    expect(screen.queryByRole("button", { name: "Play test tone" })).toBeNull();
    expect(screen.getByRole("combobox", { name: "Capture Source" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(calls.some((c) => c.method === "stream.start")).toBe(true));
    expect(calls.some((c) => c.method === "stream.start_custom")).toBe(false);
  });

  async function devTestStep() {
    const r = await toTestStep();
    await r.core.request("settings.set", { settings: { ...r.core.state.snap!.settings, developer_mode: true } });
    await screen.findByRole("button", { name: "Play test tone" });
    return r;
  }

  it("developer mode plays the core's test tone on the headphones", async () => {
    const { calls } = await devTestStep();
    fireEvent.click(screen.getByRole("button", { name: "Play test tone" }));
    await waitFor(() => expect(calls.some((c) => c.method === "stream.start_custom")).toBe(true));
    const params = calls.find((c) => c.method === "stream.start_custom")!.params;
    expect(params).toMatchObject({ device: "AC:80:0A:12:34:56", codec: "ldac", quality: "hq", source: "tone" });
    fireEvent.click(await screen.findByRole("button", { name: "Stop" }));
    await waitFor(() => expect(calls.some((c) => c.method === "stream.stop")).toBe(true));
    await screen.findByRole("button", { name: "Play test tone" });
  });

  it("says so when the tone cannot be played", async () => {
    const { mock } = await devTestStep();
    // The mock refuses to stream without WinUSB
    const m = mock as unknown as { snap: { adapter: { adapters: { driver: string }[] } } };
    m.snap.adapter.adapters[0] = { ...m.snap.adapter.adapters[0], driver: "bluetooth" };
    fireEvent.click(screen.getByRole("button", { name: "Play test tone" }));
    const failed = await screen.findByText((text) => text.startsWith(t("ui.setup.tone_failed", "").slice(0, 12)));
    expect(failed).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Connect" })).toBeInTheDocument();
  });
});

describe("helpers", () => {
  it("source text", () => {
    const p = sampleProfile({ app_exe: "Spotify.exe", app_name: "Spotify" });
    expect(sourceText({ kind: "tone", name: "", waiting: false }, null).text).toBe("Test tone");
    expect(sourceText({ kind: "loopback", name: "", waiting: false }, null).text).toBe("System Loopback");
    expect(sourceText({ kind: "virtual", name: "CABLE Input", waiting: false }, null).text).toBe("CABLE Input");
    expect(sourceText({ kind: "app", name: "spotify.exe", waiting: false }, p).text).toBe("Spotify");
    const waiting = sourceText({ kind: "app", name: "foo.exe", waiting: true }, p);
    expect(waiting).toMatchObject({ waiting: true, text: t("ui.source.waiting", "foo.exe") });
  });

  it("error copy text", () => {
    expect(errorCopyText({ key: "error.connect_no_answer", args: [] })).toBe(t("error.connect_no_answer"));
    expect(errorCopyText({ key: "error.connect_no_answer", args: [] }, "stream.failed")).toMatch(/\n\[stream\.failed\]$/);
  });

  it("capture sign text from v1's title suffix", async () => {
    expect(captureRunningText()).toBe("HCI capture running");
    await setLanguage("ja");
    expect(captureRunningText()).toBe("HCI キャプチャ中");
  });

  it("tone parameters from the profile", () => {
    const p = sampleProfile({ sample_rate: 0, bit_depth: 24, codec: "aac", max_media_payload: 0 });
    expect(toneParams(p)).toEqual({
      device: "AC:80:0A:12:34:56",
      device_name: "WH-1000XM5",
      codec: "aac",
      quality: "hq",
      sample_rate: null,
      bit_depth: 24,
      source: "tone",
      max_media_payload: null,
      abr: false,
      codec_fallback: true,
    });
  });
});
