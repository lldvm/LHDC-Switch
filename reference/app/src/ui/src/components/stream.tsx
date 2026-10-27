// The streaming controls shared by Home and the flyout: the connection
// ring, the Connect / Disconnect button, volume, codec badge, link quality
// and the error panel with its fix.

import { createMemo, createSignal, onCleanup, Show, type JSX } from "solid-js";
import { errorText, useApp } from "../app/context";
import { t, tm } from "../i18n";
import { bitrateNoteKey, codecLabelKey } from "../lib/codecs";
import { copyText, khz, kbps, percentToVolume, volumePercent } from "../lib/format";
import { firmwareUrl } from "../lib/links";
import type { AdapterInfo, LinkSample, Msg, Profile, Snapshot, SourceInfo, StreamInfo, StreamState } from "../ipc/types";
import { Sparkline } from "./charts";
import { Banner, Button, Dialog } from "./controls";
import { Icon } from "./Icon";
import type { IconName } from "./icons";

/** The profile the big button connects: the one streaming, else the active one */
export function currentProfile(snap: Snapshot): Profile | null {
  const name = snap.stream.state !== "idle" ? (snap.stream.profile ?? snap.active_profile) : snap.active_profile;
  return snap.profiles.find((p) => p.name === name) ?? snap.profiles[0] ?? null;
}

export function stateLabel(s: StreamState): string {
  return t(`status.${s}`);
}

/** The ring around the device: pulses while connecting, glows while streaming */
export function Ring(props: { state: StreamState; volume: number | null; size?: "big" | "small"; children: JSX.Element }) {
  const glow = () => (props.volume === null ? 0.6 : 0.35 + (0.65 * props.volume) / 127);
  return (
    <div
      class={`ring ring-${props.size ?? "big"} ring-${props.state}`}
      style={{ "--glow": String(glow()) }}
      aria-hidden="true"
    >
      <div class="ring-inner">{props.children}</div>
    </div>
  );
}

/** Connect / Disconnect for the current profile; reports a failed request through onError */
export function ConnectButton(props: { onError?: (e: unknown) => void; compact?: boolean }) {
  const { core } = useApp();
  const [busy, setBusy] = createSignal(false);
  const snap = () => core.state.snap!;
  const state = () => snap().stream.state;
  const profile = () => currentProfile(snap());
  const active = () => state() === "streaming" || state() === "connecting" || state() === "reconnecting";
  const label = () =>
    state() === "streaming"
      ? t("connection.disconnect")
      : state() === "connecting" || state() === "reconnecting"
        ? t("modal.cancel")
        : t("connection.connect");
  const press = async () => {
    setBusy(true);
    try {
      if (active()) await core.request("stream.stop");
      else if (profile()) await core.request("stream.start", { profile: profile()!.name });
    } catch (e) {
      props.onError?.(e);
    } finally {
      setBusy(false);
    }
  };
  return (
    <button
      type="button"
      class={`connect-btn ${props.compact ? "compact" : ""}`}
      classList={{ on: active() }}
      disabled={busy() || !profile()}
      aria-busy={busy() ? "true" : undefined}
      onClick={press}
    >
      <Icon name={active() ? "plug_disconnected" : "power"} />
      <span>{label()}</span>
    </button>
  );
}

export function VolumeControl() {
  const { core } = useApp();
  const vol = () => core.state.snap?.stream.volume ?? null;
  // The value under the pointer while dragging, so the core's echo of an
  // older value does not make the thumb jump back
  const [held, setHeld] = createSignal<number | null>(null);
  const shown = () => held() ?? (vol() === null ? 0 : volumePercent(vol()!));
  let lastSent = 0;
  let pending: ReturnType<typeof setTimeout> | undefined;
  let release: ReturnType<typeof setTimeout> | undefined;
  onCleanup(() => {
    clearTimeout(pending);
    clearTimeout(release);
  });
  const send = (pct: number) => {
    // At most ten requests a second while dragging, and the final value
    clearTimeout(pending);
    const go = () => {
      lastSent = Date.now();
      core.request("stream.set_volume", { volume: percentToVolume(pct) }).catch(() => {});
    };
    const wait = 100 - (Date.now() - lastSent);
    if (wait <= 0) go();
    else pending = setTimeout(go, wait);
  };
  const disabled = () => vol() === null;
  return (
    <div class="volume" classList={{ disabled: disabled() }} title={t("tooltip.volume")}>
      <Icon name={shown() === 0 ? "speaker_mute" : "speaker_2"} />
      <input
        type="range"
        min="0"
        max="100"
        step="1"
        value={shown()}
        disabled={disabled()}
        aria-label={t("volume.label")}
        aria-valuetext={disabled() ? t("ui.volume.unknown") : `${shown()}%`}
        style={{ "--fill": `${shown()}%` }}
        onInput={(e) => {
          const v = Number(e.currentTarget.value);
          setHeld(v);
          clearTimeout(release);
          send(v);
        }}
        onChange={(e) => {
          send(Number(e.currentTarget.value));
          clearTimeout(release);
          release = setTimeout(() => setHeld(null), 600);
        }}
      />
      <span class="num volume-value">{disabled() ? "–" : `${shown()}%`}</span>
    </div>
  );
}

