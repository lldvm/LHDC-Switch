; A2DPWB v2 installer hooks for Tauri's NSIS bundler
; (app/src/ui/src-tauri/tauri.conf.json: bundle.windows.nsis.installerHooks).
; SPDX-License-Identifier: Apache-2.0 OR MIT
;
; Install:
;   - a running v1 (its mutex A2DPWB_SingleInstance) is asked to exit;
;   - a running v2 core is closed cleanly (`a2dpwb-core.exe --quit`: the
;     stream stops and the adapter is released) and started again after;
;   - v1 installed by Inno Setup is uninstalled. Its uninstaller removes the
;     program files only: the settings folder %APPDATA%\A2DPWB (profiles,
;     pairings, firmware) stays, and v2 reads it as it is;
;   - "start with Windows" (HKCU Run, value A2DPWB, shared with v1) is
;     pointed at `a2dpwb-core.exe --background` when it was on.
; Uninstall: the core is closed; unless updating, the user is asked whether
; the adapters A2DPWB put on WinUSB go back to Windows (one UAC prompt), and
; the Run value is removed when it points at this installation.
;
; Command line: /KEEPV1 leaves an installed v1, a running v1 and the Run
; value alone (testing v2 beside an installed v1; both use the same settings
; folder). The core still replaces v1 when it starts: it asks a running v1
; to exit and takes over v1's Run value (resident.rs, refresh_autostart).
;
; This file is included at the top of Tauri's installer.nsi, after
; LogicLib, FileFunc and StrFunc. Saved as UTF-8 with a BOM.

!define A2DPWB_RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"
!define A2DPWB_RUN_VALUE "A2DPWB"
; v1's AppId (installer/A2DPWB.iss); Inno Setup adds "_is1"
!define A2DPWB_V1_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\{4D8EA889-0FE6-4753-A25F-2C1A3E4BF658}_is1"
!define A2DPWB_V1_MUTEX "A2DPWB_SingleInstance"
!define A2DPWB_CORE "a2dpwb-core.exe"

Var A2dpwbKeepV1
Var A2dpwbCoreWasRunning
Var A2dpwbRunWasSet

; The installer's declarations of StrFunc are for the installer only
${UnStrLoc}
${UnStrCase}

