using System;
using System.IO;
using System.IO.Pipes;
using System.Text;
using System.Collections.Generic;
using System.Collections.Concurrent;
using System.Diagnostics;
using System.Security.Principal;
using System.Threading;
using System.Threading.Tasks;
using System.Text.RegularExpressions;

// Documented A2DPWB protocol v1: u32 little-endian length + UTF-8 JSON.
// The elevated session owns this connection; the UI only reads snapshots.
internal sealed class EngineClient : IDisposable
{
    readonly NamedPipeClientStream pipe;
    readonly object writeLock = new object(), modelLock = new object();
    readonly ConcurrentDictionary<long, TaskCompletionSource<Dictionary<string, object>>> pending = new ConcurrentDictionary<long, TaskCompletionSource<Dictionary<string, object>>>();
    readonly TaskCompletionSource<bool> ready = new TaskCompletionSource<bool>();
    Dictionary<string, object> stream = new Dictionary<string, object>(), link = new Dictionary<string, object>();
    long serial; bool disposed;
    string linkTimestamp = "";
    int peerMaxKbps, adaptiveMaxKbps;
    public Action<string> LogReceived;
    internal string LastError = "";
    internal const string Instance = "lhdcswitch";
    internal EngineClient(string instance)
    {
        string sid = WindowsIdentity.GetCurrent().User.Value;
        pipe = new NamedPipeClientStream(".", "a2dpwb-" + sid + "-" + instance, PipeDirection.InOut, PipeOptions.Asynchronous);
    }
    internal void Connect(int milliseconds)
    {
        pipe.Connect(milliseconds);
        Write(new { type = "hello", protocol = 1, client = "LHDC-Switch/2" });
        Task.Run((Func<Task>)ReadLoop);
        if (!ready.Task.Wait(milliseconds)) throw new TimeoutException("实时接口连接超时。");
        ready.Task.GetAwaiter().GetResult();
    }
    internal static byte[] Frame(object value)
    {
        byte[] body = Encoding.UTF8.GetBytes(Program.Json.Serialize(value));
        if (body.Length > 4 * 1024 * 1024) throw new InvalidDataException("IPC frame exceeds 4 MiB");
        byte[] bytes = new byte[body.Length + 4];
        Buffer.BlockCopy(BitConverter.GetBytes(body.Length), 0, bytes, 0, 4); Buffer.BlockCopy(body, 0, bytes, 4, body.Length);
        return bytes;
    }
    void Write(object value)
    {
        byte[] bytes = Frame(value);
        lock (writeLock) { pipe.Write(bytes, 0, bytes.Length); pipe.Flush(); }
    }
    internal Dictionary<string, object> Call(string method, object parameters, int timeout = 10000)
    {
        long id = Interlocked.Increment(ref serial);
        var reply = new TaskCompletionSource<Dictionary<string, object>>(); pending[id] = reply;
        try
        {
            var message = new Dictionary<string, object> { { "type", "request" }, { "id", id }, { "method", method } };
            if (parameters != null) message["params"] = parameters;
            Write(message);
            if (!reply.Task.Wait(timeout)) throw new TimeoutException("播放接口无响应：" + method);
            var result = reply.Task.GetAwaiter().GetResult();
            var error = Program.Child(result, "error");
            if (error != null) throw new InvalidOperationException("引擎拒绝操作：" + Program.Get(error, "code") + " " + Program.Json.Serialize(Program.Child(error, "message")));
            return result;
        }
        finally { TaskCompletionSource<Dictionary<string, object>> ignored; pending.TryRemove(id, out ignored); }
    }
    async Task<byte[]> ReadBytes(int length)
    {
        byte[] buffer = new byte[length]; int read = 0;
        while (read < length) { int n = await pipe.ReadAsync(buffer, read, length - read).ConfigureAwait(false); if (n == 0) throw new EndOfStreamException("播放引擎已关闭实时接口。"); read += n; }
        return buffer;
    }
    async Task ReadLoop()
    {
        try
        {
            while (!disposed)
            {
                int length = BitConverter.ToInt32(await ReadBytes(4).ConfigureAwait(false), 0);
                if (length < 1 || length > 4 * 1024 * 1024) throw new InvalidDataException("无效的实时数据帧。");
                var value = Program.Json.Deserialize<Dictionary<string, object>>(Encoding.UTF8.GetString(await ReadBytes(length).ConfigureAwait(false)));
                string type = Program.Get(value, "type");
                if (type == "reject") throw new InvalidOperationException(Program.Get(value, "reason"));
                if (type == "snapshot")
                {
                    var state = Program.Child(value, "state");
                    lock (modelLock) stream = Program.Child(state, "stream") ?? new Dictionary<string, object>();
                    ready.TrySetResult(true);
                }
                else if (type == "response")
                {
                    TaskCompletionSource<Dictionary<string, object>> reply;
                    if (pending.TryGetValue(Convert.ToInt64(value["id"]), out reply)) reply.TrySetResult(value);
                }
                else if (type == "event")
                {
                    var ev = Program.Child(value, "event"); string kind = Program.Get(ev, "kind");
                    var body = Program.Child(ev, "value");
                    if (kind == "stream") lock (modelLock) stream = body ?? new Dictionary<string, object>();
                    if (kind == "link") lock (modelLock) { link = body ?? new Dictionary<string, object>(); linkTimestamp = DateTime.UtcNow.ToString("o"); }
                    if (kind == "log")
                    {
                        string text = Program.Get(body, "message");
                        var adaptive = Regex.Match(text, @"adaptive ceiling (\d+) kbps");
                        if (adaptive.Success) lock (modelLock) adaptiveMaxKbps = Int32.Parse(adaptive.Groups[1].Value);
                        var match = Regex.Match(text, @"peer limits idx \d+\.\.(\d+)");
                        if (match.Success) { int index; if (Int32.TryParse(match.Groups[1].Value, out index) && index >= 0 && index <= 8) lock (modelLock) peerMaxKbps = new[] { 64, 160, 192, 256, 320, 400, 500, 900, 1000 }[index]; }
                        if (LogReceived != null) LogReceived(Program.Get(body, "level") + " " + text);
                    }
                }
            }
        }
        catch (Exception ex)
        {
            if (!disposed) LastError = ex.Message;
            ready.TrySetException(ex);
            foreach (var entry in pending) entry.Value.TrySetException(ex);
        }
    }
    internal Dictionary<string, object> Telemetry()
    {
        lock (modelLock)
            return new Dictionary<string, object> { { "timestamp", DateTime.UtcNow.ToString("o") }, { "link_timestamp", linkTimestamp }, { "peer_max_kbps", peerMaxKbps }, { "adaptive_max_kbps", adaptiveMaxKbps }, { "active", !disposed }, { "stream", stream }, { "link", link }, { "error", LastError } };
    }
    public void Dispose() { disposed = true; pipe.Dispose(); }
}

