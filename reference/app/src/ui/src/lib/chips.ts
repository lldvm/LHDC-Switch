// Bluetooth chips for the firmware (settings.bt_chip_pid / bt_chip_fw_stem),
// the table of v1 (bt_adapter_enum.cpp chip_db, develop branch) and the core
// (crates/bt-host/src/chips.rs).

export const CHIP_AUTO = 0;
export const CHIP_NOT_REALTEK = 0xfffe;
export const CHIP_CUSTOM = 0xffff;

export interface Chip {
  pid: number;
  name: string;
  /** Firmware file stem: <stem>_fw.bin, <stem>_config.bin */
  stem: string;
}

export const REALTEK_CHIPS: Chip[] = [
  { pid: 0x8771, name: "RTL8761BU", stem: "rtl8761bu" },
  { pid: 0x876c, name: "RTL8761CU", stem: "rtl8761cu" },
  { pid: 0xb82c, name: "RTL8822BU", stem: "rtl8822bu" },
  { pid: 0xc821, name: "RTL8821CU", stem: "rtl8821cu" },
  { pid: 0xc82c, name: "RTL8822CU", stem: "rtl8822cu" },
  { pid: 0xd723, name: "RTL8723DU", stem: "rtl8723du" },
  { pid: 0xc852, name: "RTL8852AU", stem: "rtl8852au" },
  { pid: 0xb85b, name: "RTL8852BU", stem: "rtl8852bu" },
  { pid: 0xb85d, name: "RTL8852BTU", stem: "rtl8852btu" },
  { pid: 0xc85a, name: "RTL8852CU", stem: "rtl8852cu" },
  { pid: 0xb083, name: "RTL8851BU", stem: "rtl8851bu" },
  { pid: 0x922a, name: "RTL8922AU", stem: "rtl8922au" },
];

/** The settings.bt_chip_fw_stem that goes with a chip choice (as v1 stored it) */
export function stemFor(pid: number): string {
  if (pid === CHIP_CUSTOM) return "custom";
  return REALTEK_CHIPS.find((c) => c.pid === pid)?.stem ?? "";
}

/** A chip setting as settings.bt_chip_pid / bt_chip_fw_stem hold it */
export interface ChipSetting {
  bt_chip_pid: number;
  bt_chip_fw_stem: string;
}

/** An entry of the chip choice: the setting it saves and its text */
export interface ChipOption {
  /** Unique within the list (pid alone does not tell file-only chips apart) */
  key: string;
  pid: number;
  stem: string;
  label: string;
}

const optionKey = (pid: number, stem: string) => `${pid}:${stem}`;
const hexPid = (n: number) => n.toString(16).toUpperCase().padStart(4, "0");

/** True for the Auto setting (core chips::is_auto_chip) */
export function isAutoChip(s: ChipSetting): boolean {
  return s.bt_chip_pid === CHIP_AUTO && s.bt_chip_fw_stem === "";
}

/**
 * The chip choice of v1's firmware dialog: Auto, Not Realtek, every chip of
 * the table (files there or not), the chips known only by their firmware
 * files in the settings folder (adapter.firmware_files entries without a
 * table PID), Custom, and the current setting if no entry stands for it.
 * `selected` is the key of the entry the setting matches.
 */
export function buildChipOptions(
  setting: ChipSetting,
  files: readonly { stem: string; display_name: string; pid: number | null }[],
  labels: { auto: string; nonRealtek: string },
): { options: ChipOption[]; selected: string } {
  const options: ChipOption[] = [];
  const add = (pid: number, stem: string, label: string) => options.push({ key: optionKey(pid, stem), pid, stem, label });
  add(CHIP_AUTO, "", labels.auto);
  add(CHIP_NOT_REALTEK, "", labels.nonRealtek);
  for (const c of REALTEK_CHIPS) add(c.pid, c.stem, `${c.name} (${c.stem}_fw.bin)`);
  const tableStems = new Set(REALTEK_CHIPS.map((c) => c.stem));
  const seen = new Set<string>();
  for (const f of files) {
    const lower = f.stem.toLowerCase();
    // Table chips are listed already; custom has its own entry
    if (f.pid !== null || tableStems.has(lower) || lower === "custom" || seen.has(lower) || f.stem === "") continue;
    seen.add(lower);
    add(CHIP_AUTO, f.stem, `${f.display_name} (${f.stem}_fw.bin)`);
  }
  add(CHIP_CUSTOM, "custom", "Custom (custom_fw.bin, custom_config.bin)");

  // v1: a PID matches by itself, PID 0 (Auto, a file-only chip) by the stem
  const { bt_chip_pid: pid, bt_chip_fw_stem: stem } = setting;
  let match = options.find((o) =>
    o.pid !== CHIP_AUTO ? o.pid === pid : pid === CHIP_AUTO && o.stem.toLowerCase() === stem.toLowerCase(),
  );
  if (!match) {
    // A setting no entry stands for (a chip since dropped from the table, a
    // file since removed) stays choosable as it is
    add(pid, stem, `0x${hexPid(pid)} ${stem}`.trimEnd());
    match = options[options.length - 1];
  }
  return { options, selected: match.key };
}

