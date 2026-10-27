// Translations: the flat JSON files of app/lang (shared with the core),
// English built in and the other languages loaded when chosen. A key
// missing from a language shows the English text, as in v1.

import { createRoot, createSignal } from "solid-js";
import en from "../../../../../app/lang/en.json";
import type { Msg } from "../ipc/types";
import { printf } from "./printf";

export { printf };

type Dict = Record<string, string>;

const loaders = import.meta.glob<Dict>(["../../../../../app/lang/*.json", "!**/en.json"], { import: "default" });

/** The languages of app/lang: code (file name) -> name in that language */
export const LANGUAGES: { code: string; name: string }[] = [
  { code: "en", name: "English" },
  { code: "ja", name: "日本語" },
  { code: "de", name: "Deutsch" },
  { code: "fr", name: "Français" },
  { code: "es", name: "Español" },
  { code: "ko", name: "한국어" },
  { code: "zh-CN", name: "简体中文" },
  { code: "zh-TW", name: "繁體中文" },
];

const english: Dict = en;

const { dict, setDict, lang, setLang } = createRoot(() => {
  const [dict, setDict] = createSignal<Dict>(english);
  const [lang, setLang] = createSignal("en");
  return { dict, setDict, lang, setLang };
});

export { lang };

function loaderFor(code: string) {
  const key = Object.keys(loaders).find((p) => p.endsWith(`/${code}.json`));
  return key ? loaders[key] : undefined;
}

/** Switches the language (unknown codes: English) */
export async function setLanguage(code: string): Promise<void> {
  if (code === lang()) return;
  const load = loaderFor(code);
  if (code === "en" || !load) {
    setDict(english);
    setLang("en");
  } else {
    const d = await load();
    setDict(d);
    setLang(code);
  }
  if (typeof document !== "undefined") document.documentElement.lang = lang();
}

/** The text of `key` in the current language with printf arguments */
export function t(key: string, ...args: (string | number)[]): string {
  const template = dict()[key] ?? english[key] ?? key;
  // Without arguments the text is shown as it is, as v1 did: some strings
  // have a literal "%" ("about 80% little room") that is no placeholder
  return args.length ? printf(template, args) : template;
}

/** The text of a core message */
export function tm(msg: Msg | null | undefined): string {
  if (!msg || !msg.key) return "";
  return t(msg.key, ...msg.args);
}

/** The English text of a core message, as the core's Msg::english makes it
 *  (the text fields next to a Msg, such as FlowStepView.detail) */
export function englishText(msg: Msg): string {
  const template = english[msg.key] ?? msg.key;
  return msg.args.length ? printf(template, msg.args) : template;
}

/** For tests: whether a key exists in English */
export function hasKey(key: string): boolean {
  return key in english;
}
