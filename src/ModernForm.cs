using System;
using System.IO;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Threading.Tasks;
using System.Windows.Forms;
using System.Runtime.InteropServices;

internal sealed partial class MainForm : Form
{
    internal static readonly Color Bg = Color.FromArgb(10, 17, 29), Card = Color.FromArgb(18, 30, 47), Muted = Color.FromArgb(143, 163, 184), Accent = Color.FromArgb(68, 219, 186);
    readonly Label badge = new Label(), codec = new Label(), codecSub = new Label(), encoder = new Label(), tx = new Label(), format = new Label(), queue = new Label(), signal = new Label(), health = new Label(), actualVolume = new Label(), volumeHint = new Label(), detail = new Label(), dropCounts = new Label();
    readonly ComboBox quality = new DarkComboBox(), rate = new DarkComboBox();
    readonly TextBox app = new TextBox(); readonly CheckBox exclusive = new CheckBox();
    readonly Button start = new Button(), stop = new Button(), muteButton = new Button();
    readonly VolumeSlider volume = new VolumeSlider(); readonly RateChart chart = new RateChart();
    readonly VolumePresentation volumeState = new VolumePresentation();
    Label queueNote, protocolCaption;
    readonly System.Windows.Forms.Timer timer = new System.Windows.Forms.Timer();
    Config config; Process worker; bool busy, closing, updatingVolume; int lastVolume = -1, previousVolume = 32, refreshTicks;
    string lastStatus = "", lastLink = ""; Dictionary<string, object> oldLink; DateTime lastHealth = DateTime.MinValue;
    bool WorkerActive { get { try { return worker != null && !worker.HasExited; } catch { return false; } } }

