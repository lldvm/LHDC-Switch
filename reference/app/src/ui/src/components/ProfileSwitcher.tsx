// Choosing the profile for the big button. While a stream runs, choosing
// another profile streams that one (stream.start stops the running one);
// otherwise it only becomes the active profile.

import { useApp } from "../app/context";
import { t } from "../i18n";
import { Select } from "./controls";
import { currentProfile } from "./stream";

export function ProfileSwitcher() {
  const app = useApp();
  const snap = () => app.core.state.snap!;
  const value = () => currentProfile(snap())?.name ?? "";
  const choose = (name: string) => {
    const running = snap().stream.state === "streaming" || snap().stream.state === "connecting";
    void app.run(() =>
      running ? app.core.request("stream.start", { profile: name }) : app.core.request("profile.set_active", { name }),
    );
  };
  return (
    <div class="profile-switcher">
      <Select
        label={t("profile.header")}
        value={value()}
        options={snap().profiles.map((p) => ({ value: p.name, label: p.name }))}
        onChange={choose}
      />
    </div>
  );
}
