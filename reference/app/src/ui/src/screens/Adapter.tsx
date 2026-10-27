// Adapter: the USB Bluetooth adapters with their driver and firmware,
// WinUSB assign / restore, the chip for the firmware, and AFH.

import { createEffect, createSignal, For, type JSX, onCleanup, onMount, Show } from "solid-js";
import { type AppContext, useApp } from "../app/context";
import { saveSettings } from "../app/settings";
import { Banner, Button, Field, Section, Segmented, Select } from "../components/controls";
import { Icon } from "../components/Icon";
import { ErrorPanel, WinusbDialog } from "../components/stream";
import { t, tm } from "../i18n";
import type { AdapterInfo, DriverKind, FirmwareFile } from "../ipc/types";
import { adapterFirmwareTag, buildChipOptions, type ChipStatus, chipStatus, isAutoChip } from "../lib/chips";
import { LINKS, vendorFirmwareUrl } from "../lib/links";

const hex4 = (n: number) => n.toString(16).toUpperCase().padStart(4, "0");

export function driverText(d: DriverKind): { text: string; status: "good" | "warn" | "bad" } {
  switch (d) {
    case "winusb":
      return { text: t("ui.adapter.driver_winusb"), status: "good" };
    case "bluetooth":
      return { text: t("ui.adapter.driver_bluetooth"), status: "warn" };
    case "other":
      return { text: t("ui.adapter.driver_other"), status: "warn" };
    default:
      return { text: t("ui.adapter.driver_none"), status: "bad" };
  }
}

/** Opens the settings folder (the shell creates it if it is not there yet) */
function openConfigFolder(app: AppContext) {
  return app.run(() => app.core.transport.openFolder(app.core.state.snap!.config_dir));
}

export function Adapter() {
  const app = useApp();
  const { core } = app;
  const snap = () => core.state.snap!;
  const adapter = () => snap().adapter;
  const [dialog, setDialog] = createSignal<{ adapter: AdapterInfo; mode: "assign" | "restore" } | null>(null);
  const [listing, setListing] = createSignal(false);
  // The *_fw.bin files in the settings folder: chips the table lacks
  const [files, setFiles] = createSignal<FirmwareFile[]>([]);
  const loadFiles = async () => {
    try {
      setFiles(await core.request("adapter.firmware_files"));
    } catch {
      // The choice keeps the table's chips
    }
  };

  const refresh = async () => {
    setListing(true);
    await Promise.all([app.run(() => core.request("adapter.list")), loadFiles()]);
    setListing(false);
  };

  // v1 checked the firmware again when its dialog got the focus back: the
  // files may have been placed through Open Config Folder meanwhile
  const onFocus = () => {
    if (listing()) return;
    void core.request("adapter.list").catch(() => {});
    void loadFiles();
  };
  onMount(() => {
    void loadFiles();
    window.addEventListener("focus", onFocus);
  });
  onCleanup(() => window.removeEventListener("focus", onFocus));
  // #/adapter/afh (the receiver test links there): the AFH section in view
  createEffect(() => {
    if (app.route().arg === "afh") document.getElementById(AFH_SECTION)?.scrollIntoView?.({ block: "start" });
  });

  return (
    <div class="page">
      <header class="page-head">
        <h1>{t("ui.nav.adapter")}</h1>
        <div class="head-actions">
          <Button icon="arrow_sync" busy={listing()} onClick={refresh}>
            {t("ui.adapter.refresh")}
          </Button>
          <Button icon="arrow_reset" onClick={() => app.run(() => core.request("adapter.restart"))}>
            {t("ui.adapter.restart")}
          </Button>
        </div>
      </header>

      <p class={`status-line st-${adapter().status}`} role="status">
        <span class="dot" aria-hidden="true" />
        {t("ui.adapter.stack")}: {t(`ui.adapter.status_${adapter().status}`)}
      </p>
      <Show when={adapter().status === "error" && adapter().error}>{(m) => <ErrorPanel message={m()} />}</Show>

      <Show
        when={adapter().adapters.length > 0}
        fallback={
          <Banner kind="warn" title={t("firmware.no_adapters")}>
            {t("ui.adapter.none_hint")}
          </Banner>
        }
      >
        <For each={adapter().adapters}>
          {(a) => {
            const drv = () => driverText(a.driver);
            const fw = () => adapterFirmwareTag(a, snap().settings);
            return (
              <section class="card adapter-card" aria-label={a.display_name}>
                <div class="adapter-head">
                  <Icon name="usb_stick" />
                  <div>
                    <h2>{a.display_name}</h2>
                    <span class="muted">
                      {a.device_name} · {a.vendor} · <span class="mono">{hex4(a.vid)}:{hex4(a.pid)}</span>
                    </span>
                  </div>
                </div>
                <dl class="kv">
                  <div>
                    <dt>{t("ui.adapter.driver")}</dt>
                    <dd>
                      <span class={`tag st-${drv().status}`}>{drv().text}</span>
                      <span class="muted mono"> {a.driver_service}</span>
                    </dd>
                  </div>
                  <div>
                    <dt>{t("ui.adapter.firmware")}</dt>
                    <dd>
                      <span class={`tag st-${fw().status}`}>{t(fw().key)}</span>
                      <Show when={a.realtek_chip}>
                        <span class="muted"> {a.realtek_chip}</span>
                      </Show>
                    </dd>
                  </div>
                </dl>
                <div class="adapter-actions">
                  <Show
                    when={a.driver === "winusb"}
                    fallback={
                      <>
                        <Button kind="strong" icon="shield" onClick={() => setDialog({ adapter: a, mode: "assign" })}>
                          {t("ui.adapter.assign_winusb")}
                        </Button>
                        <span class="hint">{t("ui.adapter.assign_short")}</span>
                      </>
                    }
                  >
                    <Button icon="arrow_reset" onClick={() => setDialog({ adapter: a, mode: "restore" })}>
                      {t("ui.adapter.restore_driver")}
                    </Button>
                    <span class="hint">{t("ui.adapter.restore_short")}</span>
                  </Show>
                </div>
              </section>
            );
          }}
        </For>
      </Show>

      <ChipSection files={files()} onChanged={loadFiles} />
      <AfhSection />

      <WinusbDialog adapter={dialog()?.adapter ?? null} mode={dialog()?.mode ?? "assign"} onClose={() => setDialog(null)} />
    </div>
  );
}