export function CodecBadge(props: { compact?: boolean }) {
  const { core } = useApp();
  const info = () => core.state.snap?.stream.info ?? null;
  const name = () => {
    const i = info();
    if (!i) return "";
    const key = codecLabelKey(i.codec);
    return key ? t(key) : i.codec;
  };
  // "400 kbps — ABR on": why the bit rate is below the profile's
  const bitrateNote = (i: StreamInfo) => {
    const key = bitrateNoteKey(i);
    return key ? `${i.bitrate_kbps} kbps${t(key)}` : undefined;
  };
  const fallbackHelp = () => {
    const i = info();
    if (!i?.requested_codec) return "";
    const req = codecLabelKey(i.requested_codec);
    return t("tooltip.codec_fallback_help", req ? t(req) : i.requested_codec, name());
  };
  return (
    <Show when={info()} fallback={<span class="codec-badge idle">{t("ui.codec.none")}</span>}>
      {(i) => (
        <span class="codec-badge" title={fallbackHelp() || undefined}>
          <Show when={i().requested_codec}>
            <Icon name="warning" class="st-warn" title={fallbackHelp()} />
          </Show>
          <strong>{name()}</strong>
          <span class="num" title={bitrateNote(i())}>
            {i().bitrate_kbps} kbps
          </span>
          <Show when={i().abr}>
            <span class="muted" title={t("tooltip.abr")}>
              ABR
            </span>
          </Show>
          <Show when={!props.compact}>
            <span class="num">{khz(i().sample_rate)} kHz</span>
          </Show>
        </span>
      )}
    </Show>
  );
}

/** What a running stream captures (StreamInfo.source), in words: the
 *  profile's application name when the executable is the profile's */
export function sourceText(src: SourceInfo, profile: Profile | null): { icon: IconName; text: string; waiting: boolean } {
  switch (src.kind) {
    case "tone":
      return { icon: "music_note_2", text: t("receiver.source_tone"), waiting: false };
    case "virtual":
      return { icon: "desktop_speaker", text: src.name || t("capture.virtual"), waiting: false };
    case "app": {
      const known = profile && profile.app_exe && profile.app_exe.toLowerCase() === src.name.toLowerCase() ? profile.app_name : "";
      const name = known || src.name || t("capture.app");
      return { icon: "window_apps", text: src.waiting ? t("ui.source.waiting", name) : name, waiting: src.waiting };
    }
    default:
      return { icon: "speaker_2", text: t("capture.loopback"), waiting: false };
  }
}

/** The capture source of the stream on screen: the core's while it
 *  streams, otherwise null */
export function CaptureSource(props: { profile: Profile | null; class?: string }) {
  const { core } = useApp();
  const src = () => core.state.snap?.stream.info?.source ?? null;
  return (
    <Show when={src()}>
      {(s) => {
        const v = () => sourceText(s(), props.profile);
        return (
          <span class={`capture-source ${props.class ?? ""}`} classList={{ "st-warn": v().waiting }} title={`${t("capture.mode")}: ${v().text}`}>
            <Icon name={v().waiting ? "warning" : v().icon} />
            <span class="ellipsis">{v().text}</span>
          </span>
        );
      }}
    </Show>
  );
}

export type QualityLevel = 0 | 1 | 2 | 3 | 4;

/** Samples (at 4 a second) the quality looks back over for drops: 10 s */
const QUALITY_DROP_SAMPLES = 40;
/** ... and for a send queue backing up: 4 s */
const QUALITY_QUEUE_SAMPLES = 16;
/** Packets in the send queue (of 64) that mean the radio fell behind */
const QUALITY_QUEUE_BACKLOG = 32;

/** Link quality from the last samples: what reaches the headphones (drops,
 *  the send queue backing up, air time). Not RSSI: controllers report it
 *  differently (vs. the golden range or not) and the headphones' power
 *  control moves it by 10-20 dB on a link that delivers everything. */
