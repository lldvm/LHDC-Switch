import { describe, expect, it } from "vitest";
import { setLanguage, t } from "../i18n";
import { sampleProfile } from "../ipc/mock";
import type { LogLine } from "../ipc/types";
import { bitrateNoteKey, codecLabelKey, fitToCodec, qualityIgnored, qualityTag, sampleRates } from "./codecs";
import { FlowTracker, stepDetail } from "./flow";
import { maskAddresses, parseAddress } from "./format";
import { thirdPartyUrl } from "./links";
import { validateProfile } from "./profile";

const line = (time_ms: number, message: string, level: LogLine["level"] = "info"): LogLine => ({
  time_ms,
  level,
  target: "test",
  message,
});

describe("connection flow", () => {
  it("follows a connection from start to streaming", () => {
    const f = new FlowTracker();
    const lines = [
      "stream: device AC:80:0A:12:34:56 codec Ldac quality Hq",
      "state Connecting: Initializing BTstack...",
      "controller: company 0x005D (Realtek), HCI version 10",
      "HCI ready after 412 ms",
      "connecting to AC:80:0A:12:34:56",
      "HCI ACL connection established to AC:80:0A:12:34:56",
      "signaling connection established (cid 0x0041, handle 0x000b)",
      "capability discovery complete (LDAC=true SBC=true)",
      "codec selected: LDAC",
      "capture: 48000 Hz, 2 channel(s), F32 -> 2 channel(s)",
      "HCI capture: started -> C:\\x.pklg",
      "codec configured (local 1, remote 3)",
      "stream established (local 1, remote 3, mtu 1005)",
      "encoder ready: LDAC 990 kbps, mtu 1005, 24-bit, ABR false",
      "streaming started",
      "state Streaming: Streaming",
    ];
    lines.forEach((m, i) => f.feed(line(1000 + i, m)));
    const s = f.session!;
    for (const step of Object.values(s.steps)) expect(step.status).toBe("done");
    expect(s.steps.btstack.detail).toContain("Realtek");
    expect(s.steps.codec.detail).toBe("LDAC");
    expect(s.steps.capture.detail).toContain("48000 Hz");
  });

  it("blames the first unfinished step on an error, and counts reconnects", () => {
    const f = new FlowTracker();
    f.feed(line(1, "stream: device AA:BB:CC:DD:EE:FF"));
    f.feed(line(2, "HCI ready after 300 ms"));
    f.feed(line(3, "connecting to AA:BB:CC:DD:EE:FF"));
    f.feed(line(4, "state Error: The device did not answer.", "error"));
    expect(f.session!.steps.connect.status).toBe("failed");
    expect(f.session!.steps.connect.detail).toBe("The device did not answer.");

    f.feed(line(10, "stream: device AA:BB:CC:DD:EE:FF"));
    expect(f.session!.id).toBe(2);
    f.feed(line(11, "state Streaming: Streaming"));
    f.feed(line(12, "connection lost, reconnecting", "warn"));
    expect(f.session!.steps.link.status).toBe("running");
    f.feed(line(13, "reconnected on attempt 1"));
    expect(f.session!.steps.link.status).toBe("warning");
    expect(f.session!.reconnects).toBe(1);
    f.feed(line(14, "streaming stopped, cleaning up"));
    expect(f.session!.stop_ms).toBe(14);
  });

  it("keeps the first connection's steps through a reconnect with a failed attempt", () => {
    const f = new FlowTracker();
    const at = (time_ms: number, target: string, message: string, level: LogLine["level"] = "info") =>
      f.feed({ time_ms, target, level, message });
    at(1, "a2dpwb_core::stream", "stream: device 58:18:62:02:7A:D7 codec LDAC");
    at(2, "bt_host", "HCI ready after 69 ms");
    at(3, "bt_host", "connecting to 58:18:62:02:7A:D7");
    at(4, "bt_host", "signaling connection established (cid 0x0041, handle 0x0001)");
    at(5, "a2dpwb_core::stream", "codec selected: LDAC");
    at(6, "audio_capture::loopback", 'audio capture: loopback of "CABLE Input": mix 96000 Hz 2 ch 32 bit');
    at(7, "a2dpwb_core::stream", "capture: 96000 Hz, 2 channel(s), F32 -> 2 channel(s)");
    at(8, "bt_host::app", "stream established (local 1, remote 3, mtu 895)");
    at(9, "a2dpwb_core::service", "state Streaming: Streaming");
    expect(f.session!.steps.capture.status).toBe("done");

    at(20, "a2dpwb_core::stream", "connection lost, reconnecting", "warn");
    at(21, "bt_host", "reconnecting to 58:18:62:02:7A:D7");
    at(40, "bt_host::app", "signaling connection failed (0x67)", "warn");
    at(40, "bt_host::app", "stream establishment failed (0x67)", "warn");
    at(40, "a2dpwb_core::stream", "reconnect attempt 1 failed", "warn");
    at(43, "bt_host", "reconnecting to 58:18:62:02:7A:D7");
    at(44, "a2dpwb_core::stream", "reconnected on attempt 2");
    const s = f.session!;
    expect(s.steps.connect.status).toBe("done");
    expect(s.steps.config.status).toBe("done");
    expect(s.steps.link.status).toBe("warning");
    expect(s.reconnects).toBe(1);
  });

  it("shows a connection that needed a second attempt as a warning", () => {
    const f = new FlowTracker();
    const at = (time_ms: number, target: string, message: string) => f.feed({ time_ms, target, level: "info", message });
    at(1, "a2dpwb_core::stream", "stream: device AA:BB:CC:DD:EE:FF");
    at(2, "bt_host", "connecting to AA:BB:CC:DD:EE:FF");
    at(3, "bt_host::app", "signaling connection failed (0x04)");
    at(4, "bt_host", "connection attempt 1/3 failed (PageTimeout), trying again");
    at(5, "bt_host::app", "signaling connection established (cid 0x0041, handle 0x000b)");
    expect(f.session!.steps.connect.status).toBe("warning");
    expect(f.session!.steps.connect.detail).toContain("after 1 failed attempt(s)");
  });

  it("shows the core's computed step details in the UI language", async () => {
    const live = { key: "dbgc.audio_live", args: ["120", "990"] };
    expect(stepDetail({ detail: "120 packets/s, 990 kbps", detail_msg: live })).toBe("120 packets/s, 990 kbps");
    expect(stepDetail({ detail: "reconnecting to AA:BB", detail_msg: null })).toBe("reconnecting to AA:BB");
    const reconnects = { key: "dbgc.link_reconnects", args: ["2"] };
    await setLanguage("ja");
    try {
      expect(stepDetail({ detail: "No disconnects", detail_msg: { key: "dbgc.link_stable", args: [] } })).toBe(t("dbgc.link_stable"));
      expect(t("dbgc.link_stable")).not.toBe("No disconnects");
      // What the core puts after its message stays
      expect(stepDetail({ detail: "2 reconnect(s) - reconnecting to AA:BB", detail_msg: reconnects })).toBe(
        `${t("dbgc.link_reconnects", "2")} - reconnecting to AA:BB`,
      );
    } finally {
      await setLanguage("en");
    }
  });
});