internal static class LiveData
{
    internal static double Number(Dictionary<string, object> d, string key)
    { object value; try { return d != null && d.TryGetValue(key, out value) && value != null ? Convert.ToDouble(value) : 0; } catch { return 0; } }
    internal static string Health(Dictionary<string, object> link, Dictionary<string, object> previous)
    {
        if (link == null || link.Count == 0) return "等待链路采样；当前尚无卡顿诊断数据。";
        if(previous!=null && Number(link,"capture_discontinuities")>Number(previous,"capture_discontinuities")) return "Windows 音源出现采集不连续；这类停顿不会计为发送丢包。";
        if(Number(link,"queue_wait_us")>=60000 || Number(link,"queue_audio_us")>=60000) return "主机发送拥塞：最老包等待 "+(Number(link,"queue_wait_us")/1000).ToString("0.0")+" ms，积压音频 "+(Number(link,"queue_audio_us")/1000).ToString("0.0")+" ms。";
        if(previous!=null && Number(link,"pacing_rebases")>Number(previous,"pacing_rebases")) return "媒体时钟已在长停顿后重新对齐；已限制集中补发，请结合采集和无线指标判断。";
        if(previous!=null && Number(link,"send_gaps_30ms")>Number(previous,"send_gaps_30ms")) return "检测到超过 30 ms 的主机发送间隔；请结合音源和无线指标判断。";
        if (previous != null && Number(link, "capture_dropped") > Number(previous, "capture_dropped")) return "采集丢帧正在增加：优先检查声卡格式、音频增强及 CPU 调度。";
        if ((previous != null && Number(link, "dropped") > Number(previous, "dropped")) || Number(link, "queue_depth") > Math.Max(8, Number(link, "queue_capacity") * .75))
            return "发送队列出现拥塞：可开启自动码率，检查耳机距离及无线干扰。";
        return "当前未发现本机新增丢包。无线端丢包不能仅凭发送计数排除。";
    }
}

internal sealed class VolumePresentation
{
    internal int Confirmed=-1, Pending=-1; int baseline=-1; DateTime sentAt;
    internal bool TimedOut;
    internal int Display {get {return Pending>=0 ? Pending : Math.Max(0,Confirmed);} }
    internal void Request(int value,DateTime now) {Pending=Math.Max(0,Math.Min(127,value));baseline=Confirmed;sentAt=now;TimedOut=false;}
    internal void Expire(DateTime now) {if(Pending>=0&&(now-sentAt).TotalMilliseconds>1800){Pending=-1;TimedOut=true;}}
    internal void Observe(int value,DateTime now)
    {
        bool changed=value!=Confirmed; Confirmed=value;
        if(Pending>=0 && (value==Pending || value!=baseline)) {Pending=-1;TimedOut=false;}
        if(Pending>=0 && (now-sentAt).TotalMilliseconds>1800) {Expire(now);}
        else if(Pending<0 && changed) TimedOut=false;
    }
}