export function linkQuality(samples: readonly LinkSample[]): { level: QualityLevel; status: "good" | "warn" | "bad" } | null {
  if (samples.length === 0) return null;
  const last = samples[samples.length - 1];
  if (last.bytes_per_second === 0 && last.rssi === null) return null;
  const caps: QualityLevel[] = [4];
  const old = samples[Math.max(0, samples.length - QUALITY_DROP_SAMPLES)];
  const dropped = last.dropped - old.dropped + (last.capture_dropped - old.capture_dropped);
  if (dropped > 5) caps.push(1);
  else if (dropped > 0) caps.push(2);
  const backlog = Math.max(...samples.slice(-QUALITY_QUEUE_SAMPLES).map((s) => s.queue_depth));
  if (backlog >= QUALITY_QUEUE_BACKLOG) caps.push(2);
  else if (backlog >= QUALITY_QUEUE_BACKLOG / 2) caps.push(3);
  if (last.airtime > 0.8) caps.push(2);
  const level = Math.min(...caps) as QualityLevel;
  return { level, status: level >= 3 ? "good" : level === 2 ? "warn" : "bad" };
}

export function QualityIndicator() {
  const { core } = useApp();
  const q = createMemo(() => linkQuality(core.link()));
  const label = () => {
    const v = q();
    if (!v) return t("ui.quality.unknown");
    return v.status === "good" ? t("ui.quality.good") : v.status === "warn" ? t("ui.quality.fair") : t("ui.quality.poor");
  };
  return (
    <span class={`quality ${q() ? `st-${q()!.status}` : ""}`} title={`${t("quality.title")}: ${label()}`}>
      <span class="bars" aria-hidden="true">
        {[1, 2, 3, 4].map((n) => (
          <span class="bar" classList={{ lit: (q()?.level ?? 0) >= n }} style={{ height: `${n * 25}%` }} />
        ))}
      </span>
      <span class="quality-label">{label()}</span>
    </span>
  );
}

