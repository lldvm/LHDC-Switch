// The tray flyout (360×440): everyday control without the main window.
// Device and state, the big Connect / Disconnect, volume right below,
// the profile switcher, codec and link quality, the capture source,
// "Open app".

import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { errorText, useApp } from "../app/context";
import { CaptureSign } from "../app/indicators";
import { CapturePicker } from "../components/CapturePicker";
import { Button, IconButton } from "../components/controls";
import { Icon } from "../components/Icon";
import { NoticeLine } from "../components/Notice";
import { ProfileSwitcher } from "../components/ProfileSwitcher";
import {
  CodecBadge,
  ConnectButton,
  copyError,
  currentProfile,
  QualityIndicator,
  Ring,
  useFixes,
  VolumeControl,
} from "../components/stream";
import { t, tm } from "../i18n";
import { IpcError } from "../ipc/types";

export function Flyout() {
  const app = useApp();
  const { core } = app;
  const snap = () => core.state.snap!;
  const stream = () => snap().stream;
  const profile = () => currentProfile(snap());
  const [reqError, setReqError] = createSignal<IpcError | null>(null);
  const { fixFor, dialog } = useFixes();
  onCleanup(core.wantLink());

  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") void core.transport.hideWindow();
  };
  onMount(() => document.addEventListener("keydown", onKey));
  onCleanup(() => document.removeEventListener("keydown", onKey));

  const error = () => {
    const e = reqError();
    if (e) return { code: e.code as string | null, message: e.msg };
    if (stream().state === "error") return { code: null, message: stream().message };
    return null;
  };
  // In the flyout a fix that needs a screen opens the main window there
  const openMain = (route?: string) => core.transport.showMain(route).catch(() => {});

  return (
    <div class="flyout">
      <header class="fly-head">
        <Ring state={stream().state} volume={stream().volume} size="small">
          <Icon name="headphones" />
        </Ring>
        <div class="fly-title">
          <div class="device-name ellipsis">{profile()?.device_name || profile()?.name || t("profile.none_selected")}</div>
          <div class={`status-line st-${stream().state}`} role="status" aria-live="polite">
            <span class="dot" aria-hidden="true" />
            <span class="ellipsis">
              {stream().state === "error" ? t("status.error") : tm(stream().message) || t(`status.${stream().state}`)}
            </span>
          </div>
        </div>
        <IconButton icon="dismiss" label={t("modal.close")} onClick={() => void core.transport.hideWindow()} />
      </header>

      <Show when={profile()} fallback={<p class="muted">{t("profile.empty")}</p>}>
        <ConnectButton compact onError={(e) => setReqError(IpcError.from(e))} />
        <VolumeControl />
        <ProfileSwitcher />
      </Show>

      <div class="fly-row">
        <CodecBadge compact />
        <QualityIndicator />
      </div>
      <Show when={profile() || snap().capture}>
        <div class="fly-meta">
          <Show when={profile()}>{(p) => <CapturePicker profile={p()} />}</Show>
          <CaptureSign onOpen={() => void openMain("diagnostics")} />
        </div>
      </Show>

      <Show when={error()}>
        {(e) => {
          const fix = () => fixFor(e().code, e().message.key);
          return (
            <div class="fly-error" role="alert">
              <Icon name="error_circle" />
              <span class="fly-error-text">{tm(e().message)}</span>
              <IconButton
                icon="copy"
                label={t("error.click_to_copy_hint")}
                onClick={() => void copyError(app, e().message, e().code)}
              />
              <Show when={fix()}>
                {(f) => (
                  <Button
                    kind="subtle"
                    icon={f().icon}
                    onClick={() => {
                      // Fixes that are a screen of the main window go there
                      const k = e().message.key;
                      if (k.startsWith("error.codec") || k.startsWith("error.app") || k.startsWith("error.no_")) {
                        void openMain("profiles");
                      } else if (e().code === "adapter.driver_missing" || k === "error.btstack_init") {
                        void openMain("adapter");
                      } else {
                        void app.run(async () => f().run()).then((ok) => !ok && app.notify(errorText(e()), "error"));
                      }
                    }}
                  >
                    {f().label}
                  </Button>
                )}
              </Show>
            </div>
          );
        }}
      </Show>

      <footer class="fly-foot">
        <Show when={snap().update.available}>
          <Button kind="subtle" icon="arrow_sync" onClick={() => void openMain("settings")}>
            {t("update.available_title")}
          </Button>
          <span class="spacer" />
        </Show>
        <Button icon="open" onClick={() => void openMain()}>
          {t("ui.flyout.open_app")}
        </Button>
      </footer>
      {dialog()}
      <NoticeLine />
    </div>
  );
}
