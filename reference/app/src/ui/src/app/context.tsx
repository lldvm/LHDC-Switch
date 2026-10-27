// What every screen reaches: the core store, navigation (hash routes, so
// the browser's back button works in dev mode) and the transient notice
// line (announced to screen readers).

import { createContext, createSignal, onCleanup, useContext, type Accessor, type JSX } from "solid-js";
import type { Core } from "../ipc/store";
import { IpcError } from "../ipc/types";
import { t, tm } from "../i18n";

export type RouteName = "home" | "profiles" | "diagnostics" | "adapter" | "settings" | "setup";

export interface Route {
  name: RouteName;
  /** profiles: "new" or the name of the profile being edited; adapter:
   *  "afh" for its channel hopping section */
  arg?: string;
}

export function parseRoute(hash: string): Route {
  const parts = hash.replace(/^#\/?/, "").split("/");
  const names: RouteName[] = ["home", "profiles", "diagnostics", "adapter", "settings", "setup"];
  const name = names.find((n) => n === parts[0]) ?? "home";
  if (name === "profiles" && parts[1] === "new") return { name, arg: "new" };
  if (name === "profiles" && parts[1] === "edit" && parts[2] !== undefined) {
    return { name, arg: decodeURIComponent(parts.slice(2).join("/")) };
  }
  if (name === "adapter" && parts[1] === "afh") return { name, arg: "afh" };
  return { name };
}

export function routeHash(r: Route): string {
  if (r.name === "profiles" && r.arg === "new") return "#/profiles/new";
  if (r.name === "profiles" && r.arg) return `#/profiles/edit/${encodeURIComponent(r.arg)}`;
  if (r.name === "adapter" && r.arg === "afh") return "#/adapter/afh";
  return `#/${r.name}`;
}

export interface Notice {
  text: string;
  kind: "info" | "error";
}

export interface AppContext {
  core: Core;
  route: Accessor<Route>;
  navigate(r: Route): void;
  notice: Accessor<Notice | null>;
  notify(text: string, kind?: Notice["kind"]): void;
  /** Runs a request for a button: shows the error as a notice; true on success */
  run(f: () => Promise<unknown>): Promise<boolean>;
  /** The first-run setup is on screen */
  setupActive: Accessor<boolean>;
  setSetupActive(v: boolean): void;
}

const Ctx = createContext<AppContext>();

export function useApp(): AppContext {
  const c = useContext(Ctx);
  if (!c) throw new Error("useApp outside AppProvider");
  return c;
}

/** The text of an error from a request */
export function errorText(e: unknown): string {
  const err = IpcError.from(e);
  const text = tm(err.msg);
  return text || t("status.error");
}

export function AppProvider(props: { core: Core; children: JSX.Element }) {
  const [route, setRoute] = createSignal<Route>(parseRoute(location.hash));
  const onHash = () => setRoute(parseRoute(location.hash));
  window.addEventListener("hashchange", onHash);
  onCleanup(() => window.removeEventListener("hashchange", onHash));

  const [notice, setNotice] = createSignal<Notice | null>(null);
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;
  const notify = (text: string, kind: Notice["kind"] = "info") => {
    setNotice({ text, kind });
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => setNotice(null), kind === "error" ? 8000 : 3500);
  };
  onCleanup(() => clearTimeout(noticeTimer));

  const [setupActive, setSetupActive] = createSignal(false);

  const ctx: AppContext = {
    core: props.core,
    route,
    navigate(r) {
      const h = routeHash(r);
      if (location.hash !== h) location.hash = h;
      setRoute(r);
    },
    notice,
    notify,
    async run(f) {
      try {
        await f();
        return true;
      } catch (e) {
        notify(errorText(e), "error");
        return false;
      }
    },
    setupActive,
    setSetupActive,
  };
  props.core.transport.onNavigate((r) => ctx.navigate(parseRoute(`#/${r}`)));
  return <Ctx.Provider value={ctx}>{props.children}</Ctx.Provider>;
}
