using System;
using System.IO;
using System.Linq;
using System.Drawing;
using System.Collections.Generic;
using System.Threading.Tasks;
using System.Windows.Forms;

internal static class EarbudCommands
{
    internal static readonly string[] All={"inspect","enable-lhdc-hires","hires-off","spatial-on","spatial-off","game-on","game-off","dual-on","dual-off",
        "anc-off","anc-transparency","anc-on","anc-light","anc-medium","anc-deep","anc-smart","anc-adaptive"};
    internal static readonly string[] Anc={"anc-off","anc-transparency","anc-on","anc-light","anc-medium","anc-deep","anc-smart","anc-adaptive"};
    static readonly int[] Slots={0,2,1,6,5,4,7,11};
    internal static int Read(Dictionary<string,object> d,string key) {int n;return Int32.TryParse(Program.Get(d,key),out n)?n:-1;}
    internal static bool Matches(string command,Dictionary<string,object> d)
    {
        if(command.StartsWith("anc-")) {
            int i=Array.IndexOf(Anc,command),m=Read(d,"anc_mask"); if(i<0||m<=0)return false;
            int slot=Slots[i]; if(slot==0)return (m&9)!=0;if(slot==2)return (m&260)!=0;
            if(slot==1)return (m&242)!=0;return (m&(1<<slot))!=0;
        }
        string key=command=="enable-lhdc-hires"||command=="hires-off"?"high_res":command.StartsWith("spatial-")?"spatial":command.StartsWith("game-")?"game_mode":"dual_connection";
        int want=command.EndsWith("-off")?0:1; return Read(d,key)==want;
    }
    internal static string AncName(int m)
    {
        if(m<=0)return "未知";
        if((m&9)!=0)return "关闭";if((m&260)!=0)return "通透";if((m&2048)!=0)return "自适应";
        if((m&128)!=0)return "智能降噪";if((m&16)!=0)return "深度降噪";if((m&32)!=0)return "中度降噪";if((m&64)!=0)return "轻度降噪";
        return (m&2)!=0?"降噪（上次强度）":"未知模式 "+m;
    }
}

