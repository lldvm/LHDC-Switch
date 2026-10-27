// First-run setup, shown when there is no profile or no adapter on WinUSB:
// 1 choose the adapter, 2 driver and firmware (WinUSB with one button),
// 3 find the headphones and create a profile, 4 choose what to send and
// connect (developer mode also offers the core's test tone).

import { createEffect, createMemo, createSignal, For, Match, on, onCleanup, Show, Switch } from "solid-js";
import { errorText, useApp } from "../app/context";
import { Banner, Button, Field } from "../components/controls";
import { DevicePicker } from "../components/DevicePicker";
import { Icon } from "../components/Icon";
import { CapturePicker } from "../components/CapturePicker";
import { CodecBadge, ConnectButton, currentProfile, Ring, WinusbDialog } from "../components/stream";
import { t, tm } from "../i18n";
import type { AdapterInfo, Snapshot, StreamState } from "../ipc/types";
import { firmwareUrl, LINKS } from "../lib/links";
import { newProfile, normalizeProfile, toneParams, validateProfile } from "../lib/profile";

export { toneParams };
import { driverText } from "./Adapter";
import { V1Running } from "./Home";

export function needsSetup(snap: Snapshot): boolean {
  return snap.profiles.length === 0 || !snap.adapter.adapters.some((a) => a.driver === "winusb");
}

const STEP_KEYS = ["ui.setup.step_adapter", "ui.setup.step_driver", "ui.setup.step_device", "ui.setup.step_test"];

export function Setup() {
  const app = useApp();
  const { core } = app;
  const snap = () => core.state.snap!;
  const [step, setStep] = createSignal(0);
  const [chosen, setChosen] = createSignal<string | null>(null);
  const adapter = createMemo<AdapterInfo | null>(() => {
    const list = snap().adapter.adapters;
    return list.find((a) => a.instance_id === chosen()) ?? list.find((a) => a.driver === "winusb") ?? list[0] ?? null;
  });
  const [profileName, setProfileName] = createSignal<string | null>(snap().active_profile);

  const finish = () => {
    app.setSetupActive(false);
    app.navigate({ name: "home" });
  };

  const canNext = () => {
    switch (step()) {
      case 0:
        return adapter() !== null;
      case 1:
        return adapter()?.driver === "winusb";
      case 2:
        return profileName() !== null && snap().profiles.some((p) => p.name === profileName());
      default:
        return true;
    }
  };

  let heading: HTMLHeadingElement | undefined;
  const go = (n: number) => {
    setStep(n);
    // Screen readers and keyboard users land on the new step's title
    queueMicrotask(() => heading?.focus());
  };

  return (
    <div class="setup">
      <header class="setup-head">
        <img src="/app-icon.png" alt="" width="32" height="32" />
        <div>
          <h1>{t("ui.setup.title")}</h1>
          <p class="muted">{t("ui.setup.intro")}</p>
        </div>
      </header>
      <V1Running />
      <ol class="stepper">
        <For each={STEP_KEYS}>
          {(k, i) => (
            <li classList={{ on: i() === step(), done: i() < step() }} aria-current={i() === step() ? "step" : undefined}>
              <span class="stepper-num num">{i() < step() ? <Icon name="checkmark" /> : i() + 1}</span>
              <span>{t(k)}</span>
            </li>
          )}
        </For>
      </ol>
      <section class="card setup-body" aria-labelledby="setup-step-title">
        <h2 id="setup-step-title" tabindex="-1" ref={heading}>
          {t(STEP_KEYS[step()])}
        </h2>
        <Switch>
          <Match when={step() === 0}>
            <StepAdapter chosen={adapter()} onChoose={setChosen} />
          </Match>
          <Match when={step() === 1}>
            <StepDriver adapter={adapter()} />
          </Match>
          <Match when={step() === 2}>
            <StepDevice profileName={profileName()} onCreated={setProfileName} />
          </Match>
          <Match when={step() === 3}>
            <StepTest profileName={profileName()} />
          </Match>
        </Switch>
      </section>
      <footer class="setup-foot">
        <Button kind="subtle" onClick={finish}>
          {t("ui.setup.skip")}
        </Button>
        <span class="spacer" />
        <Show when={step() > 0}>
          <Button icon="arrow_left" onClick={() => go(step() - 1)}>
            {t("ui.setup.back")}
          </Button>
        </Show>
        <Show
          when={step() < 3}
          fallback={
            <Button kind="strong" icon="checkmark" onClick={finish}>
              {t("ui.setup.finish")}
            </Button>
          }
        >
          <Button kind="strong" icon="arrow_right" disabled={!canNext()} onClick={() => go(step() + 1)}>
            {t("ui.setup.next")}
          </Button>
        </Show>
      </footer>
    </div>
  );
}

