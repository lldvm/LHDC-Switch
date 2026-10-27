// Profiles: the saved device + sound combinations as cards; the editor at
// #/profiles/new and #/profiles/edit/<name>.

import { createSignal, For, Show } from "solid-js";
import { useApp } from "../app/context";
import { Button, Dialog, IconButton } from "../components/controls";
import { Icon } from "../components/Icon";
import { t } from "../i18n";
import { codecSpec, codecSupport, qualityTag } from "../lib/codecs";
import { knownCodecs } from "../lib/profile";
import { captureText } from "../components/CapturePicker";
import { ProfileEditor } from "./ProfileEditor";

export function Profiles() {
  const app = useApp();
  return (
    <Show when={app.route().arg} keyed fallback={<ProfileList />}>
      {(arg) => <ProfileEditor original={arg === "new" ? null : arg} />}
    </Show>
  );
}

function ProfileList() {
  const app = useApp();
  const { core, navigate } = app;
  const snap = () => core.state.snap!;
  const [deleting, setDeleting] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  const streamingName = () => (snap().stream.state !== "idle" ? snap().stream.profile : null);

  const remove = async () => {
    const name = deleting();
    if (!name) return;
    setBusy(true);
    await app.run(() => core.request("profile.delete", { name }));
    setBusy(false);
    setDeleting(null);
  };

  return (
    <div class="page">
      <header class="page-head">
        <h1>{t("profile.header")}</h1>
        <Button kind="strong" icon="add" onClick={() => navigate({ name: "profiles", arg: "new" })}>
          {t("profile.new")}
        </Button>
      </header>
      <Show when={snap().profiles.length > 0} fallback={<p class="card empty">{t("profile.empty")}</p>}>
        <ul class="profile-grid">
          <For each={snap().profiles}>
            {(p) => {
              const active = () => p.name === snap().active_profile;
              const live = () => p.name === streamingName();
              return (
                <li class="card profile-card" classList={{ active: active() }}>
                  <div class="profile-card-head">
                    <Icon name="headphones" />
                    <div class="profile-card-title">
                      <h2 class="ellipsis">{p.name}</h2>
                      <span class="muted ellipsis">{p.device_name || p.device_address}</span>
                    </div>
                    <Show when={live()}>
                      <span class={`tag st-${snap().stream.state}`}>{t(`status.${snap().stream.state}`)}</span>
                    </Show>
                  </div>
                  <div class="profile-card-meta">
                    {/* LE Audio (experimental): LC3, no codec choice */}
                    <Show when={p.le_audio} fallback={<span class="tag">{t(codecSpec(p.codec).label)}</span>}>
                      <span class="tag">{t("ui.codec.lc3")}</span>
                    </Show>
                    {/* The quality and its bit rate (v1's list) */}
                    <Show when={qualityTag(p, t)}>{(q) => <span class="tag num">{q()}</span>}</Show>
                    {/* The device did not offer this codec when it last connected */}
                    <Show
                      when={
                        !p.le_audio && codecSupport(p.codec, knownCodecs(snap().devices, p.device_address)) === "not_offered"
                      }
                    >
                      <span class="tag">
                        <Icon name="warning" />
                        {t("ui.profile.codec_not_offered")}
                      </span>
                    </Show>
                    <span class="tag">
                      <Icon name={captureText(p).icon} />
                      <span class="ellipsis">{captureText(p).text}</span>
                    </span>
                  </div>
                  <div class="profile-card-actions">
                    <Button
                      kind={live() ? "default" : "strong"}
                      icon={live() ? "plug_disconnected" : "power"}
                      onClick={() =>
                        app.run(() =>
                          live() ? core.request("stream.stop") : core.request("stream.start", { profile: p.name }),
                        )
                      }
                    >
                      {live() ? t("connection.disconnect") : t("profile.connect")}
                    </Button>
                    <IconButton
                      icon="edit"
                      label={`${t("profile.edit")}: ${p.name}`}
                      onClick={() => navigate({ name: "profiles", arg: p.name })}
                    />
                    <IconButton
                      icon="delete"
                      label={`${t("profile.delete")}: ${p.name}`}
                      disabled={live()}
                      onClick={() => setDeleting(p.name)}
                    />
                  </div>
                </li>
              );
            }}
          </For>
        </ul>
      </Show>
      <Dialog
        open={deleting() !== null}
        title={t("modal.confirm_title")}
        confirm={t("profile.delete")}
        cancel={t("modal.cancel")}
        danger
        busy={busy()}
        onConfirm={remove}
        onCancel={() => setDeleting(null)}
      >
        <p>{t("modal.confirm_delete_profile")}</p>
        <p>
          <strong>{deleting()}</strong>
        </p>
      </Dialog>
    </div>
  );
}
