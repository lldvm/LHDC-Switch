using System;
using System.IO;
using System.Text;
using System.Text.RegularExpressions;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Linq;
using System.Security.Principal;
using System.Security.Cryptography;
using System.Threading;
using System.Threading.Tasks;
using System.Web.Script.Serialization;
using System.Windows.Forms;

// A reversible session controller around A2DPWB 2.0.1 with LHDC Switch ABR.
// No driver mutation occurs during application startup, status or diagnostics.
internal static class Program
{
    internal static readonly string Root = AppDomain.CurrentDomain.BaseDirectory;
    internal static readonly string Data = Path.Combine(Root, "data");
    internal static readonly string Engine = Path.Combine(Root, "engine", "A2DPWB-2.0.1");
    internal static readonly string Journal = Path.Combine(Data, "pending-restore.json");
    internal static readonly string StatusFile = Path.Combine(Data, "status.json");
    internal static readonly string StopFile = Path.Combine(Data, "stop.request");
    internal static readonly string LogFile = Path.Combine(Data, "session.log");
    internal static readonly string LiveFile = Path.Combine(Data, "live.json");
    internal static readonly string VolumeFile = Path.Combine(Data, "volume-request.json");
    internal static readonly string Exe = Application.ExecutablePath;
    internal static readonly object LogLock = new object();
    internal const string ShowSignalName = @"Local\LHDC-Switch-Show";
    internal const string ExitSignalName = @"Local\LHDC-Switch-Exit";
    internal const string StopSignalName = @"Local\LHDC-Switch-Stop";
    internal const string AllowedPrefix = @"USB\VID_8087&PID_0026\";
    internal static readonly string[] QualityKeys={"abr900","abr1000","abr800","abr750","abr700","abr500","abr","mq","sq","fixed700","fixed750","fixed800","mhq","hq"};
    internal static JavaScriptSerializer Json { get { return new JavaScriptSerializer { MaxJsonLength = 8 * 1024 * 1024 }; } }
    internal static string VariantLabel { get { string path=Path.Combine(Root,"variant.json"); return File.Exists(path)?Get(ReadObject(path),"label"):""; } }

    [STAThread]
    private static int Main(string[] args)
    {
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        Directory.CreateDirectory(Data);
        try
        {
            if (args.Contains("--self-test")) return Tests.Run();
            if (args.Contains("--ipc-test")) return IpcSmokeTest();
            if (args.Contains("--diagnose")) { Diagnose(); return 0; }
            if (args.Contains("--exit")) { using(var signal = new EventWaitHandle(false,EventResetMode.AutoReset,ExitSignalName)) signal.Set(); return 0; }
            if (args.Length == 3 && args[0] == "--session")
                return Session(int.Parse(args[1]), long.Parse(args[2]));
            if (args.Contains("--restore-worker")) return RestoreWorker();
            if (args.Contains("--restore"))
            {
                // An active elevated session owns the controller and its rollback.
                if (SessionRunning())
                {
                    RequestStop("emergency restore");
                    MessageBox.Show("已请求停止 LHDC 并恢复蓝牙。请等待主窗口显示恢复结果。", "LHDC Switch");
                    return 0;
                }
                Process p = Elevate("--restore-worker");
                if (p != null) p.WaitForExit();
                var s = ReadStatus();
                MessageBox.Show(Get(s, "message"), "蓝牙恢复", MessageBoxButtons.OK,
                    Get(s, "phase") == "restored" ? MessageBoxIcon.Information : MessageBoxIcon.Warning);
                return p == null ? 1 : p.ExitCode;
            }
            bool created;
            using (var showSignal = new EventWaitHandle(false,EventResetMode.AutoReset,ShowSignalName))
            using (var exitSignal = new EventWaitHandle(false,EventResetMode.AutoReset,ExitSignalName))
            using (var singleton = new Mutex(true, @"Local\LHDC-Switch-UI", out created))
            {
                if (!created) {
                    showSignal.Set();
                    foreach(var other in Process.GetProcessesByName("LHDC-Switch")) {
                        try { if(other.Id!=Process.GetCurrentProcess().Id && !String.Equals(other.MainModule.FileName,Exe,StringComparison.OrdinalIgnoreCase)) {
                            MessageBox.Show("另一个版本仍在运行，已显示它的窗口。请从其托盘菜单选择“退出并恢复蓝牙”，完成后再启动此版本。","切换测试版本"); break;
                        } } catch { } finally {other.Dispose();}
                    }
                    return 0;
                }
                Application.Run(new MainForm(showSignal,exitSignal));
            }
            return 0;
        }
        catch (Exception ex)
        {
            Log(ex.ToString());
            if (args.Contains("--self-test") || args.Contains("--ipc-test") || args.Contains("--diagnose"))
            {
                Save(Path.Combine(Data, args.Contains("--self-test") ? "selftest-results.json" : "test-failure.json"), new { passed = false, error = ex.ToString() });
                return 1;
            }
            MessageBox.Show(ex.Message, "LHDC Switch", MessageBoxButtons.OK, MessageBoxIcon.Error);
            return 1;
        }
    }

