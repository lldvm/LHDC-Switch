// The profile editor against v1's profile dialog: saved devices renamed and
// removed, the codecs the device offers, the capture device's format

import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../app/App";
import { setLanguage, t } from "../i18n";
import { MockCore } from "../ipc/mock";
import { createCore } from "../ipc/store";

let mock: MockCore | null = null;

function start(hash: string) {
  location.hash = hash;
  const calls: { method: string; params: unknown }[] = [];
  mock = new MockCore({ speed: 0, onCall: (method, params) => calls.push({ method, params }) });
  const core = createCore(mock);
  render(() => <App core={core} />);
  return { calls };
}

const edit = (name: string) => `#/profiles/edit/${encodeURIComponent(name)}`;

afterEach(async () => {
  cleanup();
  mock?.dispose();
  mock = null;
  await setLanguage("en");
});

describe("saved devices in the picker", () => {
  it("renames a saved device", async () => {
    const { calls } = start("#/profiles/new");
    fireEvent.click(await screen.findByRole("button", { name: `${t("device.edit_name")}: SRS-XB13` }));
    const dialog = await screen.findByRole("dialog");
    const input = within(dialog).getByLabelText(t("device.enter_name"));
    expect(input).toHaveValue("SRS-XB13");

    // Not empty, no "|": the core refuses those
    fireEvent.input(input, { target: { value: "a|b" } });
    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.save") }));
    await within(dialog).findByText(t("ui.device.err_name"));
    expect(calls.some((c) => c.method === "device.rename")).toBe(false);

    fireEvent.input(input, { target: { value: "Desk speaker" } });
    fireEvent.click(within(dialog).getByRole("button", { name: t("modal.save") }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(calls.find((c) => c.method === "device.rename")?.params).toEqual({
      address: "00:1A:7D:DA:71:13",
      name: "Desk speaker",
    });
    expect(await screen.findByRole("radio", { name: /Desk speaker/ })).toBeInTheDocument();
  });

  it("renaming the chosen device updates the profile's device name", async () => {
    start(edit("WH-1000XM5 (LDAC)"));
    fireEvent.click(await screen.findByRole("button", { name: `${t("device.edit_name")}: WH-1000XM5` }));
    const dialog = await screen.findByRole("dialog");
    const input = within(dialog).getByLabelText(t("device.enter_name"));
    fireEvent.input(input, { target: { value: "My XM5" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    // The selected-device line shows it (the profile's own name stays)
    expect(document.querySelector(".selected-device")).toHaveTextContent("My XM5");
    expect(screen.getByLabelText("Profile Name")).toHaveValue("WH-1000XM5 (LDAC)");
  });

  it("removes a saved device after asking", async () => {
    const { calls } = start("#/profiles/new");
    fireEvent.click(await screen.findByRole("button", { name: `${t("ui.device.forget")}: SRS-XB13` }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("00:1A:7D:DA:71:13");
    fireEvent.click(within(dialog).getByRole("button", { name: t("ui.device.forget") }));
    await waitFor(() => expect(screen.queryByRole("radio", { name: /SRS-XB13/ })).toBeNull());
    expect(calls.find((c) => c.method === "device.forget")?.params).toEqual({ address: "00:1A:7D:DA:71:13" });
  });

  it("offers no rename or removal for devices that are not saved", async () => {
    start("#/profiles/new");
    fireEvent.click(await screen.findByRole("button", { name: t("connection.scan") }));
    await screen.findByRole("radio", { name: /Galaxy Buds2 Pro/ });
    expect(screen.queryByRole("button", { name: /Galaxy Buds2 Pro/ })).toBeNull();
  });
});

describe("codec cards", () => {
  it("show what the device offered when it last connected", async () => {
    start(edit("WH-1000XM5 (LDAC)"));
    const ldac = await screen.findByRole("radio", { name: /LDAC/ });
    expect(ldac.closest("label")).toHaveTextContent(t("ui.profile.codec_offered"));
    const hd = screen.getByRole("radio", { name: /aptX HD/ });
    expect(hd.closest("label")).toHaveTextContent(t("ui.profile.codec_not_offered"));
    expect(screen.queryByText(t("ui.profile.codecs_unknown"))).toBeNull();

    // A codec it does not offer: said plainly, with what happens instead
    fireEvent.click(hd);
    expect(await screen.findByText(t("error.codec_not_supported", "aptX HD"))).toBeInTheDocument();
    expect(screen.getByText(t("ui.profile.codec_fallback_on"))).toBeInTheDocument();
  });

  it("say it is not known before the first connection", async () => {
    start(edit("Desk speaker (AAC)"));
    expect(await screen.findByText(t("ui.profile.codecs_unknown"))).toBeInTheDocument();
    expect(screen.queryByText(t("ui.profile.codec_offered"))).toBeNull();
    expect(screen.queryByText(t("ui.profile.codec_not_offered"))).toBeNull();
  });
});

describe("LHDC V5", () => {
  it("offers 192 kHz and greys out the quality while ABR is on", async () => {
    start("#/profiles/new");
    fireEvent.click(await screen.findByRole("radio", { name: new RegExp(t("codec.lhdc_v5")) }));
    fireEvent.click(screen.getByRole("button", { name: /Advanced/ }));
    const rate = screen.getByLabelText(t("connection.sample_rate"));
    expect(within(rate).getByRole("option", { name: /192/ })).toBeInTheDocument();
    const quality = screen.getByLabelText(t("connection.quality"));
    expect(quality).not.toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox", { name: t("connection.abr") }));
    await waitFor(() => expect(quality).toBeDisabled());
    fireEvent.click(screen.getByRole("checkbox", { name: t("connection.abr") }));
    await waitFor(() => expect(quality).not.toBeDisabled());

    // LDAC's ABR keeps the quality, and has no 192 kHz
    fireEvent.click(screen.getByRole("radio", { name: new RegExp(t("codec.ldac")) }));
    fireEvent.click(screen.getByRole("checkbox", { name: t("connection.abr") }));
    expect(screen.getByLabelText(t("connection.quality"))).not.toBeDisabled();
    expect(within(screen.getByLabelText(t("connection.sample_rate"))).queryByRole("option", { name: /192/ })).toBeNull();
  });
});

describe("capture source", () => {
  it("shows the capture device's format and warns when the profile differs", async () => {
    start(edit("WH-1000XM5 (LDAC)"));
    // Loopback: the default device (the mock's speakers mix 48 kHz, 32-bit)
    const line = await screen.findByText(t("capture.device_format", 48000, 32));
    expect(line).not.toHaveClass("field-error");

    fireEvent.click(screen.getByRole("button", { name: /Advanced/ }));
    fireEvent.change(screen.getByLabelText(t("connection.sample_rate")), { target: { value: "96000" } });
    await waitFor(() => expect(document.querySelector(".field-error")).not.toBeNull());
    const warning = document.querySelector(".field-error")!;
    expect(warning).toHaveTextContent(t("capture.device_format", 48000, 32));
    expect(warning).toHaveTextContent(t("ui.profile.format_differs", t("connection.sample_rate")));
  });

  it("follows the virtual device by default", async () => {
    start("#/profiles/new");
    fireEvent.click(await screen.findByRole("radio", { name: t("capture.virtual") }));
    expect(await screen.findByRole("switch", { name: (n) => n.startsWith(t("capture.auto_switch")) })).toBeChecked();
  });

  it("says what application capture needs when the Windows build is not known", async () => {
    start(edit("Desk speaker (AAC)"));
    expect(await screen.findByText(t("ui.profile.app_capture_requirement"))).toBeInTheDocument();
    expect(screen.queryByText(t("ui.profile.app_capture_unsupported"))).toBeNull();
  });

  it("says plainly when this Windows cannot capture one application", async () => {
    // WebView2's client hints of Windows 10 1909 (build 18363)
    Object.defineProperty(navigator, "userAgentData", {
      configurable: true,
      value: { getHighEntropyValues: () => Promise.resolve({ platform: "Windows", platformVersion: "8.0.0" }) },
    });
    try {
      start(edit("Desk speaker (AAC)"));
      expect(await screen.findByText(t("ui.profile.app_capture_unsupported"))).toBeInTheDocument();
      expect(screen.getByLabelText(t("capture.app_select"))).toBeDisabled();
    } finally {
      delete (navigator as { userAgentData?: unknown }).userAgentData;
    }
  });
});