/** v1 FirmwareDialog: the chip choice and the firmware state of that chip */
function ChipSection(props: { files: FirmwareFile[]; onChanged: () => Promise<void> }) {
  const app = useApp();
  const snap = () => app.core.state.snap!;
  const settings = () => snap().settings;
  const adapters = () => snap().adapter.adapters;
  const detected = () => adapters().find((a) => a.realtek_chip)?.realtek_chip ?? null;
  const choice = () =>
    buildChipOptions(settings(), props.files, {
      auto: t("firmware.chip_auto", detected() ?? t("firmware.chip_auto_none")),
      nonRealtek: t("firmware.chip_non_realtek"),
    });
  const hint = () => (isAutoChip(settings()) && detected() ? t("firmware.auto_selected") : t("firmware.select_chip"));
  const choose = async (key: string) => {
    const o = choice().options.find((x) => x.key === key);
    if (!o || (o.pid === settings().bt_chip_pid && o.stem === settings().bt_chip_fw_stem)) return;
    // The core lists the adapters anew with the firmware state for this chip
    // (and releases an idle stack); the folder may have changed as well
    if (await saveSettings(app, { bt_chip_pid: o.pid, bt_chip_fw_stem: o.stem })) await props.onChanged();
  };
  return (
    <Section title={t("firmware.title")}>
      <Field label={t("settings.bt_chip")} hint={hint()}>
        {(id, described) => (
          <Select
            id={id}
            describedBy={described}
            value={choice().selected}
            options={choice().options.map((o) => ({ value: o.key, label: o.label }))}
            onChange={(key) => void choose(key)}
          />
        )}
      </Field>
      <div aria-live="polite">
        <Show when={chipStatus(settings(), adapters(), props.files)} keyed>
          {(st) => <FirmwareStatus status={st} />}
        </Show>
      </div>
      <div class="inline">
        <Button icon="folder_open" kind="subtle" onClick={() => void openConfigFolder(app)}>
          {t("firmware.open_folder")}
        </Button>
        <Button icon="open" kind="subtle" onClick={() => app.core.transport.openUrl(LINKS.setupGuide)}>
          {t("ui.setup.guide")}
        </Button>
      </div>
    </Section>
  );
}