function StepAdapter(props: { chosen: AdapterInfo | null; onChoose: (id: string) => void }) {
  const app = useApp();
  const adapters = () => app.core.state.snap!.adapter.adapters;
  const [busy, setBusy] = createSignal(false);
  const refresh = async () => {
    setBusy(true);
    await app.run(() => app.core.request("adapter.list"));
    setBusy(false);
  };
  return (
    <div class="stack">
      <p>{t("ui.setup.adapter_intro")}</p>
      <Show
        when={adapters().length > 0}
        fallback={
          <Banner kind="warn" title={t("firmware.no_adapters")}>
            {t("ui.adapter.none_hint")}
          </Banner>
        }
      >
        <div class="choice-list" role="radiogroup" aria-label={t("firmware.detected_adapters")}>
          <For each={adapters()}>
            {(a) => (
              <label class="choice" classList={{ on: props.chosen?.instance_id === a.instance_id }}>
                <input
                  type="radio"
                  name="setup-adapter"
                  checked={props.chosen?.instance_id === a.instance_id}
                  onChange={() => props.onChoose(a.instance_id)}
                />
                <Icon name="usb_stick" />
                <span class="choice-text">
                  <strong>{a.display_name}</strong>
                  <span class="muted">
                    {a.device_name} · {a.vendor}
                  </span>
                </span>
                <span class={`tag st-${driverText(a.driver).status}`}>{driverText(a.driver).text}</span>
              </label>
            )}
          </For>
        </div>
      </Show>
      <div>
        <Button icon="arrow_sync" busy={busy()} onClick={refresh}>
          {t("ui.adapter.refresh")}
        </Button>
      </div>
    </div>
  );
}

function StepDriver(props: { adapter: AdapterInfo | null }) {
  const app = useApp();
  const [dialog, setDialog] = createSignal(false);
  return (
    <Show when={props.adapter} fallback={<p class="muted">{t("firmware.no_adapters")}</p>}>
      {(a) => (
        <div class="stack">
          <h3 class="sub">{t("ui.adapter.driver")}</h3>
          <Show
            when={a().driver === "winusb"}
            fallback={
              <Banner
                kind="info"
                title={t("ui.adapter.assign_title")}
                actions={
                  <Button kind="strong" icon="shield" onClick={() => setDialog(true)}>
                    {t("ui.adapter.assign_winusb")}
                  </Button>
                }
              >
                <p>{t("ui.adapter.assign_explain")}</p>
                <p class="muted">{t("ui.adapter.uac_note")}</p>
              </Banner>
            }
          >
            <Banner kind="good" title={t("ui.setup.driver_ok")}>
              {t("ui.adapter.restore_short")}
            </Banner>
          </Show>
          <h3 class="sub">{t("ui.adapter.firmware")}</h3>
          <Switch>
            <Match when={a().firmware_present === null}>
              <p class="muted">{t("ui.adapter.firmware_not_needed")}</p>
            </Match>
            <Match when={a().firmware_present === true}>
              <Banner kind="good" title={t("firmware.present")}>
                {a().realtek_chip}
              </Banner>
            </Match>
            <Match when={a().firmware_present === false}>
              <Banner
                kind="warn"
                title={a().firmware_invalid ? t("firmware.invalid_short") : t("firmware.missing_manual")}
                actions={
                  <>
                    <Button icon="open" onClick={() => app.core.transport.openUrl(firmwareUrl(a().vendor))}>
                      {t("firmware.open_download_page")}
                    </Button>
                    <Button icon="folder_open" onClick={() => app.core.transport.openFolder(app.core.state.snap!.config_dir)}>
                      {t("firmware.open_folder")}
                    </Button>
                    <Button icon="open" kind="subtle" onClick={() => app.core.transport.openUrl(LINKS.setupGuide)}>
                      {t("ui.setup.guide")}
                    </Button>
                  </>
                }
              >
                <Show when={a().firmware_invalid}>{(file) => <p>{t("firmware.invalid", file())}</p>}</Show>
                <p>
                  {t("firmware.required_files")} <span class="mono">{a().firmware_files.join(", ")}</span>
                </p>
                <p class="hint">{t("firmware.manual_instructions")}</p>
              </Banner>
            </Match>
          </Switch>
          <WinusbDialog adapter={dialog() ? a() : null} mode="assign" onClose={() => setDialog(false)} />
        </div>
      )}
    </Show>
  );
}