; $OUT = message NAME in the installer's language (English otherwise)
!macro A2DPWB_Text OUT NAME
  !if "${NAME}" == "v1_running"
    StrCpy ${OUT} "A2DP Windows Bridge v1 is running. Exit it (right-click its tray icon > Exit), then click Retry.$\n$\nCancel stops the installation."
    ${If} $LANGUAGE = 1041
      StrCpy ${OUT} "A2DP Windows Bridge v1 が起動しています。タスクトレイのアイコンを右クリックして「終了」を選んでから、「再試行」を押してください。$\n$\n「キャンセル」でインストールを中止します。"
    ${ElseIf} $LANGUAGE = 1031
      StrCpy ${OUT} "A2DP Windows Bridge v1 läuft noch. Beenden Sie es (Rechtsklick auf das Taskleistensymbol > Beenden) und klicken Sie dann auf Wiederholen.$\n$\nAbbrechen beendet die Installation."
    ${ElseIf} $LANGUAGE = 1036
      StrCpy ${OUT} "A2DP Windows Bridge v1 est en cours d'exécution. Quittez-le (clic droit sur son icône dans la zone de notification > Quitter), puis cliquez sur Réessayer.$\n$\nAnnuler arrête l'installation."
    ${ElseIf} $LANGUAGE = 1034
      StrCpy ${OUT} "A2DP Windows Bridge v1 se está ejecutando. Ciérrelo (clic derecho en su icono de la bandeja > Salir) y pulse Reintentar.$\n$\nCancelar detiene la instalación."
    ${ElseIf} $LANGUAGE = 1042
      StrCpy ${OUT} "A2DP Windows Bridge v1이 실행 중입니다. 트레이 아이콘을 마우스 오른쪽 버튼으로 클릭하고 [종료]를 선택한 다음 [다시 시도]를 누르세요.$\n$\n[취소]를 누르면 설치를 중단합니다."
    ${ElseIf} $LANGUAGE = 2052
      StrCpy ${OUT} "A2DP Windows Bridge v1 正在运行。请右键单击其托盘图标并选择“退出”，然后单击“重试”。$\n$\n单击“取消”将停止安装。"
    ${ElseIf} $LANGUAGE = 1028
      StrCpy ${OUT} "A2DP Windows Bridge v1 正在執行。請以滑鼠右鍵按一下其系統匣圖示並選擇「結束」，然後按一下「重試」。$\n$\n按一下「取消」將停止安裝。"
    ${EndIf}
  !else if "${NAME}" == "v1_left"
    StrCpy ${OUT} "A2DP Windows Bridge v1 could not be uninstalled. You can remove it later in Settings > Apps; your settings, pairings and firmware stay in either case."
    ${If} $LANGUAGE = 1041
      StrCpy ${OUT} "A2DP Windows Bridge v1 をアンインストールできませんでした。あとで「設定」>「アプリ」から削除できます。設定、ペアリング、ファームウェアはどちらの場合も残ります。"
    ${ElseIf} $LANGUAGE = 1031
      StrCpy ${OUT} "A2DP Windows Bridge v1 konnte nicht deinstalliert werden. Sie können es später unter Einstellungen > Apps entfernen; Ihre Einstellungen, Kopplungen und Firmware bleiben in jedem Fall erhalten."
    ${ElseIf} $LANGUAGE = 1036
      StrCpy ${OUT} "A2DP Windows Bridge v1 n'a pas pu être désinstallé. Vous pourrez le supprimer plus tard dans Paramètres > Applications ; vos réglages, appairages et firmware sont conservés dans tous les cas."
    ${ElseIf} $LANGUAGE = 1034
      StrCpy ${OUT} "No se pudo desinstalar A2DP Windows Bridge v1. Puede quitarlo más tarde en Configuración > Aplicaciones; su configuración, emparejamientos y firmware se conservan en cualquier caso."
    ${ElseIf} $LANGUAGE = 1042
      StrCpy ${OUT} "A2DP Windows Bridge v1을 제거하지 못했습니다. 나중에 [설정] > [앱]에서 제거할 수 있습니다. 설정, 페어링, 펌웨어는 어느 경우에도 유지됩니다."
    ${ElseIf} $LANGUAGE = 2052
      StrCpy ${OUT} "无法卸载 A2DP Windows Bridge v1。您可以稍后在“设置”>“应用”中将其删除；无论如何，您的设置、配对和固件都会保留。"
    ${ElseIf} $LANGUAGE = 1028
      StrCpy ${OUT} "無法解除安裝 A2DP Windows Bridge v1。您可以稍後在「設定」>「應用程式」中將其移除；無論如何，您的設定、配對和韌體都會保留。"
    ${EndIf}
  !else if "${NAME}" == "restore_ask"
    StrCpy ${OUT} "A2DP Windows Bridge assigned the WinUSB driver to your USB Bluetooth adapter, which hides it from Windows' Bluetooth. Give the adapter back to Windows now? Windows asks for administrator permission.$\n$\nNo leaves it on WinUSB; you can restore it later in Device Manager."
    ${If} $LANGUAGE = 1041
      StrCpy ${OUT} "A2DP Windows Bridge が USB Bluetooth アダプターに WinUSB ドライバーを割り当てたため、そのアダプターは Windows の Bluetooth から見えなくなっています。今アダプターを Windows に戻しますか？ Windows が管理者の許可を求めます。$\n$\n「いいえ」を選ぶと WinUSB のまま残ります。あとでデバイス マネージャーから戻せます。"
    ${ElseIf} $LANGUAGE = 1031
      StrCpy ${OUT} "A2DP Windows Bridge hat Ihrem USB-Bluetooth-Adapter den WinUSB-Treiber zugewiesen; dadurch sieht das Bluetooth von Windows ihn nicht. Den Adapter jetzt an Windows zurückgeben? Windows fragt nach Administratorrechten.$\n$\nNein lässt ihn auf WinUSB; Sie können ihn später im Geräte-Manager zurücksetzen."
    ${ElseIf} $LANGUAGE = 1036
      StrCpy ${OUT} "A2DP Windows Bridge a attribué le pilote WinUSB à votre adaptateur Bluetooth USB, ce qui le cache au Bluetooth de Windows. Rendre l'adaptateur à Windows maintenant ? Windows demande les droits d'administrateur.$\n$\nNon le laisse sur WinUSB ; vous pourrez le rétablir plus tard dans le Gestionnaire de périphériques."
    ${ElseIf} $LANGUAGE = 1034
      StrCpy ${OUT} "A2DP Windows Bridge asignó el controlador WinUSB a su adaptador Bluetooth USB, por lo que el Bluetooth de Windows no lo ve. ¿Devolver ahora el adaptador a Windows? Windows pedirá permisos de administrador.$\n$\nNo lo deja en WinUSB; puede restaurarlo más tarde en el Administrador de dispositivos."
    ${ElseIf} $LANGUAGE = 1042
      StrCpy ${OUT} "A2DP Windows Bridge가 USB 블루투스 어댑터에 WinUSB 드라이버를 할당했기 때문에 Windows 블루투스에서 이 어댑터가 보이지 않습니다. 지금 어댑터를 Windows에 돌려주시겠습니까? Windows가 관리자 권한을 요청합니다.$\n$\n[아니요]를 누르면 WinUSB로 남습니다. 나중에 장치 관리자에서 되돌릴 수 있습니다."
    ${ElseIf} $LANGUAGE = 2052
      StrCpy ${OUT} "A2DP Windows Bridge 已为您的 USB 蓝牙适配器分配 WinUSB 驱动程序，因此 Windows 蓝牙看不到该适配器。现在将适配器还给 Windows 吗？Windows 会请求管理员权限。$\n$\n选择“否”将保留 WinUSB；之后可以在设备管理器中恢复。"
    ${ElseIf} $LANGUAGE = 1028
      StrCpy ${OUT} "A2DP Windows Bridge 已為您的 USB 藍牙介面卡指派 WinUSB 驅動程式，因此 Windows 藍牙看不到該介面卡。現在要將介面卡交還給 Windows 嗎？Windows 會要求系統管理員權限。$\n$\n選擇「否」會保留 WinUSB；之後可以在裝置管理員中還原。"
    ${EndIf}
  !else if "${NAME}" == "restore_v1_note"
    StrCpy ${OUT} "$\n$\nA2DP Windows Bridge v1 is still on this PC and needs WinUSB too: choose No to keep using the adapter with v1."
    ${If} $LANGUAGE = 1041
      StrCpy ${OUT} "$\n$\nこの PC には A2DP Windows Bridge v1 が残っていて、v1 も WinUSB を使います。v1 でアダプターを使い続けるなら「いいえ」を選んでください。"
    ${ElseIf} $LANGUAGE = 1031
      StrCpy ${OUT} "$\n$\nA2DP Windows Bridge v1 ist noch auf diesem PC und braucht WinUSB ebenfalls: Wählen Sie Nein, um den Adapter weiter mit v1 zu verwenden."
    ${ElseIf} $LANGUAGE = 1036
      StrCpy ${OUT} "$\n$\nA2DP Windows Bridge v1 est encore sur ce PC et a aussi besoin de WinUSB : choisissez Non pour continuer à utiliser l'adaptateur avec v1."
    ${ElseIf} $LANGUAGE = 1034
      StrCpy ${OUT} "$\n$\nA2DP Windows Bridge v1 sigue en este equipo y también necesita WinUSB: elija No para seguir usando el adaptador con v1."
    ${ElseIf} $LANGUAGE = 1042
      StrCpy ${OUT} "$\n$\n이 PC에는 A2DP Windows Bridge v1이 남아 있으며 v1도 WinUSB를 사용합니다. v1에서 어댑터를 계속 사용하려면 [아니요]를 선택하세요."
    ${ElseIf} $LANGUAGE = 2052
      StrCpy ${OUT} "$\n$\n此电脑上仍有 A2DP Windows Bridge v1，它也需要 WinUSB：如需继续在 v1 中使用该适配器，请选择“否”。"
    ${ElseIf} $LANGUAGE = 1028
      StrCpy ${OUT} "$\n$\n此電腦上仍有 A2DP Windows Bridge v1，它也需要 WinUSB：若要繼續在 v1 中使用該介面卡，請選擇「否」。"
    ${EndIf}
  !else if "${NAME}" == "restore_failed"
    StrCpy ${OUT} "The adapter could not be given back to Windows completely: it may be unplugged, or Windows chose WinUSB again. Plug it in, right-click it in Device Manager, choose Uninstall device, then unplug and replug it (see $\"By hand in Device Manager$\" in the setup guide)."
    ${If} $LANGUAGE = 1041
      StrCpy ${OUT} "アダプターを Windows に戻しきれませんでした。アダプターが抜かれているか、Windows がまた WinUSB を選んだ可能性があります。アダプターを挿した状態でデバイス マネージャーを開き、アダプターを右クリックして「デバイスのアンインストール」を選んでから、抜いて挿し直してください（セットアップガイドの「デバイス マネージャーで手動で行う」を参照）。"
    ${ElseIf} $LANGUAGE = 1031
      StrCpy ${OUT} "Der Adapter konnte nicht vollständig an Windows zurückgegeben werden: Er ist vielleicht nicht eingesteckt, oder Windows hat wieder WinUSB gewählt. Stecken Sie ihn ein, klicken Sie im Geräte-Manager mit der rechten Maustaste darauf, wählen Sie Gerät deinstallieren, und stecken Sie ihn dann aus und wieder ein (siehe „Von Hand im Geräte-Manager“ in der Einrichtungsanleitung)."
    ${ElseIf} $LANGUAGE = 1036
      StrCpy ${OUT} "L'adaptateur n'a pas pu être entièrement rendu à Windows : il est peut-être débranché, ou Windows a de nouveau choisi WinUSB. Branchez-le, faites un clic droit dessus dans le Gestionnaire de périphériques, choisissez Désinstaller l'appareil, puis débranchez-le et rebranchez-le (voir « À la main dans le Gestionnaire de périphériques » dans le guide d'installation)."
    ${ElseIf} $LANGUAGE = 1034
      StrCpy ${OUT} "No se pudo devolver el adaptador a Windows por completo: puede que esté desconectado, o que Windows haya vuelto a elegir WinUSB. Conéctelo, haga clic derecho en él en el Administrador de dispositivos, elija Desinstalar el dispositivo y después desconéctelo y vuelva a conectarlo (consulte «A mano en el Administrador de dispositivos» en la guía de configuración)."
    ${ElseIf} $LANGUAGE = 1042
      StrCpy ${OUT} "어댑터를 Windows에 완전히 돌려주지 못했습니다. 어댑터가 분리되었거나 Windows가 다시 WinUSB를 선택했을 수 있습니다. 어댑터를 꽂은 상태에서 장치 관리자를 열고 어댑터를 마우스 오른쪽 버튼으로 클릭한 다음 [디바이스 제거]를 선택하고, 어댑터를 뽑았다가 다시 꽂으세요(설정 가이드의 '장치 관리자에서 직접 하기' 참조)."
    ${ElseIf} $LANGUAGE = 2052
      StrCpy ${OUT} "未能将适配器完全还给 Windows：适配器可能已拔出，或 Windows 再次选择了 WinUSB。请插入适配器，在设备管理器中右键单击它，选择“卸载设备”，然后拔出并重新插入（参见设置指南中的“在设备管理器中手动操作”）。"
    ${ElseIf} $LANGUAGE = 1028
      StrCpy ${OUT} "無法將介面卡完全交還給 Windows：介面卡可能已拔除，或 Windows 再次選擇了 WinUSB。請插入介面卡，在裝置管理員中以滑鼠右鍵按一下它，選擇「解除安裝裝置」，然後拔除並重新插入（請參閱設定指南中的「在裝置管理員中手動操作」）。"
    ${EndIf}
  !endif