/** Bandwidth over the last minute, while the caller is on screen */
export function LinkSparkline() {
  const { core } = useApp();
  const release = core.wantLink();
  onCleanup(release);
  const values = createMemo(() => core.link().map((s) => kbps(s.bytes_per_second)));
  const latest = () => {
    const v = values();
    return v.length ? Math.round(v[v.length - 1]) : null;
  };
  return (
    <div class="link-mini">
      <Sparkline values={values()} min={0} label={`${t("quality.bitrate")}: ${latest() ?? "–"} kbps`} />
      <span class="num link-mini-value">{latest() === null ? "–" : `${latest()} kbps`}</span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Errors and their fixes
// ---------------------------------------------------------------------------

export interface Fix {
  label: string;
  icon: IconName;
  run: () => void | Promise<unknown>;
}

const RETRY_KEYS = [
  "error.connect_no_answer",
  "error.connect_link_lost",
  "error.connect_refused",
  "error.connect_auth_failed",
  "error.reconnect_failed",
  "error.pairing_hint",
  "error.stream_start",
];
const PROFILE_KEYS = [
  "error.codec_not_supported",
  "error.codec_adaptive_only",
  "error.codec_sep_in_use",
  "error.no_compatible_codec",
  "error.no_virtual_device_selected",
  "error.virtual_capture_init",
  "error.no_app_selected",
  "error.app_capture_unsupported",
  "error.app_capture_init",
  "error.aac_unavailable",
];

/** The adapter A2DPWB would use but cannot: not on WinUSB */
export function adapterNeedingDriver(snap: Snapshot): AdapterInfo | null {
  return snap.adapter.adapters.find((a) => a.driver !== "winusb") ?? null;
}

/** The next step for an error: a code from a request, and/or the message key */
export function useFixes() {
  const app = useApp();
  const { core, navigate } = app;
  const [winusbFor, setWinusbFor] = createSignal<AdapterInfo | null>(null);

  const fixFor = (code: string | null, key: string): Fix | null => {
    const snap = core.state.snap;
    if (!snap) return null;
    if (code === "adapter.driver_missing" || key === "error.btstack_init" || key === "firmware.auto_no_adapter") {
      const a = adapterNeedingDriver(snap);
      if (a) return { label: t("ui.adapter.assign_winusb"), icon: "shield", run: () => void setWinusbFor(a) };
      return { label: t("ui.nav.adapter"), icon: "usb_stick", run: () => navigate({ name: "adapter" }) };
    }
    if (code === "adapter.firmware_missing" || key.startsWith("firmware.")) {
      const vendor = snap.adapter.adapters[0]?.vendor ?? "";
      return { label: t("firmware.open_download_page"), icon: "open", run: () => core.transport.openUrl(firmwareUrl(vendor)) };
    }
    if (key === "error.btstack_no_answer" || key.startsWith("error.btstack_crash")) {
      return { label: t("ui.adapter.restart"), icon: "arrow_reset", run: () => core.request("adapter.restart") };
    }
    if (RETRY_KEYS.includes(key)) {
      const p = currentProfile(snap);
      if (p) return { label: t("ui.fix.retry"), icon: "arrow_sync", run: () => core.request("stream.start", { profile: p.name }) };
    }
    if (PROFILE_KEYS.includes(key)) {
      const p = currentProfile(snap);
      return {
        label: t("profile.edit"),
        icon: "edit",
        run: () => navigate(p ? { name: "profiles", arg: p.name } : { name: "profiles" }),
      };
    }
    return { label: t("ui.nav.diagnostics"), icon: "pulse", run: () => navigate({ name: "diagnostics" }) };
  };

  const dialog = () => <WinusbDialog adapter={winusbFor()} mode="assign" onClose={() => setWinusbFor(null)} />;
  return { fixFor, dialog };
}

/** The text "copy the error" puts on the clipboard: the message, and the
 *  code of a failed request (what an issue report needs) */
export function errorCopyText(message: Msg, code?: string | null): string {
  const text = tm(message) || message.key;
  return code ? `${text}\n[${code}]` : text;
}

/** Copies an error's text (v1 copied the status text on click) */
export async function copyError(app: { notify: (text: string, kind?: "info" | "error") => void }, message: Msg, code?: string | null) {
  const ok = await copyText(errorCopyText(message, code));
  app.notify(ok ? t("ui.error.copied") : t("dbgc.copy_failed"), ok ? "info" : "error");
}

/** An error with its cause and the button that fixes it */
export function ErrorPanel(props: { code?: string | null; message: Msg; title?: string }) {
  const app = useApp();
  const { fixFor, dialog } = useFixes();
  const fix = () => fixFor(props.code ?? null, props.message.key);
  const [busy, setBusy] = createSignal(false);
  const run = async () => {
    const f = fix();
    if (!f) return;
    setBusy(true);
    await app.run(async () => f.run());
    setBusy(false);
  };
  return (
    <>
      <Banner
        kind="error"
        title={props.title ?? t("status.error")}
        actions={
          <>
            <Show when={fix()}>
              {(f) => (
                <Button kind="strong" icon={f().icon} busy={busy()} onClick={run}>
                  {f().label}
                </Button>
              )}
            </Show>
            <Button
              kind="subtle"
              icon="copy"
              title={t("error.click_to_copy_hint")}
              onClick={() => void copyError(app, props.message, props.code)}
            >
              {t("ui.error.copy")}
            </Button>
          </>
        }
      >
        {tm(props.message)}
      </Banner>
      {dialog()}
    </>
  );
}

/**
 * Assign WinUSB / restore the driver, after saying what it does: while
 * A2DPWB has the adapter, Windows' Bluetooth does not see it.
 */
export function WinusbDialog(props: { adapter: AdapterInfo | null; mode: "assign" | "restore"; onClose: (done: boolean) => void }) {
  const app = useApp();
  const [busy, setBusy] = createSignal(false);
  const go = async () => {
    const a = props.adapter;
    if (!a) return;
    setBusy(true);
    try {
      if (props.mode === "assign") await app.core.request("adapter.assign_winusb", { instance_id: a.instance_id });
      else await app.core.request("adapter.restore_driver", { instance_id: a.instance_id });
      app.notify(props.mode === "assign" ? t("ui.adapter.assigned", a.display_name) : t("ui.adapter.restored", a.display_name));
      props.onClose(true);
    } catch (e) {
      app.notify(errorText(e), "error");
      props.onClose(false);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      open={props.adapter !== null}
      title={props.mode === "assign" ? t("ui.adapter.assign_title") : t("ui.adapter.restore_title")}
      confirm={props.mode === "assign" ? t("ui.adapter.assign_winusb") : t("ui.adapter.restore_driver")}
      cancel={t("modal.cancel")}
      busy={busy()}
      danger={props.mode === "restore"}
      onConfirm={go}
      onCancel={() => props.onClose(false)}
    >
      <p>
        <strong>{props.adapter?.display_name}</strong>
        <span class="muted"> · {props.adapter?.device_name}</span>
      </p>
      <p>{props.mode === "assign" ? t("ui.adapter.assign_explain") : t("ui.adapter.restore_explain")}</p>
      <p class="muted">{t("ui.adapter.uac_note")}</p>
    </Dialog>
  );
}