function StepDevice(props: { profileName: string | null; onCreated: (name: string) => void }) {
  const app = useApp();
  const snap = () => app.core.state.snap!;
  const [address, setAddress] = createSignal("");
  const [deviceName, setDeviceName] = createSignal("");
  const [name, setName] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [problem, setProblem] = createSignal<string | null>(null);
  const existing = () => snap().profiles.find((p) => p.name === props.profileName) ?? null;

  const create = async () => {
    const p = { ...newProfile(), name: name() || deviceName() || address(), device_address: address(), device_name: deviceName() };
    const found = validateProfile(p, snap().profiles.map((x) => x.name));
    const first = found.device ?? found.name;
    if (first) {
      setProblem(t(first.key, ...(first.args ?? [])));
      return;
    }
    setProblem(null);
    setBusy(true);
    try {
      const saved = normalizeProfile(p);
      await app.core.request("profile.save", { profile: saved, original_name: null });
      await app.core.request("profile.set_active", { name: saved.name });
      props.onCreated(saved.name);
    } catch (e) {
      setProblem(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div class="stack">
      <Show when={existing()}>
        {(p) => (
          <Banner kind="good" title={t("ui.setup.profile_ready")}>
            <strong>{p().name}</strong> <span class="mono muted">{p().device_address}</span>
          </Banner>
        )}
      </Show>
      <p>{t("ui.setup.device_intro")}</p>
      <DevicePicker
        address={address()}
        classicOnly
        onPick={(a, n) => {
          setAddress(a);
          setDeviceName(n);
          setName(n || a);
        }}
      />
      <Field label={t("profile.name")} hint={t("profile.name_hint")} error={problem()}>
        {(id, described) => (
          <div class="inline">
            <input id={id} class="input" value={name()} aria-describedby={described} onInput={(e) => setName(e.currentTarget.value)} />
            <Button kind="strong" icon="add" busy={busy()} disabled={!address()} onClick={create}>
              {t("ui.setup.create_profile")}
            </Button>
          </div>
        )}
      </Field>
    </div>
  );
}


const ACTIVE: StreamState[] = ["connecting", "reconnecting", "streaming"];

function StepTest(props: { profileName: string | null }) {
  const app = useApp();
  const { core } = app;
  const snap = () => core.state.snap!;
  const stream = () => snap().stream;
  const profile = () => snap().profiles.find((p) => p.name === props.profileName) ?? currentProfile(snap());
  // The tone this step started (until stopped, failed or ended)
  const [ours, setOurs] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  // Why the tone could not be played: the fallback message
  const [failed, setFailed] = createSignal<string | null>(null);
  const playing = () => ours() && ACTIVE.includes(stream().state);
  const dev = () => snap().settings.developer_mode;

  createEffect(
    on(
      () => stream().state,
      (st) => {
        if (!ours()) return;
        if (st === "error") {
          setFailed(tm(stream().message) || t("status.error"));
          setOurs(false);
        } else if (st === "idle") {
          setOurs(false);
        }
      },
      { defer: true },
    ),
  );

  const play = async () => {
    const p = profile();
    if (!p) return;
    setBusy(true);
    setFailed(null);
    try {
      // The profile's own stream (or anything else) gives way to the tone
      if (ACTIVE.includes(stream().state)) await core.request("stream.stop");
      await core.request("stream.start_custom", toneParams(p));
      setOurs(true);
    } catch (e) {
      setFailed(errorText(e));
    } finally {
      setBusy(false);
    }
  };
  const stop = async () => {
    setBusy(true);
    await app.run(() => core.request("stream.stop"));
    setOurs(false);
    setBusy(false);
  };
  // Leaving the step (Back, Finish, Skip) ends the tone
  onCleanup(() => {
    if (playing()) core.request("stream.stop").catch(() => {});
  });

  return (
    <div class="stack">
      <p>{t("ui.setup.test_intro")}</p>
      <div class="test-row">
        <Ring state={stream().state} volume={stream().volume} size="small">
          <Icon name="headphones" />
        </Ring>
        <div class="stack-tight">
          <span class={`status-line st-${stream().state}`} role="status" aria-live="polite">
            <span class="dot" aria-hidden="true" />
            {tm(stream().message) || t(`status.${stream().state}`)}
          </span>
          <CodecBadge />
        </div>
      </div>
      <Show when={profile()}>{(p) => <CapturePicker profile={p()} />}</Show>
      <ConnectButton onError={(e) => app.notify(errorText(e), "error")} />
      <Show when={dev()}>
        <div class="inline">
          <Show
            when={playing()}
            fallback={
              <Button icon="music_note_2" busy={busy()} disabled={!profile()} onClick={play}>
                {t("ui.setup.play_tone")}
              </Button>
            }
          >
            <Button icon="stop" busy={busy()} onClick={stop}>
              {t("receiver.stop")}
            </Button>
          </Show>
          <Show when={playing() && stream().state === "streaming"}>
            <span class="hint">{t("ui.setup.tone_playing_hint")}</span>
          </Show>
        </div>
        <Show when={failed()}>
          {(why) => (
            <Banner kind="warn" title={t("status.error")}>
              {t("ui.setup.tone_failed", why())}
            </Banner>
          )}
        </Show>
      </Show>
    </div>
  );
}