/** The firmware file stem of a chip setting, null if it names no Realtek
 *  firmware (core chips::firmware_files) */
export function firmwareStem(pid: number, stem: string): string | null {
  if (pid === CHIP_NOT_REALTEK) return null;
  const c = REALTEK_CHIPS.find((x) => x.pid === pid);
  if (c) return c.stem;
  if (pid === CHIP_CUSTOM) return "custom";
  return stem !== "" ? stem : null;
}

/** What the Adapter screen needs of an adapter for the firmware */
export interface AdapterFirmware {
  vendor: string;
  realtek_chip: string | null;
  firmware_present: boolean | null;
  firmware_files: string[];
  /** A file that is there but is no Realtek firmware (a saved web page) */
  firmware_invalid?: string | null;
  vendor_note: { key: string; args: string[] } | null;
}

/**
 * The firmware state of the chip setting (v1 FirmwareDialog::
 * UpdateFirmwareStatus):
 * - present / missing: the Realtek files of the chosen (or, for Auto, the
 *   detected) chip, as the core checked them for the adapters
 * - invalid: one of them is there but is no Realtek firmware, usually a web
 *   page saved instead of the file (v1 firmware.invalid)
 * - unchecked: no adapter to check them for; from the folder scan if it
 *   lists the file, unknown for Custom (the scan leaves it out)
 * - vendor: no Realtek chip, and an adapter of a vendor the core has a note
 *   for (Intel, Broadcom, CSR, unsupported)
 * - non_realtek, auto_no_adapter, auto_unrecognized, select_chip: v1's texts
 */
export type ChipStatus =
  | { kind: "present"; files: string[] }
  | { kind: "missing"; files: string[] }
  | { kind: "invalid"; files: string[]; file: string }
  | { kind: "unchecked"; files: string[]; present: boolean | null }
  | { kind: "vendor"; note: { key: string; args: string[] }; vendor: string }
  | { kind: "non_realtek" | "auto_no_adapter" | "auto_unrecognized" | "select_chip" };

export function chipStatus(
  setting: ChipSetting,
  adapters: readonly AdapterFirmware[],
  found: readonly { stem: string; has_config: boolean }[],
): ChipStatus {
  const auto = isAutoChip(setting);
  const realtek = adapters.find((a) => a.realtek_chip !== null);
  const notRealtek = setting.bt_chip_pid === CHIP_NOT_REALTEK;
  if (notRealtek || (auto && !realtek)) {
    // v1 tells about another vendor only when no Realtek adapter is there
    const other = realtek ? undefined : adapters.find((a) => a.vendor_note !== null);
    if (other) return { kind: "vendor", note: other.vendor_note!, vendor: other.vendor };
    if (notRealtek) return { kind: "non_realtek" };
    return { kind: adapters.length > 0 ? "auto_unrecognized" : "auto_no_adapter" };
  }
  // The core resolves the setting for each adapter; Auto by its own chip
  const checked = auto ? realtek : adapters.find((a) => a.firmware_present !== null);
  if (checked && checked.firmware_present !== null) {
    if (!checked.firmware_present && checked.firmware_invalid) {
      return { kind: "invalid", files: checked.firmware_files, file: checked.firmware_invalid };
    }
    return { kind: checked.firmware_present ? "present" : "missing", files: checked.firmware_files };
  }
  const stem = firmwareStem(setting.bt_chip_pid, setting.bt_chip_fw_stem);
  if (stem === null) return { kind: "select_chip" };
  const files = [`${stem}_fw.bin`, `${stem}_config.bin`];
  const entry = found.find((f) => f.stem.toLowerCase() === stem.toLowerCase());
  const present = entry ? entry.has_config : stem.toLowerCase() === "custom" ? null : false;
  return { kind: "unchecked", files, present };
}

/** The adapter card's short firmware state: a language key and its colour */
export function adapterFirmwareTag(
  a: AdapterFirmware,
  setting: ChipSetting,
): { key: string; status: "good" | "warn" | "bad" } {
  if (a.firmware_present === true) return { key: "firmware.present", status: "good" };
  if (a.firmware_present === false) {
    return { key: a.firmware_invalid ? "firmware.invalid_short" : "firmware.missing_short", status: "bad" };
  }
  if (a.vendor_note?.key === "firmware.vendor_unsupported") return { key: "ui.adapter.firmware_unsupported", status: "warn" };
  if (a.vendor_note || setting.bt_chip_pid === CHIP_NOT_REALTEK) return { key: "ui.adapter.firmware_not_realtek", status: "good" };
  return { key: "ui.adapter.firmware_choose_chip", status: "warn" };
}
