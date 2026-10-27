import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import { App } from "../app/App";
import { MockCore } from "../ipc/mock";
import { createCore } from "../ipc/store";

let mock: MockCore | null = null;

function start() {
  location.hash = "#/diagnostics";
  const calls: { method: string; params: unknown }[] = [];
  mock = new MockCore({ speed: 0, onCall: (method, params) => calls.push({ method, params }) });
  const core = createCore(mock);
  const view = render(() => <App core={core} />);
  return { core, calls, view, mock };
}

afterEach(() => {
  cleanup();
  mock?.dispose();
  mock = null;
});

describe("Diagnostics", () => {
  it("shows a capture started elsewhere, its size, and the saved file", async () => {
    const { core, mock, view } = start();
    await screen.findByRole("heading", { level: 1, name: "Diagnostics" });
    const reveal = vi.spyOn(mock, "revealPath");
    // Started from another window: the snapshot event shows it here
    const path = await core.request("diag.capture_start");
    const line = await waitFor(() => {
      const el = view.container.querySelector(".capture-line");
      expect(el).not.toBeNull();
      return el!;
    });
    expect(line).toHaveTextContent(path);
    expect(line.querySelector(".capture-size")!.textContent).toMatch(/\d+ (B|KB)/);

    fireEvent.click(screen.getByRole("button", { name: "Stop HCI Capture" }));
    await waitFor(() => expect(view.container.querySelector(".capture-saved")).not.toBeNull());
    expect(view.container.querySelector(".capture-saved")).toHaveTextContent(path);
    fireEvent.click(view.container.querySelector(".capture-saved .btn-subtle")!);
    expect(reveal).toHaveBeenCalledWith(path);
  });

  it("masks addresses in the copied diagnostic info only while asked to", async () => {
    const { calls } = start();
    const copy = await screen.findByRole("button", { name: "Copy Diagnostics" });
    fireEvent.click(copy);
    await waitFor(() => expect(calls.some((c) => c.method === "diag.info")).toBe(true));
    expect(calls.find((c) => c.method === "diag.info")!.params).toEqual({ mask: true });
    fireEvent.click(screen.getByRole("checkbox", { name: "Mask Bluetooth addresses" }));
    fireEvent.click(copy);
    await waitFor(() => expect(calls.filter((c) => c.method === "diag.info").length).toBe(2));
    expect(calls.filter((c) => c.method === "diag.info")[1].params).toEqual({ mask: false });
    // Back to the default for the other tests (the setting lives with the window)
    fireEvent.click(screen.getByRole("checkbox", { name: "Mask Bluetooth addresses" }));
  });

  it("jumps from a flow step to its log line", async () => {
    const { core, view } = start();
    await screen.findByRole("heading", { level: 1, name: "Diagnostics" });
    await waitFor(() => expect(core.state.snap).not.toBeNull());
    await core.request("stream.start", { profile: core.state.snap!.profiles[0].name });
    await waitFor(() => expect(core.state.snap!.stream.state).toBe("streaming"));

    fireEvent.click(screen.getByRole("tab", { name: "Connection Flow" }));
    const jump = await waitFor(() => {
      const b = view.container.querySelector<HTMLButtonElement>(".flow-step .flow-jump button");
      expect(b).not.toBeNull();
      return b!;
    });
    // The step's time of day, from the core's clock (after its column name
    // for screen readers)
    expect(view.container.querySelector(".flow-step .flow-clock")!.textContent).toMatch(/^Time: \d\d:\d\d:\d\d\.\d{3}$/);
    expect(view.container.querySelector(".flow-head")).toHaveTextContent(/Step\s*Status\s*Time\s*Elapsed\s*Detail/);
    // The details the core computes: the audio rate
    await waitFor(() => expect(view.container.querySelector(".flow-step:nth-child(10) .flow-detail")).toHaveTextContent(/packets\/s/));
    fireEvent.click(jump);

    await waitFor(() => expect(screen.getByRole("tab", { name: "Log" })).toHaveAttribute("aria-selected", "true"));
    const line = await waitFor(() => {
      const el = view.container.querySelector(".log-line.jumped");
      expect(el).not.toBeNull();
      return el!;
    });
    expect(line).toHaveClass("selected");
    expect(line).toHaveTextContent("stream: device");
    expect(screen.getByRole("button", { name: "Copy Selected Lines" })).not.toBeDisabled();

    // This attempt only keeps the attempt's lines
    fireEvent.click(screen.getByRole("checkbox", { name: "Current attempt only" }));
    await waitFor(() => expect(view.container.querySelector(".log-line")).toHaveTextContent("stream: device"));
  });
});
