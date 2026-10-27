// The codecs a profile can ask for and what each allows, as v1's profile
// dialog (wx_profile_dialog.cpp on the develop branch, update_codec_dependent): quality
// modes with their bitrates, ABR only for LDAC / LHDC V5 (and Auto, which may
// pick them), 24-bit and 88.2 / 96 kHz only for LDAC and LHDC V5, 192 kHz and
// "mhq" only for LHDC V5, no quality choice for the aptX family or under LHDC
// V5 ABR, 48 kHz only for Opus and Opus 05.

export type CodecId =
  | "auto"
  | "ldac"
  | "aptxhd"
  | "aptxll"
  | "aptx"
  | "lhdcv5"
  | "opus"
  | "aac"
  | "sbcxq"
  | "sbc"
  | "faststream"
  | "opus05";

export interface QualityMode {
  id: string;
  /** Translation key of the name */
  label: string;
  kbps?: string;
}

export interface CodecSpec {
  id: CodecId;
  /** Translation key of the name */
  label: string;
  /** 1..5 for the comparison bars (Auto has none: it depends on the device) */
  bars?: { quality: number; latency: number; stability: number };
  /** Quality modes; empty: the codec has a fixed bitrate */
  qualities: QualityMode[];
  abr: boolean;
  /** 24-bit and 88.2 / 96 kHz */
  hiRes: boolean;
  /** The only capture rates it takes (0 = auto), when not the usual ones */
  rates?: number[];
}

const HQ = "quality.high";
const MHQ = "quality.medium_high";
const SQ = "quality.standard";
const MQ = "quality.mobile";

export const CODECS: CodecSpec[] = [
  {
    id: "auto",
    label: "codec.auto",
    qualities: [
      { id: "hq", label: HQ },
      { id: "sq", label: SQ },
      { id: "mq", label: MQ },
    ],
    abr: true,
    hiRes: false,
  },
  {
    id: "ldac",
    label: "codec.ldac",
    bars: { quality: 5, latency: 2, stability: 2 },
    qualities: [
      { id: "hq", label: HQ, kbps: "990" },
      { id: "sq", label: SQ, kbps: "660" },
      { id: "mq", label: MQ, kbps: "330" },
    ],
    abr: true,
    hiRes: true,
  },
  {
    id: "lhdcv5",
    label: "codec.lhdc_v5",
    bars: { quality: 5, latency: 2, stability: 2 },
    // Android's four fixed levels (the sink may cap the maximum)
    qualities: [
      { id: "hq", label: HQ, kbps: "1000" },
      { id: "mhq", label: MHQ, kbps: "900" },
      { id: "sq", label: SQ, kbps: "500" },
      { id: "mq", label: MQ, kbps: "400" },
    ],
    abr: true,
    hiRes: true,
    rates: [0, 44100, 48000, 88200, 96000, 192000],
  },
  { id: "aptxhd", label: "codec.aptx_hd", bars: { quality: 4, latency: 3, stability: 3 }, qualities: [], abr: false, hiRes: false },
  { id: "aptx", label: "codec.aptx", bars: { quality: 3, latency: 3, stability: 4 }, qualities: [], abr: false, hiRes: false },
  { id: "aptxll", label: "codec.aptx_ll", bars: { quality: 2, latency: 5, stability: 4 }, qualities: [], abr: false, hiRes: false },
  {
    id: "opus",
    label: "codec.opus",
    bars: { quality: 4, latency: 3, stability: 4 },
    // Android's A2DP Opus: 256 kbps stereo (mono: half), 48 kHz, 20 ms frames
    qualities: [
      { id: "hq", label: HQ, kbps: "256" },
      { id: "sq", label: SQ, kbps: "192" },
      { id: "mq", label: MQ, kbps: "128" },
    ],
    abr: false,
    hiRes: false,
    rates: [0, 48000],
  },
  {
    id: "aac",
    label: "codec.aac",
    bars: { quality: 3, latency: 2, stability: 4 },
    qualities: [
      { id: "hq", label: HQ, kbps: "256" },
      { id: "sq", label: SQ, kbps: "192" },
      { id: "mq", label: MQ, kbps: "128" },
    ],
    abr: false,
    hiRes: false,
  },
  {
    // Dual channel, high bitpool (PipeWire's SBC-XQ); only on sinks whose
    // SBC allows it
    id: "sbcxq",
    label: "codec.sbc_xq",
    bars: { quality: 3, latency: 3, stability: 3 },
    qualities: [
      { id: "hq", label: HQ, kbps: "~552" },
      { id: "sq", label: SQ, kbps: "~505" },
      { id: "mq", label: MQ, kbps: "~385" },
    ],
    abr: false,
    hiRes: false,
  },
  {
    id: "sbc",
    label: "codec.sbc",
    bars: { quality: 2, latency: 3, stability: 5 },
    qualities: [
      { id: "hq", label: HQ, kbps: "~345" },
      { id: "sq", label: SQ, kbps: "~240" },
      { id: "mq", label: MQ, kbps: "~150" },
    ],
    abr: false,
    hiRes: false,
  },
  // CSR FastStream: fixed SBC (bitpool 29, ~200 kbps), music only; never
  // picked by Auto
  { id: "faststream", label: "codec.faststream", bars: { quality: 1, latency: 4, stability: 4 }, qualities: [], abr: false, hiRes: false },
  {
    // PipeWire's own Opus ("Opus 05"; experimental, never picked by Auto):
    // Android Opus' levels, stereo, 48 kHz, 10 ms frames when offered
    id: "opus05",
    label: "codec.opus05",
    bars: { quality: 4, latency: 3, stability: 3 },
    qualities: [
      { id: "hq", label: HQ, kbps: "256" },
      { id: "sq", label: SQ, kbps: "192" },
      { id: "mq", label: MQ, kbps: "128" },
    ],
    abr: false,
    hiRes: false,
    rates: [0, 48000],
  },
];

