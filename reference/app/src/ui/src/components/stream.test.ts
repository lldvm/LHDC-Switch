import { describe, expect, it } from "vitest";
import type { LinkSample } from "../ipc/types";
import { linkQuality } from "./stream";

const sample = (i: number, over: Partial<LinkSample> = {}): LinkSample => ({
  time_ms: i * 250,
  rssi: -18,
  afh_in_use: 79,
  afh_usable: 79,
  afh_avoided: "",
  bytes_per_second: 125_000,
  packets_per_second: 188,
  queue_depth: 0,
  dropped: 0,
  capture_dropped: 0,
  airtime: 0.4,
  ...over,
});
const run = (n: number, f: (i: number) => Partial<LinkSample> = () => ({})) =>
  Array.from({ length: n }, (_, i) => sample(i, f(i)));

describe("link quality", () => {
  it("is good on a link that delivers everything, whatever the RSSI", () => {
    const q = linkQuality(run(60, (i) => ({ rssi: i % 2 ? -27 : -6 })));
    expect(q).toEqual({ level: 4, status: "good" });
  });

  it("is poor while packets drop, and recovers 10 s later", () => {
    const drops = run(60, (i) => ({ dropped: i < 50 ? 0 : 128 }));
    expect(linkQuality(drops)?.status).toBe("bad");
    const later = [...drops, ...run(40, () => ({ dropped: 128 }))];
    expect(linkQuality(later)?.status).toBe("good");
  });

  it("is fair while the send queue backs up", () => {
    const q = linkQuality(run(30, (i) => ({ queue_depth: i === 25 ? 40 : 0 })));
    expect(q?.status).toBe("warn");
  });

  it("has no data when nothing streams", () => {
    expect(linkQuality([])).toBeNull();
    expect(linkQuality([sample(0, { bytes_per_second: 0, rssi: null })])).toBeNull();
  });
});