    public MainForm(System.Threading.EventWaitHandle showSignal, System.Threading.EventWaitHandle exitSignal)
    {
        SuspendLayout(); AutoScaleDimensions = new SizeF(96F, 96F); AutoScaleMode = AutoScaleMode.Dpi;
        config = Program.LoadConfig(); Text = "LHDC Switch · 耳机控制台" + (Program.VariantLabel.Length>0 ? " · "+Program.VariantLabel : "");
        Font = new Font("Microsoft YaHei UI", 10F); BackColor = Bg; ForeColor = Color.White;
        ClientSize = new Size(1010, 660); MinimumSize = new Size(1026, 699); MaximizeBox = false;
        StartPosition = FormStartPosition.CenterScreen; DoubleBuffered = true;
        var side = new Panel { BackColor = Color.FromArgb(14, 24, 39), Dock = DockStyle.Left, Width = 160 }; Controls.Add(side);
        LabelAt(side, "◉", 24, 26, 100, 43, 30, Accent);
        LabelAt(side, "LHDC", 24, 85, 120, 35, 23, Color.White, true);
        LabelAt(side, "S W I T C H", 25, 124, 130, 24, 9, Muted);
        if(Program.VariantLabel.Length>0) LabelAt(side,Program.VariantLabel,13,160,140,26,9,Accent);
        var nav = ButtonAt(side, "◈   耳机控制台", 13, 196, 134, 42, Card, Accent);
        var log = ButtonAt(side, "诊断与日志  ↗", 13, 250, 134, 40, side.BackColor, Muted);
        log.Click += (s, e) => OpenDiagnostics();
        var hi = ButtonAt(side, "耳机功能面板", 13, 299, 134, 40, side.BackColor, Muted);
        hi.Click += (s, e) => ShowEarbudPanel();
        var install = ButtonAt(side, "安装虚拟声卡", 13, 348, 134, 40, side.BackColor, Muted);
        install.Click += (s, e) => { try { Program.OpenCableInstaller(); } catch (Exception ex) { SetDetail(ex.Message); } };
        var radio = ButtonAt(side, "链路设置与信号", 13, 397, 134, 40, side.BackColor, Muted);
        radio.Click += (s,e) => ShowRadio();
        LabelAt(side, "本机控制器", 23, 536, 125, 22, 9, Muted);
        LabelAt(side, "Intel AX201", 23, 560, 125, 25, 11, Color.White, true);
        LabelAt(side, "关闭驻留 · 退出恢复", 23, 593, 130, 35, 8, Muted);
        LabelAt(this, "耳机控制台", 185, 23, 420, 35, 23, Color.White, true);
        LabelAt(this, config.Headphone, 187, 62, 500, 22, 10, Muted);
        badge.SetBounds(791, 32, 190, 30); badge.TextAlign = ContentAlignment.MiddleRight; badge.ForeColor = Muted; badge.Text = "●  Windows 蓝牙"; Controls.Add(badge);

        var hero = CardAt(184, 101, 798, 142);
        protocolCaption=LabelAt(hero, "当前音频协议", 22, 16, 490, 22, 9, Muted);
        Value(hero, codec, "等待连接", 20, 42, 380, 47, 27, Color.White);
        Value(hero, codecSub, "以耳机实际协商结果为准", 23, 100, 480, 24, 9, Muted);
        StyleButton(start, "连接 LHDC", 566, 29, 210, 43, Accent, Bg); hero.Controls.Add(start); start.Click += StartClicked;
        StyleButton(stop, "断开并恢复蓝牙", 566, 83, 210, 35, Color.FromArgb(33, 48, 66), Color.White); hero.Controls.Add(stop); stop.Click += StopClicked;

        Metric(184, "实时编码码率", encoder, "—", "编码器当前目标值");
        Metric(388, "实时发送速率", tx, "—", "含 RTP 等协议开销");
        Metric(592, "音频格式", format, "—", "实际采样率 / 位深");
        Metric(796, "音频积压", queue, "—", "待发音频时长 · 包数");

        var graph = CardAt(184, 367, 490, 166);
        LabelAt(graph, "链路速率", 20, 13, 160, 23, 11, Color.White, true);
        signal.SetBounds(215, 15, 250, 23); signal.TextAlign = ContentAlignment.MiddleRight; signal.ForeColor = Muted; signal.Text = "等待实时采样"; graph.Controls.Add(signal);
        chart.SetBounds(16, 46, 458, 84); graph.Controls.Add(chart);
        Value(graph, dropCounts, "本机丢弃 — / 采集不连续 — / 发送停顿 —", 20, 138, 450, 22, 8, Muted);
        var audio = CardAt(690, 367, 292, 166);
        LabelAt(audio, "耳机音量", 20, 13, 130, 24, 11, Color.White, true);
        Value(audio, actualVolume, "—", 180, 10, 90, 31, 19, Accent); actualVolume.TextAlign = ContentAlignment.MiddleRight;
        volume.SetBounds(20, 54, 250, 36); volume.Enabled = false; audio.Controls.Add(volume); volume.Requested += SetVolume;
        volume.Previewed += raw => { actualVolume.Text=(raw*100/127)+"%"; volumeHint.Text="调节中 · 等待耳机确认"; };
        volumeHint.SetBounds(20, 101, 252, 23); volumeHint.ForeColor = Muted; volumeHint.Text = "连接后直接控制耳机 AVRCP 音量"; audio.Controls.Add(volumeHint);
        StyleButton(muteButton, "静音", 20, 130, 66, 25, Color.FromArgb(33, 48, 66), Muted); audio.Controls.Add(muteButton);
        muteButton.Enabled = false; muteButton.Click += (s, e) => SetVolume(lastVolume == 0 ? previousVolume : 0);

        health.SetBounds(187, 539, 790, 25); health.ForeColor = Muted; health.Text = "实时诊断就绪；连接后记录丢包、采集丢帧与发送队列。"; Controls.Add(health);
        var options = CardAt(184, 574, 798, 67);
        LabelAt(options, "音质", 14, 9, 50, 20, 8, Muted); SetupCombo(quality, options, 13, 30, 176);
        quality.DropDownWidth = 256;
        quality.MaxDropDownItems=14;
        quality.Items.AddRange(new object[] { "自动 · 最高 900", "自动 · 最高 1000", "自动 · 最高 800", "自动 · 最高 750", "自动 · 最高 700", "自动 · 最高 500", "自动 · 最高 400", "固定 · 400 kbps", "固定 · 500 kbps", "固定 · 700 kbps", "固定 · 750 kbps", "固定 · 800 kbps", "固定 · 900 kbps", "固定 · 1000 kbps" });
        quality.SelectedIndex = Math.Max(0, Array.IndexOf(Program.QualityKeys, config.Quality));
        LabelAt(options, "采样率", 202, 9, 85, 20, 8, Muted); SetupCombo(rate, options, 201, 30, 104);
        rate.Items.AddRange(new object[] { "44.1 kHz", "48 kHz", "96 kHz", "192 kHz" }); rate.SelectedIndex = Array.IndexOf(new[] { 44100, 48000, 96000, 192000 }, config.SampleRate);
        LabelAt(options, "仅指定应用（留空发送系统声音）", 319, 9, 260, 20, 8, Muted);
        app.SetBounds(319, 31, 240, 26); app.Text = config.Application; app.BorderStyle = BorderStyle.FixedSingle; app.BackColor = Color.FromArgb(29, 44, 62); app.ForeColor = Color.White; options.Controls.Add(app);
        exclusive.SetBounds(580, 28, 200, 30); exclusive.Text = "只在耳机播放"; exclusive.Checked = config.ExclusiveOutput; exclusive.ForeColor = Muted; options.Controls.Add(exclusive);
        detail.SetBounds(187, 648, 790, 1); detail.Visible = false; Controls.Add(detail);
        timer.Interval = 250; timer.Tick += Tick; timer.Start();
        Shown += async (s, e) => {
            Program.Save(Path.Combine(Program.Data, "ui-rendering.json"), new { dpi = DeviceDpi, client_width = ClientSize.Width, client_height = ClientSize.Height,
                per_monitor_v2 = DpiState.AreDpiAwarenessContextsEqual(DpiState.GetWindowDpiAwarenessContext(Handle), new IntPtr(-4)),
                font = Font.Name, font_points = Font.SizeInPoints, framework = AppDomain.CurrentDomain.SetupInformation.TargetFrameworkName });
            await RefreshDriver();
        }; FormClosing += OnClosing;
        ResumeLayout(true);
        InitializeTray(showSignal,exitSignal);
    }
    static Label LabelAt(Control parent, string text, int x, int y, int w, int h, float size, Color color, bool bold = false)
    {
        var l = new Label { Text = text, Location = new Point(x, y), Size = new Size(w, h), ForeColor = color, BackColor = Color.Transparent,
            Font = new Font("Microsoft YaHei UI", Math.Max(9.5F, size), bold ? FontStyle.Bold : FontStyle.Regular) }; parent.Controls.Add(l); return l;
    }
    static void Value(Control parent, Label l, string text, int x, int y, int w, int h, float size, Color color)
    { l.Text = text; l.SetBounds(x, y, w, h); l.Font = new Font("Microsoft YaHei UI", Math.Max(9.5F, size), FontStyle.Bold); l.ForeColor = color; l.BackColor = Color.Transparent; parent.Controls.Add(l); }
    Panel CardAt(int x, int y, int w, int h) { var p = new RoundedPanel { BackColor = Card }; p.SetBounds(x, y, w, h); Controls.Add(p); return p; }
    void Metric(int x, string title, Label value, string text, string note)
    { var p = CardAt(x, 259, 186, 92); LabelAt(p, title, 15, 9, 170, 20, 9, Muted); Value(p, value, text, 13, 30, 170, 31, 20, Color.White); var noteLabel=LabelAt(p, note, 15, 66, 170, 20, 8, Muted); if(value==queue) queueNote=noteLabel; }
    static void StyleButton(Button b, string text, int x, int y, int w, int h, Color bg, Color fg)
    { b.Text = text; b.SetBounds(x, y, w, h); b.FlatStyle = FlatStyle.Flat; b.FlatAppearance.BorderSize = 0; b.BackColor = bg; b.ForeColor = fg; b.Cursor = Cursors.Hand; b.Font = new Font("Microsoft YaHei UI", 10, FontStyle.Bold); }
    static Button ButtonAt(Control p, string text, int x, int y, int w, int h, Color bg, Color fg)
    { var b = new Button(); StyleButton(b, text, x, y, w, h, bg, fg); p.Controls.Add(b); return b; }
    static void SetupCombo(ComboBox c, Control parent, int x, int y, int w)
    { c.SetBounds(x, y, w, 26); c.DropDownStyle = ComboBoxStyle.DropDownList; c.FlatStyle = FlatStyle.Flat; c.BackColor = Color.FromArgb(29, 44, 62); c.ForeColor = Color.White; parent.Controls.Add(c); }
    void SetDetail(string message) { codecSub.Text = message; Program.Log(message); }
    void Busy(bool b)
    {
        busy = b; bool running = WorkerActive || Program.SessionRunning();
        start.Enabled = !b && !running && !File.Exists(Program.Journal);
        quality.Enabled = rate.Enabled = app.Enabled = exclusive.Enabled = !b && !running;
        stop.Enabled = !b;
    }
    void SaveSettings()
    {
        config.Quality = Program.QualityKeys[quality.SelectedIndex]; config.SampleRate = new[] { 44100, 48000, 96000, 192000 }[rate.SelectedIndex];
        config.Application = app.Text.Trim(); config.ExclusiveOutput = exclusive.Checked; config.TestTone = false;
        Program.Validate(config); Program.Save(Path.Combine(Program.Root, "settings.json"), config);
    }
    async void StartClicked(object sender, EventArgs e)
    {
        if (busy || WorkerActive) return;
        try
        {
            SaveSettings(); Busy(true);
            await Task.Run(() => { Program.CheckEngine(); Program.RequireHealthyBluetooth(Program.DriverState(config)); });
            if (config.ExclusiveOutput && String.IsNullOrEmpty(await Task.Run(() => Program.FindCable()))) throw new InvalidOperationException("未找到 VB-CABLE，请先点左侧“安装虚拟声卡”。");
            if (File.Exists(Program.StopFile)) File.Delete(Program.StopFile);
            Program.Save(Program.LiveFile, new { active = false, timestamp = DateTime.UtcNow.ToString("o") });
            chart.Clear(); oldLink = null; lastLink = ""; lastVolume = -1;
            Program.Status("waiting", "请在 Windows 权限窗口中允许接管。");
            using (var self = Process.GetCurrentProcess()) worker = Program.Elevate(Program.Args("--session", self.Id.ToString(), self.StartTime.ToUniversalTime().Ticks.ToString()));
        }
        catch (Exception ex) { SetDetail(ex.Message); }
        finally { Busy(false); }
    }
    async void StopClicked(object sender, EventArgs e)
    {
        if (busy) return;
        if (Program.SessionRunning()) { Program.RequestStop("user stop"); SetDetail("正在停止音频并恢复原来的蓝牙及播放设备……"); return; }
        try { Busy(true); Program.CheckEngine(false); worker = Program.Elevate("--restore-worker"); await Task.Run(() => worker.WaitForExit()); await RefreshDriver(); }
        catch (Exception ex) { SetDetail(ex.Message); } finally { Busy(false); }
    }
    void SetVolume(int raw)
    {
        if (updatingVolume || !volume.Enabled) return;
        raw = Math.Max(0, Math.Min(127, raw)); if (lastVolume > 0) previousVolume = lastVolume;
        volumeState.Request(raw,DateTime.UtcNow); volume.Value=raw; actualVolume.Text=(raw*100/127)+"%";
        timer.Interval=100;
        Program.Save(Program.VolumeFile, new { sequence = Guid.NewGuid().ToString("N"), volume = raw });
        volumeHint.Text = "已发送请求，等待耳机回报 " + (raw * 100 / 127) + "%";
    }
    async void OpenDiagnostics()
    {
        if (busy) return;
        try { Busy(true); await Task.Run(() => Program.Diagnose()); Process.Start(new ProcessStartInfo("explorer.exe", Program.Quote(Program.Data)) { UseShellExecute = true }); }
        catch (Exception ex) { SetDetail(ex.Message); } finally { Busy(false); }
    }
    void ShowRadio()
    {
        using (var dialog = new Form())
        using (var refresh = new System.Windows.Forms.Timer { Interval = 1000 })
        {
            dialog.SuspendLayout(); dialog.AutoScaleDimensions=new SizeF(96F,96F); dialog.AutoScaleMode=AutoScaleMode.Dpi;
            dialog.Text="蓝牙链路设置与信号"; dialog.ClientSize=new Size(650,446); dialog.BackColor=Card; dialog.ForeColor=Color.White;
            dialog.Font=Font; dialog.StartPosition=FormStartPosition.CenterParent; dialog.MaximizeBox=false; dialog.MinimizeBox=false;
            LabelAt(dialog,"蓝牙链路",22,18,600,34,20,Color.White,true);
            LabelAt(dialog,"跳频避让",24,67,120,26,10,Muted);
            var policy = new DarkComboBox(); SetupCombo(policy,dialog,150,65,470);
            policy.Items.AddRange(new object[] { "自动：避让本机正在使用的 2.4 GHz Wi-Fi", "由蓝牙控制器自行判断干扰信道" });
            policy.SelectedIndex = config.AfhPolicy == "off" ? 1 : 0;
            var active = new CheckBox { Text="播放期间避免 Sniff 省电模式", Checked=config.ActiveRadio, ForeColor=Color.White };
            active.SetBounds(24,105,595,30); dialog.Controls.Add(active);
            LabelAt(dialog,"自动跳频在 5 / 6 GHz Wi-Fi 下不屏蔽 Wi-Fi 信道；无需手动强推。",24,144,600,29,10,Muted);
            var readings=LabelAt(dialog,"",24,187,600,144,10,Color.White);
            LabelAt(dialog,"链路质量是控制器的 0–255 读数；功率由硬件调节。未读取时显示 —。",24,338,600,28,9.5F,Muted);
            var save=ButtonAt(dialog,"保存，下次连接生效",24,386,596,40,Accent,Bg);
            save.Click += (s,e) => { config.AfhPolicy=policy.SelectedIndex==1 ? "off" : "auto"; config.ActiveRadio=active.Checked;
                Program.Validate(config); Program.Save(Path.Combine(Program.Root,"settings.json"),config); dialog.Close(); };
            Action update=()=>{
                try {
                    var data=Program.ReadObject(Program.LiveFile); DateTime at;
                    bool fresh=Program.Bool(data,"active") && DateTime.TryParse(Program.Get(data,"link_timestamp"),out at) && (DateTime.UtcNow-at.ToUniversalTime()).TotalSeconds<3;
                    if(!fresh || Program.Get(Program.Child(data,"stream"),"state")!="streaming") {readings.Text="当前没有 LHDC 链路。\r\n连接后显示实际模式、链路质量、功率和跳频信道。\r\n设置项表示下次连接的策略，不代表已生效。";return;}
                    var link=Program.Child(data,"link"); string mode=Program.Get(link,"link_mode");
                    string modeName=mode=="0"?"Active":mode=="1"?"Hold":mode=="2"?"Sniff":mode=="3"?"Park":"—";
                    Func<string,string> value=k=>Program.Get(link,k).Length==0?"—":Program.Get(link,k);
                    readings.Text="实际模式："+modeName+"    链路策略："+value("link_policy")+"（1 允许角色切换；5 另允许 Sniff）"+
                        "\r\n链路质量："+value("link_quality")+" / 255    RSSI："+value("rssi")+"（相对值，非 dBm）"+
                        "\r\n发射功率："+value("tx_power_dbm")+" dBm    硬件最大："+value("tx_power_max_dbm")+" dBm"+
                        "\r\n实际跳频信道："+value("afh_in_use")+" / 79    主机避让 Wi-Fi："+(Program.Get(link,"afh_avoided").Length==0?"无":Program.Get(link,"afh_avoided"))+
                        "\r\n各硬件读数轮询约 6 秒；发送统计持续更新。";
                } catch {readings.Text="正在等待链路数据……";}
            };
            refresh.Tick += (s,e)=>update(); dialog.ResumeLayout(true); update(); refresh.Start(); dialog.ShowDialog(this);
        }
    }
    async Task RefreshDriver()
    {
        try
        {
            var s = await Task.Run(() => { Program.CheckEngine(); return Program.DriverState(config); });
            if (WorkerActive || Program.SessionRunning()) return;
            bool normal = Program.Get(s, "kind") == "bluetooth";
            badge.Text = normal ? "●  Windows 蓝牙" : "●  需要恢复"; badge.ForeColor = normal ? Muted : Color.Orange;
            codec.Text = "等待连接";
            string last = Program.Get(Program.ReadStatus(), "message");
            codecSub.Text = File.Exists(Program.Journal) ? "检测到未完成的会话，请先恢复蓝牙。" : last.Contains("认证失败") || last.Contains("auth_failed") ? "上次配对认证失败，请将耳机置于配对模式后再连接。" : "连接后显示实际协议、码率和耳机音量。";
        }
        catch (Exception ex) { SetDetail(ex.Message); }
    }
    async void Tick(object sender, EventArgs e)
    {
        PollTraySignals();
        if(IsDisposed) return;
        volumeState.Expire(DateTime.UtcNow);
        timer.Interval=Visible?(volume.Dragging||volumeState.Pending>=0?100:250):1000;
        var status = Program.ReadStatus(); string stamp = Program.Get(status, "timestamp");
        if (stamp.Length > 0 && stamp != lastStatus)
        {
            lastStatus = stamp; string phase = Program.Get(status, "phase");
            if (phase != "streaming")
            {
                string message = Program.Get(status, "message");
                SetDetail(message.Contains("auth_failed") || message.Contains("认证失败") ? "配对认证失败：请将耳机置于配对模式后再连接。" : message.Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries).LastOrDefault() ?? message);
            }
            if (phase == "assigning" || phase == "connecting" || phase == "preparing" || phase == "waiting") { badge.Text = "●  正在连接"; badge.ForeColor = Accent; codec.Text = "连接中"; }
            if (phase == "restoring") { badge.Text = "●  正在恢复"; codec.Text = "恢复中"; }
            if (phase == "restored") { badge.Text = "●  Windows 蓝牙"; badge.ForeColor = Muted; }
        }
        bool live = false;
        try
        {
            if (File.Exists(Program.LiveFile))
            {
                var data = Program.ReadObject(Program.LiveFile); DateTime updated;
                live = Program.Bool(data, "active") && DateTime.TryParse(Program.Get(data, "timestamp"), out updated) && (DateTime.UtcNow - updated.ToUniversalTime()).TotalSeconds < 3;
                var stream = Program.Child(data, "stream"); var info = Program.Child(stream, "info"); var link = Program.Child(data, "link");
                DateTime linkAt; bool linkFresh = DateTime.TryParse(Program.Get(data, "link_timestamp"), out linkAt) && (DateTime.UtcNow - linkAt.ToUniversalTime()).TotalSeconds < 3;
                bool playing = live && linkFresh && Program.Get(stream, "state") == "streaming" && info != null;
                if (playing)
                {
                    string name = Program.Get(info, "codec"); bool isLhdc = name.StartsWith("LHDC", StringComparison.OrdinalIgnoreCase);
                    codec.Text = name; badge.Text = isLhdc ? "●  LHDC 已确认" : "●  协议不符"; badge.ForeColor = isLhdc ? Accent : Color.Orange;
                    bool hasLiveRate = LiveData.Number(link, "encoder_kbps") > 0;
                    encoder.Text = (hasLiveRate ? Program.Get(link, "encoder_kbps") : Program.Get(info, "bitrate_kbps")) + " kbps";
                    format.Text = (LiveData.Number(info, "sample_rate") / 1000).ToString("0.#") + " kHz";
                    bool capped = Program.Bool(info, "bitrate_capped"), abr = Program.Bool(info, "abr");
                    string ceiling = LiveData.Number(data, "peer_max_kbps") > 0 ? "耳机上限 " + Program.Get(data, "peer_max_kbps") + " kbps  ·  " : "";
                    protocolCaption.Text="当前音频协议 · "+Program.Get(link,"encoder_backend");
                    codecSub.Text = ceiling + "24-bit  ·  " + (abr ? (hasLiveRate ? "自动上限 " + Program.Get(data, "adaptive_max_kbps") + " kbps" : "自动码率（编码初始值）") : capped ? "码率受耳机限制" : "固定码率");
                    string sample = Program.Get(link, "time_ms");
                    if (sample.Length > 0 && sample != lastLink)
                    {
                        lastLink = sample; double kbps = LiveData.Number(link, "bytes_per_second") * .008; tx.Text = kbps.ToString("0") + " kbps"; chart.Push(kbps);
                        queue.Text = (LiveData.Number(link,"queue_audio_us")/1000).ToString("0.0")+" ms";
                        queueNote.Text=Program.Get(link,"queue_depth")+" 包 · 等待 "+(LiveData.Number(link,"queue_wait_us")/1000).ToString("0.0")+" ms";
                        dropCounts.Text = "本机丢弃 "+Program.Get(link,"dropped")+" · 等待 "+(LiveData.Number(link,"queue_wait_us")/1000).ToString("0.0")+" ms · 定时峰值 "+(LiveData.Number(link,"pacing_late_max_us")/1000).ToString("0.0")+" ms";
                        signal.Text = "RSSI " + (Program.Get(link, "rssi").Length == 0 ? "—" : Program.Get(link, "rssi")) + "（相对值）";
                        if ((DateTime.UtcNow - lastHealth).TotalSeconds > 2 || health.Text.StartsWith("实时") || (oldLink != null &&
                            (LiveData.Number(link, "capture_dropped") > LiveData.Number(oldLink, "capture_dropped") || LiveData.Number(link, "dropped") > LiveData.Number(oldLink, "dropped"))))
                        { health.Text = LiveData.Health(link, oldLink); lastHealth = DateTime.UtcNow; }
                        oldLink = link;
                    }
                    string v = Program.Get(stream, "volume"); int actual;
                    if (Int32.TryParse(v, out actual))
                    {
                        volumeState.Observe(actual,DateTime.UtcNow); lastVolume=actual;
                        if(!volume.Dragging) {
                            int shown=volumeState.Display;
                            actualVolume.Text=(shown*100/127)+"%";
                            updatingVolume=true; volume.Value=shown; updatingVolume=false;
                            volumeHint.Text=volumeState.Pending>=0 ? "等待确认 · 耳机当前 "+(actual*100/127)+"%" : volumeState.TimedOut ? "未收到新确认 · 显示上次回报" : "耳机已回报 · AVRCP 绝对音量";
                        }
                    }
                }
                else if (live && !linkFresh && Program.Get(stream, "state") == "streaming") { badge.Text = "●  数据已超时"; badge.ForeColor = Color.Orange; }
                else if (!live && !WorkerActive && !Program.SessionRunning())
                { codec.Text = "等待连接"; encoder.Text = tx.Text = format.Text = queue.Text = "—"; signal.Text = "等待实时采样"; actualVolume.Text = "—"; }
                volume.Enabled = muteButton.Enabled = playing;
            }
        }
        catch { volume.Enabled = muteButton.Enabled = false; }
        Busy(busy);
        if (closing && !WorkerActive && !Program.SessionRunning())
        {
            if (File.Exists(Program.Journal)) { closing = false; exitRequested=false; ShowFromTray(); SetDetail("恢复未完成，请点击恢复蓝牙并查看诊断日志。"); }
            else { timer.Stop(); Close(); return; }
        }
        if (++refreshTicks % 200 == 0 && !busy && !WorkerActive && !Program.SessionRunning()) { Busy(true); await RefreshDriver(); Busy(false); }
    }
    void OnClosing(object sender, FormClosingEventArgs e)
    {
        if(e.CloseReason==CloseReason.UserClosing && !exitRequested && !closing && config.CloseToTray) { e.Cancel=true; HideToTray(); return; }
        if (!WorkerActive && !Program.SessionRunning()) return;
        closing = true; e.Cancel = true; Program.RequestStop("window closing"); SetDetail("正在恢复，完成后自动关闭。");
    }
}