describe("profiles", () => {
  it("accepts the address forms v1 accepted", () => {
    expect(parseAddress("aa:bb:cc:dd:ee:ff")).toBe("AA:BB:CC:DD:EE:FF");
    expect(parseAddress("AA-BB-CC-DD-EE-FF")).toBe("AA:BB:CC:DD:EE:FF");
    expect(parseAddress("AABBCCDDEEFF")).toBe("AA:BB:CC:DD:EE:FF");
    expect(parseAddress("AA BB CC DD EE FF")).toBe("AA:BB:CC:DD:EE:FF");
    expect(parseAddress("AA:BB:CC:DD:EE")).toBeNull();
    expect(parseAddress("GG:BB:CC:DD:EE:FF")).toBeNull();
  });

  it("validates before saving", () => {
    const ok = sampleProfile();
    expect(validateProfile(ok, [])).toEqual({});
    expect(validateProfile({ ...ok, name: "  " }, []).name?.key).toBe("ui.profile.err_name_empty");
    expect(validateProfile(ok, [ok.name]).name?.key).toBe("ui.profile.err_name_taken");
    expect(validateProfile({ ...ok, device_address: "" }, []).device?.key).toBe("ui.profile.err_device");
    expect(validateProfile({ ...ok, device_address: "12:34" }, []).device?.key).toBe("error.invalid_address_format");
    expect(validateProfile({ ...ok, max_media_payload: 600 }, []).packet?.args).toEqual([679, 1679]);
    expect(validateProfile({ ...ok, max_media_payload: 1680 }, []).packet).toBeDefined();
    expect(validateProfile({ ...ok, capture_mode: "virtual" }, []).audio_device?.key).toBe("error.no_virtual_device_selected");
    expect(validateProfile({ ...ok, capture_mode: "app", app_exe: "" }, []).app?.key).toBe("error.no_app_selected");
  });

  it("drops what the new codec cannot do", () => {
    const p = { ...sampleProfile(), codec: "lhdcv5", quality: "mhq", abr: true, sample_rate: 96000, bit_depth: 24 };
    expect(fitToCodec({ ...p, codec: "ldac" })).toMatchObject({ quality: "hq", abr: true, sample_rate: 96000, bit_depth: 24 });
    expect(fitToCodec({ ...p, codec: "aac" })).toMatchObject({ abr: false, sample_rate: 0, bit_depth: 0 });
    expect(fitToCodec({ ...p, codec: "auto" })).toMatchObject({ abr: true, sample_rate: 0, bit_depth: 24 });
    expect(fitToCodec({ ...p, codec: "aptx" })).toMatchObject({ quality: "mhq", abr: false });
    expect(fitToCodec({ ...p, codec: "opus" })).toMatchObject({ quality: "hq", abr: false, sample_rate: 0, bit_depth: 0 });
    expect(fitToCodec({ ...p, codec: "opus", sample_rate: 48000 }).sample_rate).toBe(48000);
    expect(fitToCodec({ ...p, codec: "opus", sample_rate: 44100 }).sample_rate).toBe(0);
    expect(sampleRates("opus")).toEqual([0, 48000]);
    expect(fitToCodec({ ...p, codec: "opus05", sample_rate: 44100 })).toMatchObject({ quality: "hq", abr: false, sample_rate: 0, bit_depth: 0 });
    expect(sampleRates("opus05")).toEqual([0, 48000]);
  });

  it("offers 192 kHz for LHDC V5 only, and no quality under its ABR", () => {
    expect(sampleRates("lhdcv5")).toEqual([0, 44100, 48000, 88200, 96000, 192000]);
    expect(sampleRates("ldac")).not.toContain(192000);
    const p = { ...sampleProfile(), codec: "lhdcv5", sample_rate: 192000 };
    expect(fitToCodec(p).sample_rate).toBe(192000);
    expect(fitToCodec({ ...p, codec: "ldac" }).sample_rate).toBe(0);
    // LHDC V5 ABR stays at 400 kbps or below whatever the quality (as on Android)
    expect(qualityIgnored("lhdcv5", true)).toBe(true);
    expect(qualityIgnored("lhdcv5", false)).toBe(false);
    expect(qualityIgnored("ldac", true)).toBe(false);
  });

  it("says why the bit rate is below the profile's", () => {
    expect(bitrateNoteKey({ abr: true, bitrate_capped: false })).toBe("quality.codec_abr");
    expect(bitrateNoteKey({ abr: false, bitrate_capped: true })).toBe("quality.codec_capped");
    expect(bitrateNoteKey({ abr: true, bitrate_capped: true })).toBe("quality.codec_abr");
    expect(bitrateNoteKey({ abr: false, bitrate_capped: false })).toBeNull();
  });

  it("names codecs as the core reports them", () => {
    expect(codecLabelKey("aptX HD")).toBe("codec.aptx_hd");
    expect(codecLabelKey("LHDC V5")).toBe("codec.lhdc_v5");
    expect(codecLabelKey("Opus")).toBe("codec.opus");
    expect(codecLabelKey("Opus 05")).toBe("codec.opus05");
    expect(codecLabelKey("SBC XQ")).toBe("codec.sbc_xq");
    expect(codecLabelKey("FastStream")).toBe("codec.faststream");
    expect(codecLabelKey("Ldac")).toBe("codec.ldac");
  });

  it("masks the device part of addresses", () => {
    expect(maskAddresses("connecting to AC:80:0A:12:34:56 now")).toBe("connecting to AC:80:0A:xx:xx:xx now");
  });
});