/** v1 UpdateFirmwareStatus: the files, what is missing, a vendor's note */
function FirmwareStatus(props: { status: ChipStatus }): JSX.Element {
  const app = useApp();
  const download = (url: string) => (
    <Button icon="open" onClick={() => app.core.transport.openUrl(url)}>
      {t("firmware.open_download_page")}
    </Button>
  );
  const fileList = (files: string[]) => (
    <ul class="mono files">
      <For each={files}>{(f) => <li>{f}</li>}</For>
    </ul>
  );
  const st = props.status;
  switch (st.kind) {
    case "present":
      return (
        <Banner kind="good" title={t("firmware.present")}>
          {fileList(st.files)}
        </Banner>
      );
    case "missing":
    case "unchecked": {
      if (st.kind === "unchecked" && st.present === true) {
        return (
          <Banner kind="good" title={t("firmware.present")}>
            {fileList(st.files)}
          </Banner>
        );
      }
      const missing = st.kind === "missing" || st.present === false;
      return (
        <Banner
          kind={missing ? "warn" : "info"}
          title={missing ? t("firmware.missing_manual") : undefined}
          actions={download(LINKS.firmwareRealtek)}
        >
          <p>{t("firmware.required_files")}</p>
          {fileList(st.files)}
          <p class="hint">{t("firmware.manual_instructions")}</p>
        </Banner>
      );
    }
    case "invalid":
      return (
        <Banner kind="warn" title={t("firmware.invalid_short")} actions={download(LINKS.firmwareRealtek)}>
          <p>{t("firmware.invalid", st.file)}</p>
          <p>{t("firmware.required_files")}</p>
          {fileList(st.files)}
        </Banner>
      );
    case "vendor": {
      const url = vendorFirmwareUrl(st.vendor);
      return (
        <Banner
          kind={st.note.key === "firmware.vendor_unsupported" ? "warn" : "info"}
          actions={url ? download(url) : undefined}
        >
          {tm(st.note)}
        </Banner>
      );
    }
    case "non_realtek":
      return <Banner kind="good">{t("firmware.non_realtek")}</Banner>;
    case "auto_no_adapter":
      return <Banner kind="warn">{t("firmware.auto_no_adapter")}</Banner>;
    case "auto_unrecognized":
      return <Banner kind="warn">{t("firmware.auto_unrecognized")}</Banner>;
    default:
      return <Banner kind="warn">{t("firmware.select_chip")}</Banner>;
  }
}

/** "6,11": Wi-Fi channels 1..14, at least one */
export function parseAfhChannels(s: string): string | null {
  const parts = s.split(/[\s,]+/).filter(Boolean);
  if (parts.length === 0) return null;
  const nums = parts.map(Number);
  if (nums.some((n) => !Number.isInteger(n) || n < 1 || n > 14)) return null;
  return [...new Set(nums)].sort((a, b) => a - b).join(",");
}

const AFH_SECTION = "adapter-afh";

function AfhSection() {
  const app = useApp();
  const afh = () => app.core.state.snap!.settings.afh;
  const mode = () => (afh() === "auto" || afh() === "off" ? afh() : "manual");
  const [channels, setChannels] = createSignal(mode() === "manual" ? afh() : "");
  const [manual, setManual] = createSignal(mode() === "manual");
  const bad = () => manual() && channels().trim() !== "" && parseAfhChannels(channels()) === null;
  const saveChannels = () => {
    const c = parseAfhChannels(channels());
    if (c && c !== afh()) void saveSettings(app, { afh: c });
  };
  return (
    <Section id={AFH_SECTION} title={t("receiver.afh")}>
      <p class="hint">{t("receiver.afh_tooltip")}</p>
      <Segmented
        label={t("receiver.afh")}
        value={manual() ? "manual" : mode()}
        options={[
          { value: "auto", label: t("receiver.afh_auto") },
          { value: "off", label: t("receiver.afh_off") },
          { value: "manual", label: t("receiver.afh_manual") },
        ]}
        onChange={(v) => {
          setManual(v === "manual");
          if (v === "manual") saveChannels();
          else void saveSettings(app, { afh: v });
        }}
      />
      <Show when={manual()}>
        <Field label={t("receiver.afh_channels")} hint={t("ui.adapter.afh_channels_hint")} error={bad() ? t("ui.adapter.afh_bad") : null}>
          {(id, described) => (
            <input
              id={id}
              class="input mono narrow"
              placeholder="6,11"
              value={channels()}
              aria-describedby={described}
              aria-invalid={bad() ? "true" : undefined}
              onInput={(e) => setChannels(e.currentTarget.value)}
              onBlur={saveChannels}
              onKeyDown={(e) => e.key === "Enter" && saveChannels()}
            />
          )}
        </Field>
      </Show>
    </Section>
  );
}