internal sealed partial class MainForm
{
    void ShowEarbudPanel()
    {
        using(var dialog=new Form())
        {
            dialog.SuspendLayout();dialog.AutoScaleDimensions=new SizeF(96,96);dialog.AutoScaleMode=AutoScaleMode.Dpi;
            dialog.Font=Font;dialog.Text="耳机功能面板 · Enco Free4";dialog.ClientSize=new Size(720,595);dialog.BackColor=Card;dialog.ForeColor=Color.White;
            dialog.StartPosition=FormStartPosition.CenterParent;dialog.MaximizeBox=false;dialog.MinimizeBox=false;
            LabelAt(dialog,"耳机功能",24,18,450,35,21,Color.White,true);
            var summary=LabelAt(dialog,"先读取耳机当前设置；此面板适用于 Enco Free4 / 丹拿版。",24,65,670,46,10,Muted);
            var read=ButtonAt(dialog,"读取当前设置",24,117,220,38,Accent,Bg);
            var release=ButtonAt(dialog,"断开 LHDC 后读取",260,117,432,38,Color.FromArgb(33,48,66),Color.White);
            string[] keys={"high_res","spatial","game_mode","dual_connection"};
            string[] titles={"高清音频 · LHDC","空间音频","游戏低延迟模式","双设备连接"};
            var values=new Label[4];var switches=new Button[4];
            for(int i=0;i<4;i++) {LabelAt(dialog,titles[i],26,180+i*49,270,30,11,Color.White,true);values[i]=LabelAt(dialog,"未读取",325,180+i*49,150,30,10,Muted);switches[i]=ButtonAt(dialog,"—",500,174+i*49,192,35,Color.FromArgb(33,48,66),Color.White);}
            LabelAt(dialog,"降噪 / 通透",26,386,275,30,11,Color.White,true);
            var anc=new DarkComboBox();SetupCombo(anc,dialog,262,382,236);anc.Items.AddRange(new object[]{"关闭","通透","降噪（上次强度）","轻度降噪","中度降噪","深度降噪","智能降噪","自适应"});anc.SelectedIndex=-1;
            var apply=ButtonAt(dialog,"应用降噪模式",514,381,178,36,Color.FromArgb(33,48,66),Color.White);
            var ancCurrent=LabelAt(dialog,"当前降噪：未读取",26,427,665,26,10,Muted);
            var message=LabelAt(dialog,"高清与空间音频可能互斥；修改后会重新读取确认。\r\n设置需要 Windows 管理蓝牙，完成后可回到主界面连接 LHDC。",26,470,665,100,10,Muted);
            Dictionary<string,object> report=null;bool working=false,fresh=false;
            Action redraw=()=>{
                bool running=Program.SessionRunning(); string product=Program.Get(report,"product_id");bool known=product=="06C010"||product=="068C10";
                read.Enabled=!working&&!running;release.Enabled=!working&&running;
                if(running&&!working) message.Text="LHDC 正在接管蓝牙。请先断开并恢复 Windows 蓝牙，才能读取或修改耳机设置。\r\n此操作会停止播放；完成设置后再手动连接 LHDC。";
                for(int i=0;i<4;i++) {
                    int state=EarbudCommands.Read(report,keys[i]);values[i].Text=state==0?"关闭":state==1?"开启":"未读取 / 不支持";
                    switches[i].Text=state==1?"关闭":"开启";
                    bool conflict=(i==0&&EarbudCommands.Read(report,"spatial")==1)||(i==1&&EarbudCommands.Read(report,"high_res")==1);
                    switches[i].Enabled=!working&&!running&&fresh&&known&&Program.Bool(report,"feature_write_supported")&&(state==0||state==1)&&!(state==0&&conflict)&&!(i==1&&Program.Bool(report,"spatial_type_variant"));
                }
                anc.Enabled=!working&&!running&&fresh&&known&&Program.Bool(report,"anc_supported");apply.Enabled=anc.Enabled&&anc.SelectedIndex>=0;
                ancCurrent.Text="当前降噪："+EarbudCommands.AncName(EarbudCommands.Read(report,"anc_mask"));
                if(report!=null) {Func<string,string> battery=k=>EarbudCommands.Read(report,k)>0?EarbudCommands.Read(report,k)+"%":"—";
                    summary.Text="型号 "+product+"  ·  左耳 "+battery("battery_left")+"  右耳 "+battery("battery_right")+"  充电盒 "+battery("battery_case")+"\r\n"+(fresh?"当前面板已读取耳机；修改结果以回读为准。":"历史缓存；请先读取当前设置。");}
            };
            Func<string,Task> run=async command=>{
                if(working)return;working=true;redraw();message.Text=command=="inspect"?"正在读取耳机……":"正在发送设置命令……";
                try {
                    var result=await Task.Run(()=>Program.EarbudSettings(command));
                    if(command!="inspect") {
                        message.Text="耳机接受了命令；等待重连并验证实际状态……";
                        await Task.Delay(command=="enable-lhdc-hires"||command=="hires-off"?4500:1500);
                        result=await Task.Run(()=>Program.EarbudSettings("inspect"));
                    }
                    report=result;fresh=true;
                    message.Text=command=="inspect"?"已读取。未报告的功能显示为不支持；高清与空间音频互斥时请先关闭另一项。":EarbudCommands.Matches(command,result)?"已回读确认设置生效。":"耳机接受了命令，但回读尚未确认；请稍后重新读取，不要反复切换。";
                } catch(Exception ex) {fresh=false;message.Text=ex.Message+"\r\n请确认耳机已连接 Windows，必要时重新读取。";}
                finally {working=false;redraw();}
            };
            for(int i=0;i<4;i++) {int row=i;switches[i].Click+=async(s,e)=>{bool on=EarbudCommands.Read(report,keys[row])!=1;
                string command=row==0?(on?"enable-lhdc-hires":"hires-off"):(row==1?"spatial-":row==2?"game-":"dual-")+(on?"on":"off");await run(command);};}
            read.Click+=async(s,e)=>await run("inspect");apply.Click+=async(s,e)=>{if(anc.SelectedIndex>=0)await run(EarbudCommands.Anc[anc.SelectedIndex]);};anc.SelectedIndexChanged+=(s,e)=>redraw();
            release.Click+=async(s,e)=>{
                if(working)return;working=true;redraw();message.Text="正在停止 LHDC 并恢复 Windows 蓝牙……";
                try {Program.RequestStop("earbud settings requested");for(int i=0;i<120&&Program.SessionRunning();i++)await Task.Delay(250);
                    if(Program.SessionRunning()||File.Exists(Program.Journal))throw new InvalidOperationException("蓝牙尚未恢复，请返回控制台完成恢复。");
                    working=false;await run("inspect");
                } catch(Exception ex) {working=false;message.Text=ex.Message;redraw();}
            };
            dialog.FormClosing+=(s,e)=>{if(working)e.Cancel=true;};
            try {report=Program.ReadObject(Path.Combine(Program.Data,"earbud-last-read.json"));}catch{}
            redraw();dialog.ResumeLayout(true);dialog.ShowDialog(this);
        }
    }
}
