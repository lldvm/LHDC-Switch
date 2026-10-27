import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../app/App";
import { setLanguage } from "../i18n";
import { MockCore, type MockOptions } from "../ipc/mock";
import { createCore } from "../ipc/store";

let mock: MockCore | null = null;

function start(hash: string, opts: MockOptions = {}) {
  location.hash = hash;
  const calls: { method: string; params: unknown }[] = [];
  mock = new MockCore({ speed: 0, ...opts, onCall: (method, params) => calls.push({ method, params }) });
  const core = createCore(mock);
  const view = render(() => <App core={core} />);
  return { core, calls, view };
}

afterEach(async () => {
  cleanup();
  mock?.dispose();
  mock = null;
  await setLanguage("en");
});

describe("Home", () => {
  it("connects and disconnects with the big button", async () => {
    const { calls } = start("#/home");
    const connect = await screen.findByRole("button", { name: "Connect" });
    fireEvent.click(connect);
    // The mock goes connecting -> streaming
    const disconnect = await screen.findByRole("button", { name: "Disconnect" });
    expect(calls.some((c) => c.method === "stream.start")).toBe(true);
    await waitFor(() => expect(screen.getAllByText("990 kbps").length).toBeGreaterThan(0));
    expect(screen.getByRole("slider", { name: "Volume" })).not.toBeDisabled();
    fireEvent.click(disconnect);
    await screen.findByRole("button", { name: "Connect" });
    expect(calls.some((c) => c.method === "stream.stop")).toBe(true);
  });

  it("asks to exit v1 while it runs", async () => {
    start("#/home");
    await screen.findByRole("button", { name: "Connect" });
    expect(screen.queryByText("A2DPWB v1 is running — exit it first")).toBeNull();
    mock!.setV1Running(true);
    expect(await screen.findByText("A2DPWB v1 is running — exit it first")).toBeInTheDocument();
    mock!.setV1Running(false);
    await waitFor(() => expect(screen.queryByText("A2DPWB v1 is running — exit it first")).toBeNull());
  });

  it("shows the cause of an error and a fix", async () => {
    start("#/home", { scenario: "error" });
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("The device did not answer");
    expect(within(alert).getByRole("button", { name: "Try again" })).toBeInTheDocument();
  });
});

describe("profile editor", () => {
  it("does not save until the profile is valid", async () => {
    const { calls } = start("#/profiles/new");
    const save = await screen.findByRole("button", { name: "Save" });
    fireEvent.click(save);
    await screen.findByText("Enter a name for the profile");
    expect(screen.getAllByText("Choose the headphones (scan, or enter the address)").length).toBeGreaterThan(0);
    expect(calls.some((c) => c.method === "profile.save")).toBe(false);

    // A name another profile has
    const name = screen.getByLabelText("Profile Name");
    fireEvent.input(name, { target: { value: "WH-1000XM5 (LDAC)" } });
    await screen.findByText("Another profile already has this name");

    fireEvent.input(name, { target: { value: "Test profile" } });
    fireEvent.input(screen.getByLabelText("Manual:"), { target: { value: "aa-bb-cc-dd-ee-ff" } });
    fireEvent.click(screen.getByRole("button", { name: "Use" }));
    fireEvent.click(screen.getByRole("radio", { name: /aptX HD/ }));
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(calls.some((c) => c.method === "profile.save")).toBe(true));
    const saved = calls.find((c) => c.method === "profile.save")!.params as {
      profile: { name: string; device_address: string; codec: string };
      original_name: string | null;
    };
    expect(saved.profile).toMatchObject({ name: "Test profile", device_address: "AA:BB:CC:DD:EE:FF", codec: "aptxhd" });
    expect(saved.original_name).toBeNull();
  });

  it("checks the packet size in the advanced settings", async () => {
    const { calls } = start(`#/profiles/edit/${encodeURIComponent("WH-1000XM5 (LDAC)")}`);
    fireEvent.click(await screen.findByRole("button", { name: /Advanced/ }));
    const packet = screen.getByLabelText("Max media packet size (bytes)");
    fireEvent.input(packet, { target: { value: "2000" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await screen.findByText("Enter a value from 679 to 1679");
    expect(calls.some((c) => c.method === "profile.save")).toBe(false);
  });
});

describe("every screen renders", () => {
  const routes: [string, string][] = [
    ["#/home", "Home"],
    ["#/profiles", "Profiles"],
    ["#/profiles/new", "New Profile"],
    ["#/diagnostics", "Diagnostics"],
    ["#/adapter", "Adapter"],
    ["#/settings", "Settings"],
  ];
  for (const [hash, heading] of routes) {
    it(hash, async () => {
      start(hash);
      expect(await screen.findByRole("heading", { level: 1, name: heading })).toBeInTheDocument();
      expect(screen.getByRole("navigation")).toBeInTheDocument();
    });
  }

  it("diagnostics tabs", async () => {
    start("#/diagnostics");
    for (const name of ["Connection Flow", "Log", "Link Quality"]) {
      fireEvent.click(await screen.findByRole("tab", { name }));
      expect(screen.getByRole("tab", { name })).toHaveAttribute("aria-selected", "true");
    }
    expect(screen.queryByRole("tab", { name: "Peer Receiver Test" })).toBeNull();
  });

  it("the first-run setup when no adapter is on WinUSB", async () => {
    const { calls } = start("#/home", { scenario: "fresh" });
    expect(await screen.findByRole("heading", { level: 1, name: "Set up A2DPWB" })).toBeInTheDocument();
    mock!.setV1Running(true);
    expect(await screen.findByText("A2DPWB v1 is running — exit it first")).toBeInTheDocument();
    mock!.setV1Running(false);
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    // Step 2: assigning WinUSB asks first
    fireEvent.click(await screen.findByRole("button", { name: "Assign WinUSB" }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("Windows' own Bluetooth no longer sees this adapter");
    fireEvent.click(within(dialog).getByRole("button", { name: "Assign WinUSB" }));
    await waitFor(() => expect(calls.some((c) => c.method === "adapter.assign_winusb")).toBe(true));
    await waitFor(() => expect(screen.getByRole("button", { name: "Next" })).not.toBeDisabled());
  });

  it("the flyout", async () => {
    start("", { window: "flyout" });
    expect(await screen.findByRole("button", { name: "Connect" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open app" })).toBeInTheDocument();
  });

  it("in Japanese", async () => {
    const { core } = start("#/settings");
    await screen.findByRole("heading", { level: 1, name: "Settings" });
    await core.request("settings.set", { settings: { ...core.state.snap!.settings, language: "ja" } });
    expect(await screen.findByRole("link", { name: "ホーム" })).toBeInTheDocument();
  });
});

describe("update", () => {
  it("installs at once when nothing streams", async () => {
    start("#/settings", { scenario: "update" });
    fireEvent.click(await screen.findByRole("button", { name: "Update now" }));
    await screen.findByText(/Installing version 2\.0\.0-beta\.1/);
  });

  it("waits for the stream to stop", async () => {
    start("#/settings", { scenario: "update" });
    await mock!.call("stream.start", { profile: "WH-1000XM5 (LDAC)" });
    await waitFor(() => expect(mock!["snap"].stream.state).toBe("streaming"));
    fireEvent.click(await screen.findByRole("button", { name: "Update now" }));
    await screen.findByText(/It is installed when streaming stops/);
    await mock!.call("stream.stop");
    await screen.findByText(/Installing version 2\.0\.0-beta\.1/, {}, { timeout: 3000 });
  });
});