export function codecSpec(id: string): CodecSpec {
  return CODECS.find((c) => c.id === id) ?? CODECS[0];
}

/** Translation key of a codec name as the core reports it ("LDAC", "aptX HD") or a profile id */
export function codecLabelKey(name: string): string {
  const n = name.toLowerCase().replace(/[\s_-]/g, "");
  const map: Record<string, string> = {
    auto: "codec.auto",
    ldac: "codec.ldac",
    aptx: "codec.aptx",
    aptxhd: "codec.aptx_hd",
    aptxll: "codec.aptx_ll",
    aptxlowlatency: "codec.aptx_ll",
    aac: "codec.aac",
    sbc: "codec.sbc",
    sbcxq: "codec.sbc_xq",
    faststream: "codec.faststream",
    lhdcv5: "codec.lhdc_v5",
    lhdc: "codec.lhdc_v5",
    opus: "codec.opus",
    opus05: "codec.opus05",
    // LE Audio (StreamInfo.le)
    lc3: "ui.codec.lc3",
  };
  return map[n] ?? "";
}

/**
 * Whether a device offers a codec, from the codec keys the core learnt when
 * it last connected (Device.codecs: "ldac", "aac", ...; empty: not known
 * until the first connection). Auto takes what the device offers.
 */
export type CodecSupport = "offered" | "not_offered" | "unknown";

export function codecSupport(codec: string, known: readonly string[]): CodecSupport {
  if (known.length === 0) return "unknown";
  if (codec === "auto") return "offered";
  const id = codec.toLowerCase();
  return known.some((k) => k.toLowerCase() === id) ? "offered" : "not_offered";
}

export function sampleRates(codec: string): number[] {
  const spec = codecSpec(codec);
  return spec.rates ?? (spec.hiRes ? [0, 44100, 48000, 88200, 96000] : [0, 44100, 48000]);
}

/** LHDC V5 ABR runs at 400 kbps or below whatever the quality (as on
 *  Android): the quality choice is greyed out then */
export function qualityIgnored(codec: string, abr: boolean): boolean {
  return codec === "lhdcv5" && abr;
}

/** LDAC's rates at 44.1 / 88.2 kHz, LHDC V5's Standard at 44.1 kHz (88.2
 *  kHz runs as 96 kHz there): v1's profile list */
const RATE_44K: Partial<Record<CodecId, Record<string, string>>> = {
  ldac: { hq: "909", sq: "606", mq: "303" },
  lhdcv5: { sq: "480" },
};

/**
 * The quality of a profile as the profile list shows it (v1's
 * rebuild_profile_list): "High (990 kbps)", "ABR (≤400 kbps)" under LHDC V5
 * ABR; null when the codec has no quality choice (aptX family, FastStream,
 * LE Audio). `label` translates the quality's key.
 */
export function qualityTag(
  p: { codec: string; quality: string; abr: boolean; sample_rate: number; le_audio?: boolean },
  label: (key: string) => string,
): string | null {
  if (p.le_audio) return null;
  const spec = codecSpec(p.codec);
  if (spec.qualities.length === 0) return null;
  if (qualityIgnored(p.codec, p.abr)) return "ABR (≤400 kbps)";
  const q = spec.qualities.find((x) => x.id === p.quality) ?? spec.qualities[0];
  const is441 = p.codec === "lhdcv5" ? p.sample_rate === 44100 : p.sample_rate === 44100 || p.sample_rate === 88200;
  const kbps = (is441 ? RATE_44K[spec.id]?.[q.id] : undefined) ?? q.kbps;
  return kbps ? `${label(q.label)} (${kbps} kbps)` : label(q.label);
}

/** Why a stream's bit rate is below the profile's, appended to the codec
 *  line: ABR (LHDC V5 stays at 400 kbps or below), or the device's maximum */
export function bitrateNoteKey(info: { abr: boolean; bitrate_capped: boolean }): string | null {
  if (info.abr) return "quality.codec_abr";
  if (info.bitrate_capped) return "quality.codec_capped";
  return null;
}

/** 0 = auto. Auto may pick LDAC / LHDC V5, so it offers 16 / 24 too (the
 *  core carries 24-bit only when the codec it picks has it) */
export function bitDepths(codec: string): number[] {
  const spec = codecSpec(codec);
  return spec.hiRes || spec.id === "auto" ? [0, 16, 24] : [0];
}

interface CodecFields {
  codec: string;
  quality: string;
  abr: boolean;
  sample_rate: number;
  bit_depth: number;
}

/** Values the codec cannot use go back to the defaults (v1 did the same when the codec changed) */
export function fitToCodec<T extends CodecFields>(p: T): T {
  const spec = codecSpec(p.codec);
  const quality = spec.qualities.length === 0 || spec.qualities.some((m) => m.id === p.quality) ? p.quality : "hq";
  return {
    ...p,
    quality,
    abr: spec.abr ? p.abr : false,
    sample_rate: sampleRates(p.codec).includes(p.sample_rate) ? p.sample_rate : 0,
    bit_depth: bitDepths(p.codec).includes(p.bit_depth) ? p.bit_depth : 0,
  };
}

// Max media packet size (crates/bt-host/src/media.rs, MEDIA_PAYLOAD_LIMIT_MIN / MAX / DEFAULT)
export const PACKET_MIN = 679;
export const PACKET_MAX = 1679;
export const PACKET_DEFAULT = 1023;
