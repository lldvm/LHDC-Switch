using System;
using System.Drawing;
using System.IO;
using System.Threading;
using System.Windows.Forms;

internal sealed partial class MainForm
{
    NotifyIcon tray;
    ToolStripMenuItem trayDisconnect, trayExit;
    EventWaitHandle showSignal, exitSignal;
    bool exitRequested;
    void InitializeTray(EventWaitHandle show, EventWaitHandle exit)
    {
        showSignal=show; exitSignal=exit;
        var menu = new ContextMenuStrip();
        menu.Items.Add("显示控制台",null,(s,e)=>ShowFromTray());
        trayDisconnect = new ToolStripMenuItem("断开 LHDC 并恢复蓝牙",null,(s,e)=>StopClicked(s,e));
        menu.Items.Add(trayDisconnect);
        var closeToTray = new ToolStripMenuItem("关闭窗口时最小化到托盘") { Checked=config.CloseToTray, CheckOnClick=true };
        closeToTray.CheckedChanged += (s,e)=> { config.CloseToTray=closeToTray.Checked; Program.Save(Path.Combine(Program.Root,"settings.json"),config); };
        menu.Items.Add(closeToTray); menu.Items.Add(new ToolStripSeparator());
        trayExit = new ToolStripMenuItem("退出并恢复蓝牙",null,(s,e)=>RequestExit()); menu.Items.Add(trayExit);
        tray = new NotifyIcon { Icon=SystemIcons.Application, Text="LHDC Switch", ContextMenuStrip=menu, Visible=true };
        tray.DoubleClick += (s,e)=>ShowFromTray();
        Resize += (s,e)=> { if(WindowState==FormWindowState.Minimized && config.CloseToTray && !closing) HideToTray(); };
        FormClosed += (s,e)=> { RecordTrayState(false); tray.Visible=false; tray.Dispose(); menu.Dispose(); };
        Shown += (s,e)=>RecordTrayState(true);
    }
    void RecordTrayState(bool alive)
    {
        Program.Save(Path.Combine(Program.Data,"ui-lifecycle.json"),new { timestamp=DateTime.UtcNow.ToString("o"),
            alive=alive, visible=alive && Visible, tray_visible=alive && tray!=null && tray.Visible,
            exit_requested=exitRequested, session_running=Program.SessionRunning() });
    }
    void HideToTray()
    {
        if(tray==null) return;
        tray.Visible=true; Hide(); ShowInTaskbar=false; RecordTrayState(true);
    }
    void ShowFromTray()
    {
        ShowInTaskbar=true; Show(); WindowState=FormWindowState.Normal; Activate(); BringToFront(); RecordTrayState(true);
    }
    void RequestExit()
    {
        if(closing) return;
        exitRequested=true; ShowFromTray(); Close();
    }
    void PollTraySignals()
    {
        if(showSignal!=null && showSignal.WaitOne(0)) ShowFromTray();
        if(exitSignal!=null && exitSignal.WaitOne(0)) RequestExit();
        if(tray==null || IsDisposed) return;
        bool running=WorkerActive || Program.SessionRunning();
        trayDisconnect.Enabled=running && !closing; trayExit.Enabled=!closing;
        string text="LHDC Switch · "+(closing?"正在恢复蓝牙":running?"LHDC 会话运行中":"Windows 蓝牙");
        if(tray.Text!=text) tray.Text=text;
    }
}
