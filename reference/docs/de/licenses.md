---
title: Lizenzen
layout: default
parent: Startseite
nav_order: 5
---

# Lizenzen
{: .no_toc }

## Inhalt
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Projektlizenz

A2DP Windows Bridge (A2DPWB) ist nach Ihrer Wahl unter der **Apache License 2.0** oder der **MIT License** doppelt lizenziert (SPDX `Apache-2.0 OR MIT`). Siehe [LICENSE](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE), [LICENSE-APACHE](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE-APACHE) und [LICENSE-MIT](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE-MIT).

{: .warning }
Der Quellcode von A2DPWB ist unter Apache-2.0 oder MIT doppelt lizenziert. Die verteilte `a2dpwb-core.exe` enthält BTstack, und A2DPWB wird nur als Gesamtheit seiner Programme verteilt; das Release darf daher nur für private, nicht kommerzielle Zwecke verwendet und weitergegeben werden.

## Bibliotheken von Drittanbietern

| Komponente | Feste Version | Lizenz | Verwendung |
|:-----------|:--------------|:-------|:-----------|
| [BTstack](https://github.com/bluekitchen/btstack) | v1.8.1-6-g5bc5cbdbe | BTstack License (BSD-3-Clause-artig mit einer Klausel gegen kommerzielle Nutzung) / kommerziell | Bluetooth-Stack im Benutzermodus |
| rijndael (in BTstack enthalten) | wie in BTstack | Gemeinfrei (Public Domain) | AES (von BTstack verwendet) |
| tinydir (in BTstack enthalten) | wie in BTstack | BSD-2-Clause | Verzeichnisauflistung im Broadcom-Chipsatztreiber von BTstack |
| liblc3 (lc3-google, in BTstack enthalten) | wie in BTstack | Apache-2.0 | LC3-Encoder (LE Audio, experimentell) |
| micro-ecc (in BTstack enthalten) | wie in BTstack | BSD-2-Clause | P-256 für das Pairing mit LE Secure Connections (LE Audio, experimentell) |
| Bluedroid-SBC-Encoder, nach Rust portiert | portiert aus der Kopie in BTstack | Apache-2.0 | SBC-Encoder |
| [libldac (AOSP)](https://android.googlesource.com/platform/external/libldac) | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 | LDAC-Encoder |
| [libopenaptx](https://github.com/pali/libopenaptx) | 0.2.0 | LGPL-2.1+ | Encoder für aptX / aptX HD / aptX LL |
| [fdk-aac](https://github.com/mstorsjo/fdk-aac) | v2.0.3-158-gd8e6b1a | FDK AAC License | AAC-LC-Encoder |
| [LHDC-V5-Encoder](https://github.com/WillyBilly06/LHDC-V5-Encoder) (C-Portierung des AOSP-Encoders) | Commit 3f9d198 | Apache-2.0 (FFT: BSD-3-Clause, KISS FFT) | LHDC-V5-Encoder |
| [libopus](https://opus-codec.org/) | 1.5.2 | BSD-3-Clause | Opus-Encoder |
| Rust-Crates: [Tauri](https://tauri.app/) 2.12, tokio, serde, windows, nusb, wasapi, tray-icon, ... | `Cargo.lock` | Überwiegend Apache-2.0 / MIT; außerdem BSD, ISC, Zlib, Unicode-3.0, CDLA-Permissive-2.0 und MPL-2.0 | Fenster, Infobereich, IPC, USB, Audio, Updates |
| [SolidJS](https://www.solidjs.com/) | 1.9.17 | MIT | UI-Bibliothek des Fensters |
| [`@tauri-apps/api`](https://github.com/tauri-apps/tauri) | 2.12.4 | Apache-2.0 OR MIT | JavaScript-Seite von Tauri |
| [Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons) | wie in die Oberfläche kopiert | MIT | Symbole |

`a2dpwb-core.exe` enthält BTstack und die Codec-Bibliotheken, `a2dpwb.exe` das Fenster (Tauri, SolidJS, die Symbole), `a2dpwb-driver.exe` nur Rust-Code. Das Fenster wird von der Microsoft Edge WebView2 Runtime dargestellt, die A2DPWB nicht mitliefert (das Installationsprogramm führt den Bootstrapper von Microsoft aus, wenn sie fehlt).

Die vollständigen Lizenztexte und Hinweise zur Einhaltung finden Sie in [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md). Das Installationsprogramm und das portable ZIP legen die Lizenztexte in einem Ordner `licenses/` neben den Programmen ab (die Rust-Crates in `licenses/rust-crates.txt`, erzeugt von cargo-about), zusammen mit `THIRD_PARTY_LICENSES.md` und einer Datei `SOURCE.txt` mit den genauen Quellcode-Commits. Realtek-Firmware und Zadig werden nicht mit A2DPWB verteilt (nur Links).

## Wichtige Hinweise

### BTstack-Lizenz (nicht kommerziell)

BTstack steht **nicht** unter BSD-3-Clause, und seine Lizenz ist keine von der OSI anerkannte Open-Source-Lizenz. Die Lizenz (SPDX `LicenseRef-BTstack`) enthält die drei Bedingungen von BSD-3-Clause und zusätzlich eine vierte Klausel:

> 4. Any redistribution, use, or modification is done solely for personal benefit and not for any commercial purpose or for monetary gain.

(Sinngemäß: Weitergabe, Nutzung oder Änderung erfolgen ausschließlich zum persönlichen Nutzen und nicht zu kommerziellen Zwecken oder zur Gewinnerzielung.)

Da `a2dpwb-core.exe` BTstack enthält, gilt diese Einschränkung für die verteilten Programme. Für eine kommerzielle Nutzung ist eine kommerzielle Lizenz von [BlueKitchen GmbH](https://bluekitchen-gmbh.com/) (contact@bluekitchen-gmbh.com) erforderlich.

### LDAC-Zertifizierung

Im NOTICE von libldac heißt es: "Taking the certification process is required to use LDAC in your products." (Um LDAC in Produkten zu verwenden, ist der Zertifizierungsprozess zu durchlaufen.) Siehe [sony.net/Products/LDAC/aosp](https://www.sony.net/Products/LDAC/aosp/). Ob der Zertifizierungsprozess von Sony für eine kostenlose, nicht kommerzielle Anwendung wie A2DPWB gilt, wurde nicht geprüft; A2DPWB ist nicht zertifiziert.

### LHDC-Marke und -Zertifizierung

LHDC ist ein Codec der Savitech Corp., der von der Allianz Hi-Res Wireless Audio (HWA) gefördert wird. Der von A2DPWB verwendete Encoder ist Apache-2.0-Code, abgeleitet von dem Encoder, den Google in AOSP (Android 17) veröffentlicht hat. Apache-2.0 gewährt jedoch keine Markenrechte, und Produkte, die mit LHDC werben, durchlaufen normalerweise eine Zertifizierung durch Savitech / HWA. A2DPWB ist nicht zertifiziert; die Unterstützung für LHDC V5 ist eine nicht kommerzielle Interoperabilitätsfunktion.

### Version von libopenaptx

libopenaptx ist auf **0.2.0** festgelegt, das letzte Release unter LGPL-2.1+. Ab 0.2.1 steht es unter GPL-3.0-or-later mit zusätzlichen Einschränkungen, was mit den anderen in A2DPWB eingebundenen Bibliotheken nicht vereinbar ist. Releases bis v1.0.4 wurden versehentlich mit 0.2.1 gebaut; bitte verwenden Sie v1.0.5 oder neuer.

libopenaptx ist statisch in `a2dpwb-core.exe` gelinkt. Das Installationsprogramm und das ZIP liefern den Text der LGPL-2.1 und THIRD_PARTY_LICENSES.md mit dem Copyright-Vermerk von libopenaptx mit, **Einstellungen > Info über A2DPWB** verweist auf die Lizenzen von Drittanbietern, und der vollständige zugehörige Quellcode (zum Neubauen/Neulinken mit einer geänderten libopenaptx) ist im Repository beim getaggten Commit des Releases samt Submodulen verfügbar.

### Patenthinweis zu aptX

aptX (klassisch), aptX HD, aptX Low Latency und aptX Adaptive sind Marken von Qualcomm. Laut seiner README ist libopenaptx eine Open-Source-Implementierung von aptX, „derived from ffmpeg 4.0 project“ (abgeleitet vom Projekt ffmpeg 4.0); sie wird hier für aptX, aptX HD und aptX Low Latency verwendet. Die aptX-Kodierung kann in manchen Rechtsordnungen patentgeschützt sein. A2DPWB enthält keinen aptX-Adaptive-Encoder.

### Lizenz von fdk-aac und Patenthinweis zu AAC

Die Lizenz von fdk-aac verlangt, dass der vollständige Lizenztext jeder Weitergabe von Binärdateien beiliegt und dass der vollständige Quellcode des FDK AAC Codec (einschließlich aller Änderungen) den Empfängern der Binärdatei kostenlos zur Verfügung gestellt wird. Sie gewährt außerdem **keine Patentlizenz**: AAC kann patentgeschützt sein (das NOTICE verweist auf Via Licensing), und eine gegebenenfalls erforderliche AAC-Patentlizenz zu beschaffen, liegt in der Verantwortung des Benutzers.

### Rust-Crates und MPL-2.0

Die Lizenzen der Rust-Crates werden beim Bauen eines Releases geprüft: `cargo-about` bricht bei jeder Lizenz ab, die nicht in der Liste der akzeptierten Lizenzen steht (`about.toml`, identisch mit `deny.toml`). Einige Crates stehen unter MPL-2.0 (cssparser, selectors, dtoa-short, option-ext); sie werden unverändert verwendet, und ihr Quellcode ist auf crates.io in den Versionen aus `Cargo.lock` verfügbar. Siehe THIRD_PARTY_LICENSES.md §9.

### Entwicklerwerkzeug a2dpwb_decode

`a2dpwb_decode` (siehe [Verwendung](usage#verify-stream)) wird nur aus dem Quellcode gebaut und ist nicht im Release enthalten. Es bindet den Bluedroid-SBC-Decoder (Apache-2.0), fdk-aac (FDK AAC License), libopenaptx 0.2.0 (LGPL-2.1+), libopus (BSD-3-Clause) und liblc3 (Apache-2.0, in BTstack enthalten) in den oben genannten festen Versionen ein und bindet BTstack und libldac **nicht** ein. Siehe THIRD_PARTY_LICENSES.md §14.

### Testumgebung tools/emu und tools/linux_sink

`tools/emu` (siehe [Bauen](building)) und der Messempfänger `tools/linux_sink` (siehe [Verwendung](usage#receiver)) verwenden [Bumble](https://github.com/google/bumble) 0.0.234 (Apache-2.0) und dessen Abhängigkeiten, die in festen Versionen in lokale, von git ignorierte Umgebungen heruntergeladen werden. Nichts davon wird in A2DPWB eingebunden oder mit A2DPWB ausgeliefert. Die Paketliste und Lizenzen finden Sie in THIRD_PARTY_LICENSES.md §15.

### Bluetooth-Qualifizierung

Kommerzielle Bluetooth-Produkte müssen den von der Bluetooth SIG verwalteten [Bluetooth Qualification Process](https://www.bluetooth.com/) durchlaufen.