    internal static Dictionary<string, object> ReadObject(string path)
    {
        return Json.Deserialize<Dictionary<string, object>>(File.ReadAllText(path, Encoding.UTF8));
    }
    internal static string Get(Dictionary<string, object> obj, string key)
    {
        object value;
        return obj != null && obj.TryGetValue(key, out value) && value != null ? Convert.ToString(value) : "";
    }
    internal static Dictionary<string, object> Child(Dictionary<string, object> obj, string key)
    {
        object value;
        return obj != null && obj.TryGetValue(key, out value) ? value as Dictionary<string, object> : null;
    }
    internal static bool Bool(Dictionary<string, object> obj, string key)
    {
        return String.Equals(Get(obj, key), "True", StringComparison.OrdinalIgnoreCase);
    }
    internal static void Save(string path, object value)
    {
        string temp = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        File.WriteAllText(temp, Json.Serialize(value), new UTF8Encoding(false));
        try
        {
            if (File.Exists(path)) File.Replace(temp, path, null);
            else File.Move(temp, path);
        }
        finally { if (File.Exists(temp)) File.Delete(temp); }
    }
    internal static void Log(string text)
    {
        lock (LogLock)
        {
            try { File.AppendAllText(LogFile, DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss ") + text + Environment.NewLine, Encoding.UTF8); }
            catch { }
        }
    }
    internal static void Status(string phase, string message)
    {
        Log(message);
        Save(StatusFile, new { phase = phase, message = message, timestamp = DateTime.UtcNow.ToString("o") });
    }
    internal static Dictionary<string, object> ReadStatus()
    {
        try { return ReadObject(StatusFile); } catch { return new Dictionary<string, object>(); }
    }
    internal static Config LoadConfig()
    {
        var c = Json.Deserialize<Config>(File.ReadAllText(Path.Combine(Root, "settings.json"), Encoding.UTF8));
        Validate(c);
        return c;
    }
    internal static Config RecoveryTarget(Dictionary<string, object> record)
    {
        // Rollback must not depend on editable playback settings being valid.
        string id = record == null ? Json.Deserialize<Config>(File.ReadAllText(Path.Combine(Root, "settings.json"), Encoding.UTF8)).InstanceId
            : Get(record, "instance_id");
        if (String.IsNullOrWhiteSpace(id) || !id.StartsWith(AllowedPrefix, StringComparison.OrdinalIgnoreCase)
            || id.IndexOf('"') >= 0 || id.IndexOf('\n') >= 0)
            throw new InvalidOperationException("恢复记录没有有效的 Intel 设备标识。");
        return new Config { InstanceId = id };
    }
    internal static void Validate(Config c)
    {
        if (c == null || String.IsNullOrWhiteSpace(c.InstanceId) || !c.InstanceId.StartsWith(AllowedPrefix, StringComparison.OrdinalIgnoreCase)
            || c.InstanceId.IndexOf('"') >= 0 || c.InstanceId.IndexOf('\n') >= 0)
            throw new InvalidOperationException("此版本仅允许接管已核对的 Intel 8087:0026 蓝牙功能。");
        if (!Regex.IsMatch(c.Address ?? "", @"\A(?:[0-9A-Fa-f]{2}:){5}[0-9A-Fa-f]{2}\z"))
            throw new InvalidOperationException("耳机地址应为 XX:XX:XX:XX:XX:XX。");
        if (!QualityKeys.Contains(c.Quality)) throw new InvalidOperationException("未知音质选项。");
        if (!new[] { 44100, 48000, 96000, 192000 }.Contains(c.SampleRate)) throw new InvalidOperationException("不支持的采样率。");
        if (c.AfhPolicy != "auto" && c.AfhPolicy != "off") throw new InvalidOperationException("未知跳频策略。");
        if (!String.IsNullOrWhiteSpace(c.Application) && !Regex.IsMatch(c.Application, @"\A[\p{L}\p{N}_. -]+\.exe\z", RegexOptions.IgnoreCase))
            throw new InvalidOperationException("应用名称只需填写文件名，例如 music.exe。");
    }
    internal static string Quote(string s)
    {
        // Standard CommandLineToArgvW quoting, including trailing backslashes.
        var b = new StringBuilder("\""); int slashes = 0;
        foreach (char c in s)
        {
            if (c == '\\') { slashes++; continue; }
            if (c == '"') { b.Append('\\', slashes * 2 + 1); b.Append(c); slashes = 0; continue; }
            b.Append('\\', slashes); slashes = 0; b.Append(c);
        }
        b.Append('\\', slashes * 2); return b.Append('"').ToString();
    }
    internal static string Args(params string[] args) { return String.Join(" ", args.Select(Quote)); }
    internal static bool Admin()
    {
        return new WindowsPrincipal(WindowsIdentity.GetCurrent()).IsInRole(WindowsBuiltInRole.Administrator);
    }
    internal static Process Elevate(string arguments)
    {
        return Process.Start(new ProcessStartInfo(Exe, arguments) { UseShellExecute = true, Verb = "runas", WorkingDirectory = Root, WindowStyle = ProcessWindowStyle.Hidden });
    }
    internal static bool SessionRunning()
    {
        try { using (Mutex.OpenExisting(@"Local\LHDC-Switch-Session")) return true; }
        catch (WaitHandleCannotBeOpenedException) { return false; }
        catch (UnauthorizedAccessException) { return true; }
    }
    internal static void CheckEngine(bool includePlayback = true)
    {
        foreach (var item in new[] { new[] { "a2dpwb-core.exe", EngineIntegrity.Core }, new[] { "a2dpwb-driver.exe", EngineIntegrity.Driver } })
        {
            if (!includePlayback && item[0] == "a2dpwb-core.exe") continue;
            string path = Path.Combine(Engine, item[0]);
            if (!File.Exists(path)) throw new FileNotFoundException("缺少播放引擎文件", path);
            using (var sha = SHA256.Create()) using (var f = File.OpenRead(path))
                if (!String.Equals(BitConverter.ToString(sha.ComputeHash(f)).Replace("-", ""), item[1], StringComparison.OrdinalIgnoreCase))
                    throw new InvalidOperationException("引擎文件校验失败，请重新解压原始程序包：" + item[0]);
        }
        if (includePlayback) PrepareFirmware();
    }
    internal static void PrepareFirmware()
    {
        var pairs = new[] {
            new[] { "sfi", "2E6A2027B9EEB3AA94255F10752BB828077739104B8BE6769C7500322EDDDFD6" },
            new[] { "ddc", "FE272982577EFDC289CFE3E8BEDAFCA607C0B7D6C9DF1AAB1880E22EF930D077" }
        };
        string dest = Path.Combine(Data, "engine-config"); Directory.CreateDirectory(dest);
        foreach (var pair in pairs)
        {
            string source = Path.Combine(Root, "firmware", "ibt-19-0-4." + pair[0]);
            using (var sha = SHA256.Create()) using (var f = File.OpenRead(source))
                if (BitConverter.ToString(sha.ComputeHash(f)).Replace("-", "") != pair[1])
                    throw new InvalidOperationException("AX201 固件文件校验失败：" + source);
            // These aliases are explicitly listed by linux-firmware's WHENCE.
            foreach (string stem in new[] { "19-0-4", "19-0-0", "19-0-1", "19-0-3", "19-16-4", "19-32-1", "19-32-0", "19-32-4", "19-240-1", "19-240-4" })
            {
                string target = Path.Combine(dest, "ibt-" + stem + "." + pair[0]);
                bool matches = false;
                if (File.Exists(target)) using (var sha = SHA256.Create()) using (var f = File.OpenRead(target))
                    matches = BitConverter.ToString(sha.ComputeHash(f)).Replace("-", "") == pair[1];
                if (!matches) File.Copy(source, target, true);
            }
        }
    }
    internal static ProcessStartInfo StartInfo(string file, IEnumerable<string> arguments)
    {
        return new ProcessStartInfo(file, String.Join(" ", arguments.Select(Quote))) {
            UseShellExecute = false, CreateNoWindow = true, WorkingDirectory = Root,
            RedirectStandardOutput = true, RedirectStandardError = true,
            StandardOutputEncoding = Encoding.UTF8, StandardErrorEncoding = Encoding.UTF8 };
    }
    internal static Result Run(string exe, int timeout, params string[] arguments)
    {
        using (var p = new Process { StartInfo = StartInfo(exe, arguments) })
        {
            p.Start(); var output = p.StandardOutput.ReadToEndAsync(); var error = p.StandardError.ReadToEndAsync();
            if (!p.WaitForExit(timeout)) { try { p.Kill(); } catch { } throw new TimeoutException(Path.GetFileName(exe) + " 超时；请查看恢复状态。"); }
            Task.WaitAll(output, error);
            return new Result { Code = p.ExitCode, Output = output.Result, Error = error.Result };
        }
    }
    internal static Dictionary<string, object> Driver(params string[] arguments)
    {
        var result = Run(Path.Combine(Engine, "a2dpwb-driver.exe"), 110000, arguments);
        Log("driver " + arguments[0] + ": " + result.Output + result.Error);
        if (String.IsNullOrWhiteSpace(result.Output)) throw new InvalidOperationException("驱动工具未返回状态：" + result.Error);
        var json = Json.Deserialize<Dictionary<string, object>>(result.Output);
        if (result.Code != 0 || !Bool(json, "ok"))
            throw new InvalidOperationException("驱动操作未通过验证：" + Get(Child(json, "error"), "message") + " (" + result.Code + ")");
        return json;
    }
    internal static Dictionary<string, object> DriverState(Config c)
    {
        return Child(Driver("state", c.InstanceId), "state");
    }
    internal static Dictionary<string, object> Audio(params string[] arguments)
    {
        string helper = Path.Combine(Root, "AudioControl.exe");
        using (var sha = SHA256.Create()) using (var f = File.OpenRead(helper))
            if (BitConverter.ToString(sha.ComputeHash(f)).Replace("-", "") != EngineIntegrity.Audio)
                throw new InvalidOperationException("扬声器控制工具校验失败。");
        var result = Run(helper, 15000, arguments);
        if (result.Code != 0) throw new InvalidOperationException("扬声器操作失败：" + result.Output + result.Error);
        return Json.Deserialize<Dictionary<string, object>>(result.Output);
    }
    internal static string FindCable()
    {
        var devices = Audio("list")["devices"] as System.Collections.IEnumerable;
        foreach (object item in devices)
        {
            var d = item as Dictionary<string, object>; string name = Get(d, "name");
            if (Get(d, "state") == "1" && name.IndexOf("CABLE Input", StringComparison.OrdinalIgnoreCase) >= 0
                && name.IndexOf("VB-Audio", StringComparison.OrdinalIgnoreCase) >= 0) return Get(d, "id");
        }
        return "";
    }
    internal static bool AudioActive(string id)
    {
        if (String.IsNullOrWhiteSpace(id)) return false;
        try { return Get(Audio("snapshot", id), "state") == "1"; } catch { return false; }
    }
    internal static void RouteToCable(string id)
    {
        var record = ReadObject(Journal);
        var fallback = Audio("defaults");
        // The previous Windows headset may not have disappeared yet.
        // Keep a known physical speaker as fallback if it cannot return.
        string speaker = "";
        foreach (object item in (System.Collections.IEnumerable)Audio("list")["devices"])
        {
            var device = item as Dictionary<string, object>;
            if (Get(device, "name").IndexOf("Senary", StringComparison.OrdinalIgnoreCase) >= 0) { speaker = Get(device, "id"); break; }
        }
        if (!String.IsNullOrEmpty(speaker))
            for (int role = 0; role < 3; role++)
            {
                string candidate = Get(fallback, role.ToString());
                string name = AudioActive(candidate) ? Get(Audio("snapshot", candidate), "name") : "";
                if (String.IsNullOrEmpty(name) || name.IndexOf("OPPO", StringComparison.OrdinalIgnoreCase) >= 0 || name.IndexOf("CABLE", StringComparison.OrdinalIgnoreCase) >= 0)
                    fallback[role.ToString()] = speaker;
            }
        record["audio_fallback"] = fallback; record["routed_to"] = id;
        Save(Journal, record); // Before the first change of an output role.
        for (int role = 0; role < 3; role++) Audio("set-default", id, role.ToString());
        Log("系统输出已切换到 VB-CABLE；原默认输出已记录。");
    }
    internal static void RestoreAudio(Dictionary<string, object> record, Action<int, string> setDefault, Func<string, bool> active)
    {
        var original = Child(record, "audio_defaults");
        if (original == null || String.IsNullOrEmpty(Get(record, "routed_to"))) return;
        var fallback = Child(record, "audio_fallback");
        for (int role = 0; role < 3; role++)
        {
            string id = Get(original, role.ToString());
            if (!active(id)) id = Get(fallback, role.ToString());
            if (!active(id)) throw new InvalidOperationException("无法恢复原播放设备（音频角色 " + role + "），可在 Windows 声音设置中选择扬声器。");
            setDefault(role, id);
        }
    }
    internal static void OpenCableInstaller()
    {
        string installer = Path.Combine(Root, "dependencies", "VBCABLE", "VBCABLE_Setup_x64.exe");
        using (var sha = SHA256.Create()) using (var f = File.OpenRead(installer))
            if (BitConverter.ToString(sha.ComputeHash(f)).Replace("-", "") != "734C35DFA6D98F48782A451633CEB471166EC70D60482FD89A1123D0EE3C4F41")
                throw new InvalidOperationException("VB-CABLE 原版安装器校验失败。");
        Process.Start(new ProcessStartInfo(installer) { UseShellExecute = true, Verb = "runas", WorkingDirectory = Path.GetDirectoryName(installer) });
    }
    internal static void RequireHealthyBluetooth(Dictionary<string, object> state)
    {
        if (Get(state, "kind") != "bluetooth" || !Bool(state, "present") ||
            (!String.IsNullOrEmpty(Get(state, "problem_code")) && Get(state, "problem_code") != "0"))
            throw new InvalidOperationException("蓝牙未处于正常的 Windows 驱动状态。请先点击“恢复 Windows 蓝牙”。");
    }
    internal static List<string> CoreArgs(Config c)
    {
        var a = new List<string> { "--cli", "-d", c.Address, "-u", c.InstanceId, "-c", "lhdcv5", "-q", EngineQuality(c),
            "--rate", c.SampleRate.ToString(), "--bits", "24" };
        if (IsAdaptive(c)) a.Add("-a");
        if (c.TestTone) a.Add("--test-tone");
        else if (!String.IsNullOrWhiteSpace(c.Application)) { a.AddRange(new[] { "-m", "app", "--app", c.Application }); }
        else if (c.ExclusiveOutput) a.AddRange(new[] { "-m", "virtual", "--audio-device", c.AudioDeviceId ?? "VB-CABLE (尚未安装)" });
        else a.AddRange(new[] { "-m", "loopback" });
        return a;
    }
    internal static bool ParentAlive(int pid, long ticks)
    {
        try { using (var p = Process.GetProcessById(pid)) return !p.HasExited && p.StartTime.ToUniversalTime().Ticks == ticks; }
        catch { return false; }
    }
    internal static void PrepareResidentConfig(string directory, Config config = null)
    {
        Directory.CreateDirectory(directory);
        string path = Path.Combine(directory, "settings.json");
        var settings = File.Exists(path) ? ReadObject(path) : new Dictionary<string, object>();
        settings["schema_version"] = 3; settings["language"] = "zh-CN";
        settings["check_updates_on_startup"] = false; settings["start_with_windows"] = false;
        settings["le_audio_experimental"] = false; settings["remember_volume"] = false;
        settings["afh"] = config == null ? "auto" : config.AfhPolicy; settings["debug_mode"] = false;
        Save(path, settings);
    }
    internal static bool IsAdaptive(Config c) { return c.Quality != null && c.Quality.StartsWith("abr", StringComparison.Ordinal); }
    internal static int AdaptiveCeiling(Config c)
    {
        return IsAdaptive(c) && c.Quality.Length>3 ? Int32.Parse(c.Quality.Substring(3)) : 400;
    }
    internal static int FixedBitrate(Config c) { return c!=null && c.Quality!=null && c.Quality.StartsWith("fixed",StringComparison.Ordinal) ? Int32.Parse(c.Quality.Substring(5)) : 0; }
    internal static string EngineQuality(Config c) { return IsAdaptive(c)?"mq":FixedBitrate(c)>0?"mhq":c.Quality; }
    internal static Process StartResident(string directory, string instance, Config config = null)
    {
        PrepareResidentConfig(directory, config);
        var start = StartInfo(Path.Combine(Engine, "a2dpwb-core.exe"), new[] { "--background" });
        start.EnvironmentVariables["A2DPWB_CONFIG_DIR"] = directory;
        start.EnvironmentVariables["A2DPWB_INSTANCE"] = instance;
        start.EnvironmentVariables["LHDC_SWITCH_ABR_MAX_KBPS"] = (config == null ? 400 : AdaptiveCeiling(config)).ToString();
        start.EnvironmentVariables["LHDC_SWITCH_FIXED_KBPS"] = FixedBitrate(config).ToString();
        start.EnvironmentVariables["LHDC_SWITCH_ACTIVE_LINK"] = config == null || config.ActiveRadio ? "1" : "0";
        start.EnvironmentVariables["LHDC_SWITCH_NO_TRAY"] = "1";
        var process = Process.Start(start);
        try { process.PriorityClass = ProcessPriorityClass.AboveNormal; } catch { }
        return process;
    }
    internal static object CustomStream(Config c)
    {
        return new { device = c.Address, device_name = c.Headphone, codec = "lhdcv5", quality = EngineQuality(c),
            sample_rate = c.SampleRate, bit_depth = 24, source = c.TestTone ? "tone" : !String.IsNullOrEmpty(c.Application) ? "app" : c.ExclusiveOutput ? "virtual" : "loopback",
            source_id = !String.IsNullOrEmpty(c.Application) ? c.Application : c.AudioDeviceId, source_name = c.ExclusiveOutput ? "VB-CABLE" : c.Application,
            max_media_payload = 1023, abr = IsAdaptive(c), codec_fallback = false };
    }
    internal static Dictionary<string, object> EarbudSettings(bool enable)
    { return EarbudSettings(enable ? "enable-lhdc-hires" : "inspect"); }
    internal static Dictionary<string, object> EarbudSettings(string command)
    {
        if(!EarbudCommands.All.Contains(command)) throw new InvalidOperationException("不支持的耳机设置命令。");
        if (SessionRunning()) throw new InvalidOperationException("请先断开 LHDC 并恢复 Windows 蓝牙，再读取或修改耳机设置。");
        string helper = Path.Combine(Root, "EarbudControl.exe");
        using (var sha = SHA256.Create()) using (var f = File.OpenRead(helper))
            if (BitConverter.ToString(sha.ComputeHash(f)).Replace("-", "") != EngineIntegrity.Earbud) throw new InvalidOperationException("耳机设置工具校验失败。");
        var result = Run(helper, 35000, command, LoadConfig().Address);
        var report = Json.Deserialize<Dictionary<string, object>>(result.Output);
        Save(Path.Combine(Data, command=="inspect" ? "earbud-last-read.json" : "earbud-last-write.json"), report);
        if (result.Code != 0) throw new InvalidOperationException(Get(report, "error").Length > 0 ? Get(report, "error") : "耳机拒绝设置，状态码 " + Get(report, "refused_status"));
        return report;
    }
    internal static int IpcSmokeTest()
    {
        CheckEngine(); Process core = null; EngineClient client = null;
        try
        {
            string instance = "lhdctest" + Process.GetCurrentProcess().Id;
            core = StartResident(Path.Combine(Data, "ipc-test"), instance);
            client = new EngineClient(instance); client.Connect(15000);
            client.Call("diag.subscribe", new { logs = "info", link = true });
            var snapshot = client.Call("state.snapshot", null);
            var devices = client.Call("audio.devices", null);
            Thread.Sleep(400);
            var stream = Child(Child(snapshot, "result"), "stream");
            if (Get(stream, "state") != "idle") throw new Exception("IPC smoke test expected idle engine.");
            string variantPath=Path.Combine(Root,"variant.json");
            string expectedBackend=File.Exists(variantPath)?Get(ReadObject(variantPath),"backend"):"";
            var timeout=Stopwatch.StartNew();
            while(expectedBackend.Length>0 && Get(Child(client.Telemetry(),"link"),"encoder_backend").Length==0 && timeout.ElapsedMilliseconds<3000) Thread.Sleep(100);
            if(expectedBackend.Length>0 && Get(Child(client.Telemetry(),"link"),"encoder_backend")!=expectedBackend) throw new Exception("IPC encoder backend does not match the test variant.");
            Save(Path.Combine(Data, "ipc-test-results.json"), new { passed = true, protocol = 1, state = stream,
                audio_devices = devices["result"], telemetry = client.Telemetry(), bluetooth_stream_started = false, driver_mutations = 0 });
            return 0;
        }
        finally
        {
            if (client != null) { try { client.Call("app.quit", null, 3000); } catch { } client.Dispose(); }
            if (core != null) { if (!core.WaitForExit(7000)) core.Kill(); core.Dispose(); }
        }
    }

    internal static void RequestStop(string reason) {
        File.WriteAllText(StopFile,reason);
        try { using(var signal=EventWaitHandle.OpenExisting(StopSignalName)) signal.Set(); } catch(WaitHandleCannotBeOpenedException) { }
    }

    internal static int Session(int parentId, long parentTicks)
    {
        if (!Admin()) throw new InvalidOperationException("接管与恢复需要管理员权限。");
        bool created;
        using (var gate = new Mutex(true, @"Local\LHDC-Switch-Session", out created))
        using (var stopSignal = new EventWaitHandle(false,EventResetMode.ManualReset,StopSignalName))
        {
            if (!created) throw new InvalidOperationException("另一个播放或恢复会话正在运行。");
            stopSignal.Reset();
            Process core = null; EngineClient client = null; bool restoreNeeded = false; string failure = null;
            try
            {
                CheckEngine(); Config c = LoadConfig();
                if (c.ExclusiveOutput && !c.TestTone)
                {
                    c.AudioDeviceId = FindCable();
                    if (String.IsNullOrEmpty(c.AudioDeviceId)) throw new InvalidOperationException("请先点击“安装 VB-CABLE”，在官方安装器中安装虚拟声卡；按其提示重启后再播放。");
                }
                if (!ParentAlive(parentId, parentTicks)) throw new InvalidOperationException("控制窗口已关闭。");
                if (File.Exists(Journal)) throw new InvalidOperationException("检测到上次未完成的恢复记录，请先恢复蓝牙。");
                Status("preparing", "正在核对原驱动并导出备份……");
                var state = DriverState(c); RequireHealthyBluetooth(state);
                string inf = Get(Child(state, "driver"), "inf_name");
                if (!Regex.IsMatch(inf, @"\Aoem\d+\.inf\z", RegexOptions.IgnoreCase))
                    throw new InvalidOperationException("未识别到可导出的 Intel OEM 驱动，停止接管。");
                string backup = Path.Combine(Data, "driver-backup", DateTime.Now.ToString("yyyyMMdd-HHmmss-fff"));
                Directory.CreateDirectory(backup);
                var export = Run(Path.Combine(Environment.SystemDirectory, "pnputil.exe"), 60000, "/export-driver", inf, backup);
                Log(export.Output + export.Error);
                if (export.Code != 0 || Directory.GetFiles(backup, "*.inf", SearchOption.AllDirectories).Length == 0)
                    throw new InvalidOperationException("原驱动备份失败，未切换驱动。");
                // Persist BEFORE the first mutation: even a crashed assign helper has a recovery record.
                Save(Journal, new { instance_id = c.InstanceId, previous = Child(state, "driver"), original_state = state,
                    audio_defaults = Audio("defaults"), backup = backup, created = DateTime.UtcNow.ToString("o") });
                restoreNeeded = true;
                if (stopSignal.WaitOne(0) || File.Exists(StopFile) || !ParentAlive(parentId, parentTicks)) throw new OperationCanceledException("已取消接管。");
                Status("assigning", "正在把 Intel 蓝牙交给 LHDC 程序……");
                Driver("assign", c.InstanceId);
                if (stopSignal.WaitOne(0) || File.Exists(StopFile) || !ParentAlive(parentId, parentTicks)) throw new OperationCanceledException("已停止，开始恢复。");
                if (c.ExclusiveOutput && !c.TestTone) RouteToCable(c.AudioDeviceId);
                Status("connecting", "正在连接耳机；首次使用请把耳机置于配对模式……");
                // Resident mode exposes measured link counters and AVRCP volume.
                // Its default USB selection is safe only when our adapter is the
                // sole WinUSB Bluetooth radio. Refuse ambiguity before opening HCI.
                var adapters = Driver("list")["adapters"] as System.Collections.IEnumerable;
                var winusb = new List<Dictionary<string, object>>();
                foreach (object adapter in adapters) { var d = adapter as Dictionary<string, object>; if (Get(d, "kind") == "winusb") winusb.Add(d); }
                if (winusb.Count != 1 || !String.Equals(Get(winusb[0], "instance_id"), c.InstanceId, StringComparison.OrdinalIgnoreCase))
                    throw new InvalidOperationException("检测到多个可被接管的蓝牙控制器，停止以避免选择错误设备。");
                core = StartResident(Path.Combine(Data, "engine-config"), EngineClient.Instance, c);
                client = new EngineClient(EngineClient.Instance); client.LogReceived = Log; client.Connect(15000);
                client.Call("diag.subscribe", new { logs = "info", link = true });
                client.Call("stream.start_custom", CustomStream(c));
                var connection = Stopwatch.StartNew(); bool reported = false; string volumeSequence = File.Exists(VolumeFile) ? Get(ReadObject(VolumeFile), "sequence") : ""; long lastCsv = 0, lastLive = -250, volumeActiveUntil=0;
                string csv = Path.Combine(Data, "link-" + DateTime.Now.ToString("yyyyMMdd-HHmmss") + ".csv");
                File.WriteAllText(csv, "utc,codec,encoder_kbps,tx_kbps,queue,dropped,capture_dropped,rssi,link_quality,tx_power_dbm,tx_power_max_dbm,link_mode,link_policy,afh_in_use,send_gaps_30ms,send_gap_max_us,capture_discontinuities,encoder_backend,queue_audio_us,queue_wait_us,queue_wait_max_us,pacing_late_max_us,pacing_rebases\r\n", Encoding.UTF8);
                while (!core.WaitForExit(connection.ElapsedMilliseconds<volumeActiveUntil?50:250))
                {
                    if (stopSignal.WaitOne(0) || File.Exists(StopFile) || !ParentAlive(parentId, parentTicks)) break;
                    var live = client.Telemetry(); var stream = Child(live, "stream"); var info = Child(stream, "info"); var link = Child(live, "link");
                    if(connection.ElapsedMilliseconds-lastLive>=(connection.ElapsedMilliseconds<volumeActiveUntil?100:250)) { Save(LiveFile, live); lastLive=connection.ElapsedMilliseconds; }
                    if (!String.IsNullOrEmpty(client.LastError)) throw new InvalidOperationException("实时接口断开：" + client.LastError);
                    if (Get(stream, "state") == "error")
                    {
                        string key = Get(Child(stream, "message"), "key");
                        throw new InvalidOperationException(key == "error.connect_auth_failed" ? "配对认证失败，请将耳机置于配对模式后重新连接。" : "播放失败：" + key);
                    }
                    if (Get(stream, "state") == "streaming")
                    {
                        if (!Get(info, "codec").StartsWith("LHDC", StringComparison.OrdinalIgnoreCase)) throw new InvalidOperationException("引擎报告的编码并非 LHDC，已停止。");
                        if (!reported) { Status("streaming", "LHDC v5 播放中，实时数据已连接。"); reported = true; }
                    }
                    if (File.Exists(VolumeFile))
                    {
                        var request = ReadObject(VolumeFile); string seq = Get(request, "sequence");
                        if (seq != volumeSequence)
                        {
                            volumeSequence = seq; int volume = Convert.ToInt32(request["volume"]);
                            if (volume < 0 || volume > 127) throw new InvalidOperationException("耳机音量请求越界。");
                            client.Call("stream.set_volume", new { volume = volume });
                            volumeActiveUntil=connection.ElapsedMilliseconds+1800;
                        }
                    }
                    if (connection.ElapsedMilliseconds - lastCsv >= 1000)
                    {
                        lastCsv = connection.ElapsedMilliseconds;
                        File.AppendAllText(csv, String.Join(",", DateTime.UtcNow.ToString("o"), Get(info, "codec"),
                            LiveData.Number(link, "encoder_kbps") > 0 ? Get(link, "encoder_kbps") : Get(info, "bitrate_kbps"),
                            (LiveData.Number(link, "bytes_per_second") * .008).ToString("F1", System.Globalization.CultureInfo.InvariantCulture),
                            Get(link, "queue_depth"), Get(link, "dropped"), Get(link, "capture_dropped"), Get(link, "rssi"),
                            Get(link,"link_quality"),Get(link,"tx_power_dbm"),Get(link,"tx_power_max_dbm"),Get(link,"link_mode"),Get(link,"link_policy"),Get(link,"afh_in_use"),
                            Get(link,"send_gaps_30ms"),Get(link,"send_gap_max_us"),Get(link,"capture_discontinuities"),
                            Get(link,"encoder_backend"),Get(link,"queue_audio_us"),Get(link,"queue_wait_us"),Get(link,"queue_wait_max_us"),Get(link,"pacing_late_max_us"),Get(link,"pacing_rebases")) + "\r\n", Encoding.UTF8);
                    }
                    if (!reported && connection.Elapsed.TotalSeconds > 120)
                        throw new TimeoutException("120 秒内未确认 LHDC 播放；已停止并准备恢复蓝牙。请检查耳机配对模式及日志。");
                }
                if (core.HasExited) { core.WaitForExit(); failure = "播放引擎已退出（" + core.ExitCode + "）。"; }
            }
            catch (Exception ex) { failure = ex.Message; Log(ex.ToString()); }
            finally
            {
                // Release every HCI handle before asking Windows to reinstall the Bluetooth driver.
                RecoveryFlow.Finish(restoreNeeded, delegate
                {
                    if (client != null)
                    {
                        try { var diag = client.Call("diag.info", new { mask = true }, 3000); Save(Path.Combine(Data, "last-engine-diagnostics.json"), diag); } catch { }
                        try { client.Call("stream.stop", null, 5000); client.Call("app.quit", null, 3000); } catch { }
                        client.Dispose();
                    }
                    if (core == null) return;
                    if (!core.HasExited && !core.WaitForExit(6000)) { core.Kill(); if (!core.WaitForExit(10000)) throw new TimeoutException("播放进程未及时退出。"); }
                    core.Dispose();
                }, delegate { Restore(failure); }, delegate(Exception ex)
                {
                    Status("restore_failed", "恢复未完成：" + ex.Message + "。请运行“恢复蓝牙.cmd”。备份和恢复记录已保留。");
                });
                Save(LiveFile, new { timestamp = DateTime.UtcNow.ToString("o"), active = false });
                if (!restoreNeeded) Status("error", failure ?? "会话未启动。");
            }
            return File.Exists(Journal) ? 1 : 0;
        }
    }

    internal static int RestoreWorker()
    {
        if (!Admin()) throw new InvalidOperationException("恢复需要管理员权限。");
        bool created;
        using (var gate = new Mutex(true, @"Local\LHDC-Switch-Session", out created))
        {
            if (!created) { RequestStop("stop"); return 1; }
            try { CheckEngine(false); Restore(null); return 0; }
            catch (Exception ex) { Status("restore_failed", "恢复未通过验证：" + ex.Message + "。恢复记录与备份已保留。"); return 1; }
        }
    }
    internal static void Restore(string reason)
    {
        Status("restoring", "正在释放蓝牙并恢复 Windows 驱动，请稍候……");
        Dictionary<string, object> record = null;
        if (File.Exists(Journal))
        {
            record = ReadObject(Journal);
        }
        Config c = RecoveryTarget(record);
        string driverError = null, audioError = null;
        try
        {
            if (record != null) Driver("restore", c.InstanceId, "--previous", "@" + Journal);
            else Driver("restore", c.InstanceId);
            var after = DriverState(c); RequireHealthyBluetooth(after);
            if (record != null)
            {
                var expected = Child(record, "previous"); var actual = Child(after, "driver");
                foreach (string key in new[] { "inf_name", "service", "version" })
                    if (!String.Equals(Get(expected, key), Get(actual, key), StringComparison.OrdinalIgnoreCase))
                        throw new InvalidOperationException("Windows 已加载蓝牙驱动，但与备份的 " + key + " 不同；请查看日志。");
            }
        }
        catch (Exception ex) { driverError = ex.Message; Log(ex.ToString()); }
        // Even if Bluetooth restoration fails, give physical speakers back.
        try { RestoreAudio(record, (role, id) => Audio("set-default", id, role.ToString()), AudioActive); }
        catch (Exception ex) { audioError = ex.Message; Log(ex.ToString()); }
        if (driverError != null || audioError != null)
            throw new InvalidOperationException(String.Join("；", new[] { driverError, audioError }.Where(x => x != null)));
        if (record != null)
        {
            string archive = Path.Combine(Data, "restored-" + DateTime.Now.ToString("yyyyMMdd-HHmmss-fff") + ".json");
            File.Move(Journal, archive);
        }
        Status("restored", "Windows 蓝牙及原播放设备已恢复。" + (String.IsNullOrEmpty(reason) ? "" : "\r\n" + reason));
    }
    internal static void Diagnose()
    {
        CheckEngine(); Config c = LoadConfig();
        var state = DriverState(c);
        Save(Path.Combine(Data, "diagnostics.json"), new { engine = "A2DPWB 2.0.1", target = c, driver = state,
            audio = Audio("snapshot"), recovery_pending = File.Exists(Journal), arguments = CoreArgs(c), generated = DateTime.UtcNow.ToString("o") });
    }
}

internal sealed class Config
{
    public Config() { ExclusiveOutput = true; AfhPolicy = "auto"; ActiveRadio = true; CloseToTray = true; }
    public string InstanceId { get; set; }
    public string Address { get; set; }
    public string Headphone { get; set; }
    public string Quality { get; set; }
    public string Application { get; set; }
    public int SampleRate { get; set; }
    public bool TestTone { get; set; }
    public bool ExclusiveOutput { get; set; }
    public string AudioDeviceId { get; set; }
    public string AfhPolicy { get; set; }
    public bool ActiveRadio { get; set; }
    public bool CloseToTray { get; set; }
}
internal sealed class Result { public int Code; public string Output; public string Error; }

internal static class Tests
{
    internal static int Run()
    {
        var checks = new List<string>();
        Action<bool, string> check = (ok, name) => { if (!ok) throw new Exception("FAILED: " + name); checks.Add(name); };
        var c = Program.LoadConfig(); c.Quality = "sq";
        check(Program.Quote(@"D:\音频 文件\") == "\"D:\\音频 文件\\\\\"", "Windows argument quoting / Unicode / trailing slash");
        var args = Program.CoreArgs(c);
        check(args.Contains("lhdcv5") && args.Contains(c.InstanceId) && !args.Contains("auto"), "explicit target and LHDC only");
        check(!args.Contains("-a"), "fixed quality does not silently enable ABR");
        c.Quality = "abr"; check(Program.CoreArgs(c).Contains("-a"), "adaptive quality");
        foreach (string adaptive in new[] { "abr500", "abr700", "abr750", "abr800", "abr900", "abr1000" })
        {
            c.Quality = adaptive; Program.Validate(c);
            var adaptiveStream = Program.Json.Deserialize<Dictionary<string, object>>(Program.Json.Serialize(Program.CustomStream(c)));
            check(Program.CoreArgs(c).Contains("-a") && Program.Bool(adaptiveStream, "abr") && Program.Get(adaptiveStream, "quality") == "mq"
                && Program.AdaptiveCeiling(c).ToString() == adaptive.Substring(3), "adaptive ceiling " + adaptive);
        }
        foreach(int fixedRate in new[]{700,750,800}) {
            c.Quality="fixed"+fixedRate;Program.Validate(c);
            var fixedStream=Program.Json.Deserialize<Dictionary<string,object>>(Program.Json.Serialize(Program.CustomStream(c)));
            check(!Program.Bool(fixedStream,"abr") && Program.Get(fixedStream,"quality")=="mhq" && Program.FixedBitrate(c)==fixedRate,"exact fixed bitrate "+fixedRate);
        }
        c.Application = "music.exe"; c.TestTone = false; check(Program.CoreArgs(c).Contains("--app"), "application capture");
        c.TestTone = true; check(Program.CoreArgs(c).Contains("--test-tone") && !Program.CoreArgs(c).Contains("--app"), "test tone takes precedence");
        c = Program.LoadConfig(); c.InstanceId = @"PCI\VEN_8086";
        bool rejected = false; try { Program.Validate(c); } catch { rejected = true; } check(rejected, "reject unrelated hardware");
        c = Program.LoadConfig(); c.Address = "x\" --scan";
        rejected = false; try { Program.Validate(c); } catch { rejected = true; } check(rejected, "reject invalid address");
        c = Program.LoadConfig(); c.Application = "x.exe\" --scan";
        rejected = false; try { Program.Validate(c); } catch { rejected = true; } check(rejected, "reject invalid application");
        string p = Path.Combine(Program.Data, "selftest-atomic.json");
        Program.Save(p, new { phase = "one" }); Program.Save(p, new { phase = "two" });
        check(Program.Get(Program.ReadObject(p), "phase") == "two", "atomic persistent journal writes"); File.Delete(p);
        check(!Program.ParentAlive(Int32.MaxValue, 0), "detect dead controller process");
        var steps = new List<string>();
        RecoveryFlow.Finish(true, () => steps.Add("release"), () => steps.Add("restore"), ex => steps.Add("error"));
        check(String.Join(",", steps) == "release,restore", "session exit releases controller before restoring driver");
        steps.Clear();
        RecoveryFlow.Finish(true, () => { steps.Add("release"); throw new Exception("simulated process cleanup failure"); },
            () => steps.Add("restore"), ex => steps.Add("error"));
        check(String.Join(",", steps) == "release,restore", "cleanup failure still attempts restoration");
        steps.Clear();
        RecoveryFlow.Finish(false, () => steps.Add("release"), () => steps.Add("restore"), ex => steps.Add("error"));
        check(String.Join(",", steps) == "release", "no driver rollback before recovery is armed");
        steps.Clear();
        RecoveryFlow.Finish(true, () => steps.Add("release"), () => { steps.Add("restore"); throw new Exception("simulated restore failure"); }, ex => steps.Add("error"));
        check(String.Join(",", steps) == "release,restore,error", "restore failure is reported rather than claimed as success");
        var recoveryRecord = new Dictionary<string, object> { { "instance_id", Program.LoadConfig().InstanceId } };
        check(Program.RecoveryTarget(recoveryRecord).InstanceId == Program.LoadConfig().InstanceId, "recovery uses persisted hardware record independently of playback settings");
        check(Program.Json.Deserialize<Config>("{}").ExclusiveOutput, "existing settings default to dedicated virtual output");
        var radioDefaults=Program.Json.Deserialize<Config>("{}");
        check(radioDefaults.ActiveRadio && radioDefaults.AfhPolicy=="auto", "existing settings enable audio link policy and automatic AFH");
        check(radioDefaults.CloseToTray,"close-to-tray defaults on for existing settings");
        var volumeModel=new VolumePresentation();var volumeNow=DateTime.UtcNow;
        volumeModel.Observe(56,volumeNow);volumeModel.Request(32,volumeNow);volumeModel.Observe(56,volumeNow.AddMilliseconds(100));
        check(volumeModel.Display==32&&volumeModel.Pending==32,"old feedback does not bounce volume slider");
        volumeModel.Observe(31,volumeNow.AddMilliseconds(300));check(volumeModel.Display==31&&volumeModel.Pending<0,"quantized earbud confirmation updates volume");
        volumeModel.Request(20,volumeNow);volumeModel.Observe(31,volumeNow.AddSeconds(2));check(volumeModel.TimedOut&&volumeModel.Display==31,"volume timeout returns to confirmed earbud value");
        check(EarbudCommands.Matches("anc-off",new Dictionary<string,object>{{"anc_mask",8}})&&EarbudCommands.Matches("anc-transparency",new Dictionary<string,object>{{"anc_mask",256}}),"ANC parent commands accept documented child report slots");
        check(!EarbudCommands.Matches("spatial-on",new Dictionary<string,object>())&&!EarbudCommands.Matches("anc-deep",new Dictionary<string,object>{{"anc_mask",2}}),"missing or generic status does not verify a requested effect");
        c=Program.LoadConfig(); c.AfhPolicy="invalid"; rejected=false;
        try { Program.Validate(c); } catch { rejected=true; } check(rejected,"reject invalid AFH policy");
        var restoredRoles = new Dictionary<int, string>();
        recoveryRecord["audio_defaults"] = new Dictionary<string, object> { { "0", "original-console" }, { "1", "original-media" }, { "2", "original-comms" } };
        recoveryRecord["audio_fallback"] = new Dictionary<string, object> { { "0", "speaker" }, { "1", "speaker" }, { "2", "speaker" } };
        recoveryRecord["routed_to"] = "cable";
        Program.RestoreAudio(recoveryRecord, (role, id) => restoredRoles[role] = id, id => true);
        check(restoredRoles.Count == 3 && restoredRoles[0] == "original-console" && restoredRoles[1] == "original-media" && restoredRoles[2] == "original-comms", "restore all saved output roles separately");
        Program.RestoreAudio(recoveryRecord, (role, id) => restoredRoles[role] = id, id => id == "speaker");
        check(restoredRoles.Values.All(id => id == "speaker"), "use saved speaker fallback when original headset is unavailable");
        Program.RestoreAudio(null, (role, id) => { throw new Exception("unexpected route mutation"); }, id => true);
        check(true, "old recovery records do not change audio routing");
        var virtualConfig = Program.LoadConfig(); virtualConfig.ExclusiveOutput = true; virtualConfig.TestTone = false; virtualConfig.Application = ""; virtualConfig.AudioDeviceId = "chosen-cable";
        check(Program.CoreArgs(virtualConfig).Contains("chosen-cable") && Program.CoreArgs(virtualConfig).Contains("virtual") && !Program.CoreArgs(virtualConfig).Contains("loopback"), "exclusive output captures the dedicated endpoint");
        string installedCable = Program.FindCable();
        check(String.IsNullOrEmpty(installedCable) || Program.AudioActive(installedCable), "detect installed cable or cleanly report missing dependency");
        var frame = EngineClient.Frame(new { type = "hello", protocol = 1, client = "测试" });
        check(BitConverter.ToInt32(frame, 0) == frame.Length - 4 && Encoding.UTF8.GetString(frame, 4, frame.Length - 4).Contains("测试"), "length-prefixed UTF8 engine protocol");
        var st = Program.Json.Deserialize<Dictionary<string, object>>(Program.Json.Serialize(Program.CustomStream(virtualConfig)));
        check(Program.Get(st, "codec") == "lhdcv5" && !Program.Bool(st, "codec_fallback") && Program.Get(st, "source_id") == "chosen-cable", "resident stream stays LHDC-only and binds dedicated audio source");
        var prior = new Dictionary<string, object> { { "capture_dropped", 0 }, { "dropped", 0 } };
        var captureLoss = new Dictionary<string, object> { { "capture_dropped", 240 }, { "dropped", 0 } };
        check(LiveData.Health(captureLoss, prior).StartsWith("采集丢帧"), "diagnosis distinguishes capture loss");
        check(LiveData.Health(new Dictionary<string, object> { { "dropped", 2 } }, prior).StartsWith("发送队列"), "diagnosis distinguishes local send loss");
        check(!LiveData.Health(new Dictionary<string, object> {{"queue_audio_us",40000},{"queue_wait_us",22000}},prior).StartsWith("主机发送拥塞"),"normal pair of 20 ms packets does not report persistent congestion");
        check(LiveData.Health(new Dictionary<string, object> {{"queue_audio_us",80000},{"queue_wait_us",75000}},prior).StartsWith("主机发送拥塞"),"diagnosis still reports actual sustained backlog");
        Program.CheckEngine(); check(true, "pinned engine SHA256");
        Program.Diagnose(); check(true, "real read-only driver query");
        Program.Save(Path.Combine(Program.Data, "selftest-results.json"), new { passed = checks.Count, checks = checks, hardware_mutations = 0 });
        return 0;
    }
}

internal static class RecoveryFlow
{
    // Shared by the real elevated session and fault-injection tests.
    internal static void Finish(bool armed, Action release, Action rollback, Action<Exception> reportFailure)
    {
        try { release(); } catch (Exception ex) { Program.Log("Process cleanup: " + ex); }
        if (!armed) return;
        try { rollback(); } catch (Exception ex) { reportFailure(ex); }
    }
}