describe("about", () => {
  it("links the third-party licences of the release tag", () => {
    expect(thirdPartyUrl("2.0.0-alpha.1")).toBe(
      "https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/v2.0.0-alpha.1/THIRD_PARTY_LICENSES.md",
    );
  });
});

describe("profile list", () => {
  it("shows the quality and its bit rate as v1 did", () => {
    const tag = (over: Parameters<typeof sampleProfile>[0]) => qualityTag(sampleProfile(over), t);
    expect(tag({ codec: "ldac", quality: "hq" })).toBe("High (990 kbps)");
    expect(tag({ codec: "ldac", quality: "sq", sample_rate: 44100 })).toBe("Standard (606 kbps)");
    expect(tag({ codec: "ldac", quality: "mq", sample_rate: 88200 })).toBe("Mobile (303 kbps)");
    expect(tag({ codec: "lhdcv5", quality: "sq", sample_rate: 44100 })).toBe("Standard (480 kbps)");
    expect(tag({ codec: "lhdcv5", quality: "sq", sample_rate: 88200 })).toBe("Standard (500 kbps)");
    expect(tag({ codec: "lhdcv5", quality: "mhq" })).toBe("Medium-High (900 kbps)");
    expect(tag({ codec: "lhdcv5", quality: "hq", abr: true })).toBe("ABR (≤400 kbps)");
    expect(tag({ codec: "sbc", quality: "sq" })).toBe("Standard (~240 kbps)");
    expect(tag({ codec: "auto", quality: "hq" })).toBe("High");
    // No quality choice
    expect(tag({ codec: "aptxhd" })).toBeNull();
    expect(tag({ codec: "faststream" })).toBeNull();
    expect(tag({ codec: "auto", le_audio: true })).toBeNull();
  });
});
