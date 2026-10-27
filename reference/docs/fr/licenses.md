---
title: Licences
layout: default
parent: Accueil
nav_order: 5
---

# Licences
{: .no_toc }

## Table des matières
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Licence du projet

A2DP Windows Bridge (A2DPWB) est sous double licence **Apache License 2.0** ou **MIT License**, au choix (SPDX `Apache-2.0 OR MIT`). Voir [LICENSE](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE), [LICENSE-APACHE](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE-APACHE) et [LICENSE-MIT](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/LICENSE-MIT).

{: .warning }
Le code source d'A2DPWB est sous double licence Apache-2.0 ou MIT. Le `a2dpwb-core.exe` distribué inclut BTstack, et A2DPWB n'est distribué que sous la forme de l'ensemble de ses programmes : la version publiée ne peut donc être utilisée et redistribuée qu'à des fins personnelles et non commerciales.

## Bibliothèques tierces

| Composant | Version figée | Licence | Utilisation |
|:----------|:---------------|:--------|:------|
| [BTstack](https://github.com/bluekitchen/btstack) | v1.8.1-6-g5bc5cbdbe | BTstack License (de type BSD-3-Clause avec une clause non commerciale) / Commerciale | Pile Bluetooth en mode utilisateur |
| rijndael (inclus dans BTstack) | celle de BTstack | Domaine public | AES (utilisé par BTstack) |
| tinydir (inclus dans BTstack) | celle de BTstack | BSD-2-Clause | Liste des fichiers d'un répertoire dans le pilote de puce Broadcom de BTstack |
| liblc3 (lc3-google, inclus dans BTstack) | celle de BTstack | Apache-2.0 | Encodeur LC3 (LE Audio, expérimental) |
| micro-ecc (inclus dans BTstack) | celle de BTstack | BSD-2-Clause | P-256 pour l'appairage LE Secure Connections (LE Audio, expérimental) |
| Encodeur SBC Bluedroid, porté en Rust | porté depuis la copie de BTstack | Apache-2.0 | Encodeur SBC |
| [libldac (AOSP)](https://android.googlesource.com/platform/external/libldac) | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 | Encodeur LDAC |
| [libopenaptx](https://github.com/pali/libopenaptx) | 0.2.0 | LGPL-2.1+ | Encodeur aptX / aptX HD / aptX LL |
| [fdk-aac](https://github.com/mstorsjo/fdk-aac) | v2.0.3-158-gd8e6b1a | FDK AAC License | Encodeur AAC-LC |
| [LHDC-V5-Encoder](https://github.com/WillyBilly06/LHDC-V5-Encoder) (portage en C de l'encodeur AOSP) | commit 3f9d198 | Apache-2.0 (FFT : BSD-3-Clause, KISS FFT) | Encodeur LHDC V5 |
| [libopus](https://opus-codec.org/) | 1.5.2 | BSD-3-Clause | Encodeur Opus |
| Crates Rust : [Tauri](https://tauri.app/) 2.12, tokio, serde, windows, nusb, wasapi, tray-icon, ... | `Cargo.lock` | Principalement Apache-2.0 / MIT ; aussi BSD, ISC, Zlib, Unicode-3.0, CDLA-Permissive-2.0 et MPL-2.0 | Fenêtre, zone de notification, IPC, USB, audio, mises à jour |
| [SolidJS](https://www.solidjs.com/) | 1.9.17 | MIT | Bibliothèque d'interface de la fenêtre |
| [`@tauri-apps/api`](https://github.com/tauri-apps/tauri) | 2.12.4 | Apache-2.0 OR MIT | Partie JavaScript de Tauri |
| [Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons) | celle copiée dans l'interface | MIT | Icônes |

`a2dpwb-core.exe` contient BTstack et les bibliothèques de codecs, `a2dpwb.exe` la fenêtre (Tauri, SolidJS, les icônes), `a2dpwb-driver.exe` uniquement du code Rust. La fenêtre est affichée par le runtime Microsoft Edge WebView2, qu'A2DPWB ne fournit pas (le programme d'installation lance le programme d'amorçage de Microsoft s'il est absent).

Pour les textes complets des licences et les détails de conformité, voir [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md). Le programme d'installation et le zip portable placent les textes des licences dans un dossier `licenses/` à côté des programmes (les crates Rust dans `licenses/rust-crates.txt`, généré par cargo-about), avec `THIRD_PARTY_LICENSES.md` et un fichier `SOURCE.txt` listant les commits sources exacts. Le firmware Realtek et Zadig ne sont pas distribués avec A2DPWB (liens uniquement).

## Remarques importantes

### Licence BTstack (non commerciale)

BTstack n'est **pas** sous BSD-3-Clause et sa licence n'est pas une licence open source approuvée par l'OSI. Sa licence (SPDX `LicenseRef-BTstack`) reprend les trois conditions de la BSD-3-Clause, plus une quatrième clause :

> 4. Any redistribution, use, or modification is done solely for personal benefit and not for any commercial purpose or for monetary gain.

(Toute redistribution, utilisation ou modification doit se faire uniquement à titre personnel, et non à des fins commerciales ou lucratives.)

Comme `a2dpwb-core.exe` contient BTstack, cette restriction s'applique aux programmes distribués. Une utilisation commerciale nécessite une licence commerciale de [BlueKitchen GmbH](https://bluekitchen-gmbh.com/) (contact@bluekitchen-gmbh.com).

### Certification LDAC

Le fichier NOTICE de libldac indique : "Taking the certification process is required to use LDAC in your products." (le processus de certification est obligatoire pour utiliser LDAC dans vos produits). Voir [sony.net/Products/LDAC/aosp](https://www.sony.net/Products/LDAC/aosp/). Il n'a pas été vérifié si le processus de certification de Sony s'applique à une application gratuite et non commerciale comme A2DPWB ; A2DPWB n'a pas été certifié.

### Marque et certification LHDC

LHDC est un codec de Savitech Corp., promu par l'alliance Hi-Res Wireless Audio (HWA). L'encodeur utilisé par A2DPWB est du code Apache-2.0 dérivé de celui que Google a publié dans AOSP (Android 17), mais la licence Apache-2.0 n'accorde aucun droit sur les marques, et les produits qui mettent en avant LHDC passent normalement par une certification Savitech / HWA. A2DPWB n'a pas été certifié ; la prise en charge de LHDC V5 est une fonction d'interopérabilité non commerciale.

### Version de libopenaptx

libopenaptx est figée à la version **0.2.0**, la dernière publiée sous LGPL-2.1+. À partir de la 0.2.1, elle est sous licence GPL-3.0-or-later avec des restrictions supplémentaires, ce qui n'est pas compatible avec les autres bibliothèques liées à A2DPWB. Les versions jusqu'à v1.0.4 ont été compilées par erreur avec la 0.2.1 ; veuillez utiliser la v1.0.5 ou ultérieure.

libopenaptx est liée statiquement à `a2dpwb-core.exe`. Le programme d'installation et le zip fournissent le texte de la LGPL-2.1 ainsi que THIRD_PARTY_LICENSES.md avec le copyright de libopenaptx, **Paramètres > À propos d'A2DPWB** renvoie aux licences tierces, et le code source correspondant complet (pour recompiler/relier avec une libopenaptx modifiée) est disponible dans le dépôt, au commit étiqueté de la version, avec ses sous-modules.

### Avis sur les brevets aptX

aptX (classique), aptX HD, aptX Low Latency et aptX Adaptive sont des marques de Qualcomm. D'après son README, libopenaptx est une implémentation open source de l'aptX « dérivée du projet ffmpeg 4.0 » ; elle est utilisée ici pour l'aptX, l'aptX HD et l'aptX Low Latency. L'encodage aptX peut être couvert par des brevets dans certaines juridictions. A2DPWB n'inclut pas d'encodeur aptX Adaptive.

### Licence fdk-aac et avis sur les brevets AAC

La licence fdk-aac exige que le texte complet de la licence accompagne toute redistribution binaire et que le code source complet du FDK AAC Codec (et de toute modification) soit mis gratuitement à la disposition des destinataires du binaire. Elle n'accorde par ailleurs **aucune licence de brevet** : l'AAC peut être couvert par des brevets (le NOTICE mentionne Via Licensing), et l'obtention de toute licence de brevet AAC nécessaire relève de la responsabilité de l'utilisateur.

### Crates Rust et MPL-2.0

Les licences des crates Rust sont vérifiées lors de la compilation d'une version : `cargo-about` échoue sur toute licence absente de la liste acceptée (`about.toml`, identique à `deny.toml`). Quelques crates sont sous MPL-2.0 (cssparser, selectors, dtoa-short, option-ext) ; elles sont utilisées sans modification, et leur code source est disponible sur crates.io aux versions indiquées dans `Cargo.lock`. Voir THIRD_PARTY_LICENSES.md §9.

### Outil de développement a2dpwb_decode

`a2dpwb_decode` (voir [Utilisation](usage#verify-stream)) se compile uniquement depuis les sources et ne figure pas dans la version publiée. Il est lié au décodeur SBC Bluedroid (Apache-2.0), à fdk-aac (FDK AAC License), à libopenaptx 0.2.0 (LGPL-2.1+), à libopus (BSD-3-Clause) et à liblc3 (Apache-2.0, inclus dans BTstack) dans les versions figées ci-dessus, et n'est **pas** lié à BTstack ni à libldac. Voir THIRD_PARTY_LICENSES.md §14.

### Environnement de test tools/emu et tools/linux_sink

`tools/emu` (voir [Compilation](building)) et le récepteur de mesure `tools/linux_sink` (voir [Utilisation](usage#receiver)) utilisent [Bumble](https://github.com/google/bumble) 0.0.234 (Apache-2.0) et ses dépendances, téléchargés dans des environnements locaux ignorés par git, en versions figées. Rien de tout cela n'est lié à A2DPWB ni livré avec lui. Voir THIRD_PARTY_LICENSES.md §15 pour la liste des paquets et leurs licences.

### Qualification Bluetooth

Les produits Bluetooth commerciaux doivent passer le [Bluetooth Qualification Process](https://www.bluetooth.com/) géré par le Bluetooth SIG.
