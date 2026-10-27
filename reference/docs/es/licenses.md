---
title: Licencias
layout: default
parent: Inicio
nav_order: 5
---

# Licencias
{: .no_toc }

## Contenido
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Licencia del proyecto

A2DP Windows Bridge (A2DPWB) tiene licencia dual **Apache License 2.0** o **MIT License**, a su elección (SPDX `Apache-2.0 OR MIT`). Consulte [LICENSE](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE), [LICENSE-APACHE](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE-APACHE) y [LICENSE-MIT](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE-MIT).

{: .warning }
El código fuente de A2DPWB tiene licencia dual Apache-2.0 o MIT. El `a2dpwb-core.exe` distribuido incluye BTstack, y A2DPWB solo se distribuye como el conjunto de sus programas, por lo que la versión publicada solo puede usarse y redistribuirse con fines personales y no comerciales.

## Bibliotecas de terceros

| Componente | Versión fijada | Licencia | Uso |
|:----------|:---------------|:--------|:------|
| [BTstack](https://github.com/bluekitchen/btstack) | v1.8.1-6-g5bc5cbdbe | Licencia de BTstack (tipo BSD-3-Clause con una cláusula no comercial) / Comercial | Pila Bluetooth en modo de usuario |
| rijndael (incluido en BTstack) | la de BTstack | Dominio público | AES (lo usa BTstack) |
| tinydir (incluido en BTstack) | la de BTstack | BSD-2-Clause | Listado de directorios en el controlador de chipsets Broadcom de BTstack |
| liblc3 (lc3-google, incluido en BTstack) | la de BTstack | Apache-2.0 | Codificador LC3 (LE Audio, experimental) |
| micro-ecc (incluido en BTstack) | la de BTstack | BSD-2-Clause | P-256 para el emparejamiento LE Secure Connections (LE Audio, experimental) |
| Codificador SBC de Bluedroid, portado a Rust | portado de la copia incluida en BTstack | Apache-2.0 | Codificador SBC |
| [libldac (AOSP)](https://android.googlesource.com/platform/external/libldac) | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 | Codificador LDAC |
| [libopenaptx](https://github.com/pali/libopenaptx) | 0.2.0 | LGPL-2.1+ | Codificador aptX / aptX HD / aptX LL |
| [fdk-aac](https://github.com/mstorsjo/fdk-aac) | v2.0.3-158-gd8e6b1a | FDK AAC License | Codificador AAC-LC |
| [LHDC-V5-Encoder](https://github.com/WillyBilly06/LHDC-V5-Encoder) (port en C del codificador de AOSP) | commit 3f9d198 | Apache-2.0 (FFT: BSD-3-Clause, KISS FFT) | Codificador LHDC V5 |
| [libopus](https://opus-codec.org/) | 1.5.2 | BSD-3-Clause | Codificador Opus |
| Crates de Rust: [Tauri](https://tauri.app/) 2.12, tokio, serde, windows, nusb, wasapi, tray-icon, ... | `Cargo.lock` | Sobre todo Apache-2.0 / MIT; también BSD, ISC, Zlib, Unicode-3.0, CDLA-Permissive-2.0 y MPL-2.0 | Ventana, bandeja, IPC, USB, audio, actualizaciones |
| [SolidJS](https://www.solidjs.com/) | 1.9.17 | MIT | Biblioteca de interfaz de la ventana |
| [`@tauri-apps/api`](https://github.com/tauri-apps/tauri) | 2.12.4 | Apache-2.0 OR MIT | Parte de JavaScript de Tauri |
| [Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons) | la copiada en la interfaz | MIT | Iconos |

`a2dpwb-core.exe` contiene BTstack y las bibliotecas de códecs; `a2dpwb.exe`, la ventana (Tauri, SolidJS, los iconos); `a2dpwb-driver.exe`, solo código Rust. La ventana la representa Microsoft Edge WebView2 Runtime, que A2DPWB no incluye (el instalador ejecuta el programa de arranque de Microsoft si falta).

Para ver los textos completos de las licencias y los detalles de cumplimiento, consulte [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md). El instalador y el zip portable colocan los textos de las licencias en una carpeta `licenses/` junto a los programas (los crates de Rust en `licenses/rust-crates.txt`, generado por cargo-about), con `THIRD_PARTY_LICENSES.md` y un archivo `SOURCE.txt` con los commits exactos del código fuente. El firmware de Realtek y Zadig no se distribuyen con A2DPWB (solo enlaces).

## Notas importantes

### Licencia de BTstack (no comercial)

BTstack **no** es BSD-3-Clause ni tiene una licencia de código abierto aprobada por la OSI. Su licencia (SPDX `LicenseRef-BTstack`) tiene las tres condiciones de BSD-3-Clause más una cuarta cláusula:

> 4. Any redistribution, use, or modification is done solely for personal benefit and not for any commercial purpose or for monetary gain.

(Toda redistribución, uso o modificación debe hacerse únicamente en beneficio personal y no con fines comerciales ni de lucro.)

Como `a2dpwb-core.exe` contiene BTstack, esta restricción se aplica a los programas distribuidos. El uso comercial requiere una licencia comercial de [BlueKitchen GmbH](https://bluekitchen-gmbh.com/) (contact@bluekitchen-gmbh.com).

### Certificación LDAC

El archivo NOTICE de libldac indica: "Taking the certification process is required to use LDAC in your products." (es obligatorio pasar el proceso de certificación para usar LDAC en sus productos). Consulte [sony.net/Products/LDAC/aosp](https://www.sony.net/Products/LDAC/aosp/). No se ha verificado si el proceso de certificación de Sony se aplica a una aplicación gratuita y no comercial como A2DPWB; A2DPWB no ha sido certificado.

### Marca y certificación LHDC

LHDC es un códec de Savitech Corp., promovido por la alianza Hi-Res Wireless Audio (HWA). El codificador que usa A2DPWB es código Apache-2.0 derivado del que Google publicó en AOSP (Android 17), pero Apache-2.0 no concede derechos de marca, y los productos que anuncian LHDC normalmente pasan por la certificación de Savitech / HWA. A2DPWB no ha sido certificado; la compatibilidad con LHDC V5 es una función de interoperabilidad no comercial.

### Versión de libopenaptx

libopenaptx está fijada en la versión **0.2.0**, la última publicada bajo LGPL-2.1+. A partir de la 0.2.1 su licencia es GPL-3.0-or-later con restricciones adicionales, lo que no es compatible con las demás bibliotecas enlazadas en A2DPWB. Las versiones hasta la v1.0.4 se compilaron por error con la 0.2.1; use la v1.0.5 o posterior.

libopenaptx se enlaza de forma estática en `a2dpwb-core.exe`. El instalador y el zip incluyen el texto de la LGPL-2.1 y THIRD_PARTY_LICENSES.md con el copyright de libopenaptx, **Configuración > Acerca de A2DPWB** enlaza a las licencias de terceros, y el código fuente correspondiente completo (para recompilar/volver a enlazar con una libopenaptx modificada) está disponible en el repositorio, en el commit etiquetado de la versión junto con sus submódulos.

### Aviso de patentes de aptX

aptX (clásico), aptX HD, aptX Low Latency y aptX Adaptive son marcas comerciales de Qualcomm. Según su README, libopenaptx es una implementación de código abierto de aptX "derived from ffmpeg 4.0 project" (derivada del proyecto ffmpeg 4.0); aquí se usa para aptX, aptX HD y aptX Low Latency. La codificación aptX puede estar cubierta por patentes en algunas jurisdicciones. A2DPWB no incluye un codificador aptX Adaptive.

### Licencia de fdk-aac y aviso de patentes de AAC

La licencia de fdk-aac exige que el texto completo de la licencia acompañe a las redistribuciones binarias y que el código fuente completo del FDK AAC Codec (y de cualquier modificación) se ponga a disposición de forma gratuita de quienes reciben el binario. Además, **no concede ninguna licencia de patentes**: AAC puede estar cubierto por patentes (el NOTICE remite a Via Licensing), y obtener cualquier licencia de patentes de AAC necesaria es responsabilidad del usuario.

### Crates de Rust y MPL-2.0

Las licencias de los crates de Rust se comprueban al compilar una versión: `cargo-about` falla con cualquier licencia que no esté en la lista aceptada (`about.toml`, la misma que `deny.toml`). Algunos crates están bajo MPL-2.0 (cssparser, selectors, dtoa-short, option-ext); se usan sin modificar, y su código fuente está disponible en crates.io en las versiones de `Cargo.lock`. Consulte THIRD_PARTY_LICENSES.md §9.

### Herramienta para desarrolladores a2dpwb_decode

`a2dpwb_decode` (consulte [Uso](usage#verify-stream)) solo se compila desde el código fuente y no está en la versión publicada. Enlaza el decodificador SBC de Bluedroid (Apache-2.0), fdk-aac (FDK AAC License), libopenaptx 0.2.0 (LGPL-2.1+), libopus (BSD-3-Clause) y liblc3 (Apache-2.0, incluido en BTstack) en las versiones fijadas indicadas arriba, y **no** enlaza BTstack ni libldac. Consulte THIRD_PARTY_LICENSES.md §14.

### Entorno de pruebas tools/emu y tools/linux_sink

`tools/emu` (consulte [Compilación](building)) y el receptor de medición `tools/linux_sink` (consulte [Uso](usage#receiver)) usan [Bumble](https://github.com/google/bumble) 0.0.234 (Apache-2.0) y sus dependencias, descargados en entornos locales ignorados por git, en versiones fijadas. Nada de ello se enlaza en A2DPWB ni se distribuye con él. Consulte THIRD_PARTY_LICENSES.md §15 para ver la lista de paquetes y sus licencias.

### Cualificación Bluetooth

Los productos Bluetooth comerciales deben pasar el [Bluetooth Qualification Process](https://www.bluetooth.com/) gestionado por el Bluetooth SIG.