!macroend

; Closes the running v2 core of this user: asks it to exit (the installed
; copy speaks to whichever core holds the user's pipe), then ends a copy
; that still runs from $INSTDIR. $A2dpwbCoreWasRunning = 1 if one ran.
!macro A2DPWB_CloseCore
  StrCpy $A2dpwbCoreWasRunning 0
  ${If} ${FileExists} "$INSTDIR\${A2DPWB_CORE}"
    ; Exit code 0: it ran and has exited; 2: none ran; 1: it still runs
    nsExec::Exec '"$INSTDIR\${A2DPWB_CORE}" --quit'
    Pop $0
    ${If} $0 = 0
    ${OrIf} $0 = 1
      StrCpy $A2dpwbCoreWasRunning 1
    ${EndIf}
    ; Tauri's check (utils.nsh): the processes using that file, ended
    ; after asking (without asking in passive and silent mode)
    !insertmacro CheckIfAppIsRunning "$INSTDIR\${A2DPWB_CORE}" "${PRODUCTNAME}"
  ${EndIf}
!macroend

; Waits until v1 has exited, asking the user to exit it. Silent mode: nobody
; to ask, $0 = 1 when it still runs.
!macro A2DPWB_WaitForV1Exit
  StrCpy $0 0
  ${Do}
    System::Call 'kernel32::OpenMutexW(i 0x00100000, i 0, w "${A2DPWB_V1_MUTEX}") i .r1'
    ${If} $1 = 0
      ${Break}
    ${EndIf}
    System::Call 'kernel32::CloseHandle(i r1)'
    ${If} ${Silent}
      StrCpy $0 1
      ${Break}
    ${EndIf}
    !insertmacro A2DPWB_Text $2 v1_running
    MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "$2" IDRETRY +2
    Abort "$2"
  ${Loop}
!macroend

; Uninstalls v1 registered under ROOT (HKCU: installed for the user, HKLM:
; for all users, which asks for administrator rights)
!macro A2DPWB_UninstallV1 ROOT
  ReadRegStr $1 ${ROOT} "${A2DPWB_V1_UNINST_KEY}" "UninstallString"
  ${If} $1 != ""
    ; "C:\...\unins000.exe" -> the path without the quotes
    StrCpy $2 $1 1
    ${If} $2 == '"'
      StrCpy $1 $1 "" 1
      ${StrLoc} $2 $1 '"' ">"
      ${If} $2 != ""
        StrCpy $1 $1 $2
      ${EndIf}
    ${EndIf}
    DetailPrint "Uninstalling A2DP Windows Bridge v1: $1"
    ${If} ${FileExists} "$1"
      ExecShellWait "open" "$1" "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART"
    ${EndIf}
    ; Inno Setup's uninstaller finishes in a copy of itself: wait for its
    ; registration to go (30 s at most)
    StrCpy $3 0
    ${Do}
      ReadRegStr $2 ${ROOT} "${A2DPWB_V1_UNINST_KEY}" "UninstallString"
      ${If} $2 == ""
        ${Break}
      ${EndIf}
      IntOp $3 $3 + 1
      ${If} $3 > 60
        !insertmacro A2DPWB_Text $2 v1_left
        DetailPrint "$2"
        MessageBox MB_OK|MB_ICONINFORMATION "$2" /SD IDOK
        ${Break}
      ${EndIf}
      Sleep 500
    ${Loop}
  ${EndIf}
!macroend

; $0 = exit code of EXE (in $5) with the arguments in $6, started with VERB
; ("runas": UAC) and waited for; -1 when it did not start, with the error in
; $1 (1223: UAC cancelled). ExecShellWait gives no exit code.
!macro A2DPWB_ShellExecWait VERB
  !if ${NSIS_PTR_SIZE} > 4
    !error "A2DPWB_ShellExecWait: SHELLEXECUTEINFOW below is laid out for 32-bit NSIS"
  !endif
  StrCpy $0 -1
  ; SHELLEXECUTEINFOW (60 bytes): fMask SEE_MASK_NOCLOSEPROCESS |
  ; SEE_MASK_NOASYNC, nShow SW_HIDE (a console program)
  System::Call '*(i 60, i 0x140, p $HWNDPARENT, w "${VERB}", w r5, w r6, p 0, i 0, p 0, p 0, p 0, p 0, i 0, p 0, p 0) p .r2'
  System::Call 'shell32::ShellExecuteExW(p r2) i .r3 ?e'
  Pop $1
  ${If} $3 <> 0
    StrCpy $1 0
    System::Call '*$2(i, i, p, p, p, p, p, i, p, p, p, p, i, p, p .r4)'
    System::Call 'kernel32::WaitForSingleObject(p r4, i -1)'
    System::Call 'kernel32::GetExitCodeProcess(p r4, *i .r0)'
    System::Call 'kernel32::CloseHandle(p r4)'
  ${EndIf}
  System::Free $2
!macroend

; Offers to give the adapters A2DPWB put on WinUSB back to Windows. The core
; keeps an assignment record per adapter in <settings>\driver\ (the result
; of `a2dpwb-driver assign`, app/src/core/src/driver.rs) and deletes it on
; "Restore driver"; `a2dpwb-driver restore --records` restores every
; recorded adapter with one UAC prompt and deletes the records of those
; given back. Silent mode keeps WinUSB: nobody to ask, and a silent
; uninstall must not raise a UAC prompt. The core must be closed already.
!macro A2DPWB_RestoreDrivers
  ReadEnvStr $6 A2DPWB_CONFIG_DIR
  ${If} $6 == ""
    StrCpy $6 "$APPDATA\A2DPWB"
  ${EndIf}
  StrCpy $6 "$6\driver"
  ClearErrors
  FindFirst $0 $1 "$6\*.json"
  ${IfNot} ${Errors}
    FindClose $0
  ${EndIf}
  ${If} $1 != ""
  ${AndIf} ${FileExists} "$INSTDIR\a2dpwb-driver.exe"
    ; v1 (running, or installed by Inno Setup) uses the adapter through
    ; WinUSB too: then No is the default
    StrCpy $3 0
    System::Call 'kernel32::OpenMutexW(i 0x00100000, i 0, w "${A2DPWB_V1_MUTEX}") i .r1'
    ${If} $1 <> 0
      System::Call 'kernel32::CloseHandle(i r1)'
      StrCpy $3 1
    ${EndIf}
    ReadRegStr $1 HKCU "${A2DPWB_V1_UNINST_KEY}" "UninstallString"
    ${If} $1 != ""
      StrCpy $3 1
    ${EndIf}
    ReadRegStr $1 HKLM "${A2DPWB_V1_UNINST_KEY}" "UninstallString"
    ${If} $1 != ""
      StrCpy $3 1
    ${EndIf}
    !insertmacro A2DPWB_Text $2 restore_ask
    StrCpy $4 0
    ${If} $3 = 1
      !insertmacro A2DPWB_Text $1 restore_v1_note
      StrCpy $2 "$2$1"
      ${If} ${Cmd} `MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "$2" /SD IDNO IDYES`
        StrCpy $4 1
      ${EndIf}
    ${ElseIf} ${Cmd} `MessageBox MB_YESNO|MB_ICONQUESTION "$2" /SD IDNO IDYES`
      StrCpy $4 1
    ${EndIf}
    ${If} $4 = 1
      DetailPrint "Restoring the drivers of the adapters recorded in $6"
      StrCpy $5 "$INSTDIR\a2dpwb-driver.exe"
      StrCpy $6 'restore --records "$6"'
      !insertmacro A2DPWB_ShellExecWait "runas"
      ; 0: all given back (or nothing to do)
      ${If} $0 = 0
        DetailPrint "The adapters are back on their own drivers"
      ${ElseIf} $0 = -1
        DetailPrint "a2dpwb-driver.exe did not start (error $1): the adapters stay on WinUSB"
      ${Else}
        DetailPrint "a2dpwb-driver.exe restore --records: exit code $0"
        !insertmacro A2DPWB_Text $2 restore_failed
        MessageBox MB_OK|MB_ICONEXCLAMATION "$2" /SD IDOK
      ${EndIf}
    ${Else}
      DetailPrint "The adapters stay on WinUSB"
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREINSTALL
  StrCpy $A2dpwbKeepV1 0
  ${GetOptions} $CMDLINE "/KEEPV1" $0
  ${IfNot} ${Errors}
    StrCpy $A2dpwbKeepV1 1
  ${EndIf}

  !insertmacro A2DPWB_CloseCore

  StrCpy $A2dpwbRunWasSet 0
  ${If} $A2dpwbKeepV1 = 0
    ; Read before v1's uninstaller deletes its own Run value
    ReadRegStr $0 HKCU "${A2DPWB_RUN_KEY}" "${A2DPWB_RUN_VALUE}"
    ${If} $0 != ""
      StrCpy $A2dpwbRunWasSet 1
    ${EndIf}
    !insertmacro A2DPWB_WaitForV1Exit
    ${If} $0 = 0
      !insertmacro A2DPWB_UninstallV1 HKCU
      !insertmacro A2DPWB_UninstallV1 HKLM
    ${Else}
      DetailPrint "A2DP Windows Bridge v1 is running: it stays installed"
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${If} $A2dpwbRunWasSet = 1
    WriteRegStr HKCU "${A2DPWB_RUN_KEY}" "${A2DPWB_RUN_VALUE}" '"$INSTDIR\${A2DPWB_CORE}" --background'
  ${EndIf}
  ; As the user (not elevated), like Tauri's start of the app
  ${If} $A2dpwbCoreWasRunning = 1
    nsis_tauri_utils::RunAsUser "$INSTDIR\${A2DPWB_CORE}" "--background"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro A2DPWB_CloseCore
  ; Not on updates: the next version keeps using the adapter
  ${If} $UpdateMode <> 1
    !insertmacro A2DPWB_RestoreDrivers
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Kept on updates (the next version's installer runs this uninstaller)
  ${If} $UpdateMode <> 1
    ReadRegStr $0 HKCU "${A2DPWB_RUN_KEY}" "${A2DPWB_RUN_VALUE}"
    ${UnStrCase} $0 $0 "L"
    ${UnStrCase} $1 "$INSTDIR\${A2DPWB_CORE}" "L"
    ${UnStrLoc} $2 $0 $1 ">"
    ${If} $2 != ""
      DeleteRegValue HKCU "${A2DPWB_RUN_KEY}" "${A2DPWB_RUN_VALUE}"
    ${EndIf}
  ${EndIf}
!macroend