internal sealed class RoundedPanel : Panel
{
    public RoundedPanel() { DoubleBuffered = true; }
    protected override void OnPaint(PaintEventArgs e)
    {
        e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
        using (var path = new GraphicsPath())
        {
            int d = 20, w = Width - 1, h = Height - 1;
            path.AddArc(0, 0, d, d, 180, 90); path.AddArc(w - d, 0, d, d, 270, 90); path.AddArc(w - d, h - d, d, d, 0, 90); path.AddArc(0, h - d, d, d, 90, 90); path.CloseFigure();
            using (var pen = new Pen(Color.FromArgb(34, 49, 68))) e.Graphics.DrawPath(pen, path);
        }
        base.OnPaint(e);
    }
}
internal sealed class RateChart : Control
{
    readonly Queue<double> points = new Queue<double>();
    public RateChart() { DoubleBuffered = true; BackColor = MainForm.Card; }
    internal void Clear() { points.Clear(); Invalidate(); }
    internal void Push(double v) { points.Enqueue(v); while (points.Count > 120) points.Dequeue(); Invalidate(); }
    protected override void OnPaint(PaintEventArgs e)
    {
        e.Graphics.SmoothingMode = SmoothingMode.AntiAlias; e.Graphics.Clear(BackColor);
        using (var p = new Pen(Color.FromArgb(38, 54, 74))) for (int i = 1; i < 4; i++) e.Graphics.DrawLine(p, 0, i * Height / 4, Width, i * Height / 4);
        if (points.Count < 2) { using (var b = new SolidBrush(MainForm.Muted)) e.Graphics.DrawString("连接后绘制真实发送速率", Font, b, 10, 37); return; }
        var data = points.ToArray(); double max = Math.Max(500, data.Max() * 1.15);
        var ps = data.Select((v, i) => new PointF(i * (Width - 1f) / 119, (float)(Height - 8 - v / max * (Height - 15)))).ToArray();
        using (var pen = new Pen(MainForm.Accent, 2)) e.Graphics.DrawLines(pen, ps);
    }
}
internal sealed class VolumeSlider : Control
{
    int value, lastSent=-1; DateTime sentAt=DateTime.MinValue; internal bool Dragging; public event Action<int> Requested, Previewed;
    internal int Value { get { return value; } set { this.value = Math.Max(0, Math.Min(127, value)); Invalidate(); } }
    public VolumeSlider() { DoubleBuffered = true; BackColor = MainForm.Card; TabStop = true; Cursor = Cursors.Hand; AccessibleName = "耳机音量，0到100百分比"; }
    void Send(bool final) { if(Requested==null || (!final && (value==lastSent || (DateTime.UtcNow-sentAt).TotalMilliseconds<120))) return;
        lastSent=value; sentAt=DateTime.UtcNow; Requested(value); }
    void FromMouse(int x) { Value = (int)Math.Round(Math.Max(0, Math.Min(1, (x - 9d) / (Width - 18))) * 127); if(Previewed!=null) Previewed(value); Send(false); }
    protected override void OnMouseDown(MouseEventArgs e) { if (!Enabled || e.Button!=MouseButtons.Left) return; Focus(); Dragging = true; Capture = true; FromMouse(e.X); }
    protected override void OnMouseMove(MouseEventArgs e) { if (Dragging) FromMouse(e.X); }
    protected override void OnMouseUp(MouseEventArgs e) { if (!Dragging) return; Dragging = false; Capture = false; Send(true); }
    protected override void OnMouseCaptureChanged(EventArgs e) { base.OnMouseCaptureChanged(e); if(Dragging && !Capture) {Dragging=false;Send(true);} }
    protected override bool IsInputKey(Keys key) { return key == Keys.Left || key == Keys.Right || base.IsInputKey(key); }
    protected override void OnKeyDown(KeyEventArgs e) { if (e.KeyCode == Keys.Left || e.KeyCode == Keys.Right) { Value += e.KeyCode == Keys.Left ? -3 : 3; if (Requested != null) Requested(value); } }
    protected override void OnPaint(PaintEventArgs e)
    {
        e.Graphics.SmoothingMode = SmoothingMode.AntiAlias; int y = Height / 2, x = 9 + value * (Width - 18) / 127;
        using (var p = new Pen(Color.FromArgb(49, 66, 86), 5)) e.Graphics.DrawLine(p, 9, y, Width - 9, y);
        using (var p = new Pen(Enabled ? MainForm.Accent : MainForm.Muted, 5)) e.Graphics.DrawLine(p, 9, y, x, y);
        using (var b = new SolidBrush(Enabled ? Color.White : MainForm.Muted)) e.Graphics.FillEllipse(b, x - 6, y - 6, 12, 12);
    }
}

