// Web pages the UI opens (the same ones v1 opened)

const REPO = "https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge";

export const LINKS = {
  repo: REPO,
  setupGuide: `${REPO}/blob/main/docs/setup.md`,
  /** Realtek firmware: linux-firmware rtl_bt (v1 wx_firmware_dialog.cpp) */
  firmwareRealtek: "https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt",
  firmwareIntel: "https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/intel",
  /** linux-firmware has no Broadcom .hcd; this collects them from the Windows drivers */
  firmwareBroadcom: "https://github.com/winterheart/broadcom-bt-firmware",
  releases: `${REPO}/releases`,
  issues: `${REPO}/issues/new/choose`,
  sponsor: "https://github.com/sponsors/SeiyaFunaokaJP",
  vbCable: "https://vb-audio.com/Cable/",
} as const;

/** The third-party licences of this version (a development build has no
 *  licenses folder next to it): the release tag's copy, not the main
 *  branch's, which may list other components */
export function thirdPartyUrl(version: string): string {
  return `${REPO}/blob/v${version}/THIRD_PARTY_LICENSES.md`;
}

/** The firmware download page for an adapter's vendor */
export function firmwareUrl(vendor: string): string {
  const v = vendor.toLowerCase();
  if (v.includes("intel")) return LINKS.firmwareIntel;
  if (v.includes("broadcom")) return LINKS.firmwareBroadcom;
  return LINKS.firmwareRealtek;
}

/** The firmware page for a non-Realtek adapter's own files (v1: Intel's
 *  ibt-* in linux-firmware, Broadcom's .hcd); null for vendors with none */
export function vendorFirmwareUrl(vendor: string): string | null {
  const v = vendor.toLowerCase();
  if (v.includes("intel")) return LINKS.firmwareIntel;
  if (v.includes("broadcom")) return LINKS.firmwareBroadcom;
  return null;
}