internal sealed class DarkComboBox : ComboBox
{
    public DarkComboBox() { DrawMode = DrawMode.OwnerDrawFixed; ItemHeight = 22; }
    protected override void OnDrawItem(DrawItemEventArgs e)
    {
        using (var bg = new SolidBrush((e.State & DrawItemState.Selected) != 0 ? Color.FromArgb(43, 67, 86) : Color.FromArgb(29, 44, 62))) e.Graphics.FillRectangle(bg, e.Bounds);
        if (e.Index >= 0) TextRenderer.DrawText(e.Graphics, GetItemText(Items[e.Index]), Font, e.Bounds, Enabled ? Color.White : MainForm.Muted, TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPadding);
    }
    protected override void WndProc(ref Message m)
    {
        base.WndProc(ref m);
        if (m.Msg == 0x000F && IsHandleCreated)
        {
            using (var g = CreateGraphics())
            {
                int width = (int)Math.Round(22 * g.DpiX / 96); var box = new Rectangle(ClientSize.Width - width, 0, width, ClientSize.Height);
                using (var bg = new SolidBrush(Color.FromArgb(29, 44, 62))) g.FillRectangle(bg, box);
                int cx = box.X + box.Width / 2, cy = box.Height / 2, r = Math.Max(3, width / 6);
                using (var brush = new SolidBrush(MainForm.Muted)) g.FillPolygon(brush, new[] { new Point(cx-r,cy-2), new Point(cx+r,cy-2), new Point(cx,cy+3) });
                using (var pen = new Pen(Color.FromArgb(57, 78, 98))) g.DrawRectangle(pen, 0, 0, ClientSize.Width - 1, ClientSize.Height - 1);
            }
        }
    }
}
internal static class DpiState
{
    [DllImport("user32.dll")] internal static extern IntPtr GetWindowDpiAwarenessContext(IntPtr hwnd);
    [DllImport("user32.dll")] [return: MarshalAs(UnmanagedType.Bool)] internal static extern bool AreDpiAwarenessContextsEqual(IntPtr a, IntPtr b);
}
