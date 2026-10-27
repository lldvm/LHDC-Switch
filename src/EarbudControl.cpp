// Independent RFCOMM implementation from public HeyMelody protocol notes.
// Documented, model-gated HeyMelody feature and noise-control commands.
#define NOMINMAX
#include <winsock2.h>
#include <ws2bth.h>
#include <windows.h>
#include <objbase.h>
#include <vector>
#include <string>
#include <cstdio>
#include <stdexcept>
#include <algorithm>
static std::string hex(const std::vector<unsigned char>& data) { std::string out; for(auto b:data){char s[4];sprintf_s(s,"%02X",b);out+=s;} return out; }
static std::vector<unsigned char> packet(unsigned short cmd,unsigned char seq,const std::vector<unsigned char>& p) {
    std::vector<unsigned char> v={0xaa}; size_t n=7+p.size(); do { unsigned char b=(unsigned char)(n&127);n>>=7;if(n)b|=128;v.push_back(b); }while(n);
    v.insert(v.end(),{0,0,(unsigned char)cmd,(unsigned char)(cmd>>8),seq,(unsigned char)p.size(),(unsigned char)(p.size()>>8)});v.insert(v.end(),p.begin(),p.end());return v;
}
struct Frame {unsigned short cmd;unsigned char seq;std::vector<unsigned char> payload;};
struct Framer {
    std::vector<unsigned char> bytes,fragment;
    std::vector<Frame> push(const unsigned char* data,size_t size) {
        bytes.insert(bytes.end(),data,data+size);std::vector<Frame> result;
        while(!bytes.empty()) {
            if(bytes[0]!=0xaa){bytes.erase(bytes.begin());continue;}
            size_t pos=1,n=0;int shift=0;bool complete=false;
            while(pos<bytes.size()&&shift<=14){unsigned char b=bytes[pos++];n|=(size_t)(b&127)<<shift;shift+=7;if(!(b&128)){complete=true;break;}}
            if(!complete)break;
            if(n<2||n>4096){bytes.erase(bytes.begin());continue;}
            if(bytes.size()<pos+n)break;
            std::vector<unsigned char> body(bytes.begin()+pos,bytes.begin()+pos+n),message;
            bytes.erase(bytes.begin(),bytes.begin()+pos+n);
            if(body[0]==0)message.assign(body.begin()+2,body.end());
            else if(body.size()>=3){if(body[0]==1)fragment.clear();fragment.insert(fragment.end(),body.begin()+3,body.end());if(body[0]==3){message=fragment;fragment.clear();}}
            if(message.size()<5)continue;
            size_t declared=message[3]|((size_t)message[4]<<8),available=message.size()-5;
            if(declared>available)continue;
            result.push_back({(unsigned short)(message[0]|message[1]<<8),message[2],std::vector<unsigned char>(message.begin()+5,message.begin()+5+std::min(declared,available))});
        }return result;
    }
};
struct Link {
    SOCKET socket=INVALID_SOCKET;unsigned char sequence=0;Framer framer;
    ~Link(){if(socket!=INVALID_SOCKET)closesocket(socket);}
    bool connectTo(BTH_ADDR address,const wchar_t* uuid,unsigned long port) {
        socket=::socket(AF_BTH,SOCK_STREAM,BTHPROTO_RFCOMM);if(socket==INVALID_SOCKET)return false;
        SOCKADDR_BTH sa{};sa.addressFamily=AF_BTH;sa.btAddr=address;sa.port=port;
        if(uuid)CLSIDFromString(uuid,&sa.serviceClassId);
        unsigned long nonblock=1;ioctlsocket(socket,FIONBIO,&nonblock);
        int rc=connect(socket,(sockaddr*)&sa,sizeof(sa));
        if(rc==SOCKET_ERROR&&WSAGetLastError()!=WSAEWOULDBLOCK){closesocket(socket);socket=INVALID_SOCKET;return false;}
        if(rc!=0){fd_set writes,errors;FD_ZERO(&writes);FD_ZERO(&errors);FD_SET(socket,&writes);FD_SET(socket,&errors);timeval wait{5,0};
            if(select(0,nullptr,&writes,&errors,&wait)<=0||FD_ISSET(socket,&errors)){closesocket(socket);socket=INVALID_SOCKET;return false;}}
        int error=0,errorSize=sizeof(error);getsockopt(socket,SOL_SOCKET,SO_ERROR,(char*)&error,&errorSize);
        if(error){closesocket(socket);socket=INVALID_SOCKET;return false;}
        nonblock=0;ioctlsocket(socket,FIONBIO,&nonblock);DWORD ms=350;
        setsockopt(socket,SOL_SOCKET,SO_RCVTIMEO,(char*)&ms,sizeof(ms));ms=2000;setsockopt(socket,SOL_SOCKET,SO_SNDTIMEO,(char*)&ms,sizeof(ms));return true;
    }
    std::vector<unsigned char> request(unsigned short cmd,const std::vector<unsigned char>& payload={},DWORD timeout=3500) {
        unsigned char seq=++sequence;auto bytes=packet(cmd,seq,payload);size_t sent=0;
        while(sent<bytes.size()){int n=send(socket,(const char*)bytes.data()+sent,(int)(bytes.size()-sent),0);if(n<=0)throw std::runtime_error("RFCOMM send failed");sent+=n;}
        ULONGLONG until=GetTickCount64()+timeout;
        while(GetTickCount64()<until){unsigned char data[2048];int n=recv(socket,(char*)data,sizeof(data),0);
            if(n==0)throw std::runtime_error("Earbuds disconnected; setting may require reconnect");
            if(n<0){if(WSAGetLastError()==WSAETIMEDOUT)continue;throw std::runtime_error("RFCOMM receive failed");}
            for(auto& frame:framer.push(data,n))if(frame.cmd==(cmd|0x8000)&&frame.seq==seq)return frame.payload;
        }throw std::runtime_error("Earbuds did not answer the documented command");
    }
};
static bool bit(const std::vector<unsigned char>& p,int n){return p.size()>1+(size_t)n/8&&(p[1+n/8]&(1<<(n%8)))!=0;}
static int feature(const std::vector<unsigned char>& p,unsigned char id){if(p.size()<2||p[0]!=0)return -1;for(size_t n=0;n<p[1]&&3+n*2<p.size();n++)if(p[2+n*2]==id)return p[3+n*2];return -1;}
static int ancSlot(const std::wstring& c) {
    if(c==L"anc-off")return 0;if(c==L"anc-on")return 1;if(c==L"anc-transparency")return 2;
    if(c==L"anc-deep")return 4;if(c==L"anc-medium")return 5;if(c==L"anc-light")return 6;
    if(c==L"anc-smart")return 7;if(c==L"anc-adaptive")return 11;return -1;
}
static std::vector<unsigned char> ancPayload(int slot) {
    if(slot<0||slot>11)throw std::runtime_error("Invalid ANC slot");
    std::vector<unsigned char> p={1,1};for(int i=0;i<=slot/8;i++)p.push_back((unsigned char)((1u<<slot)>>(i*8)));return p;
}
static int ancMask(const std::vector<unsigned char>& p) {
    if(p.size()<4||p.size()>7||p[0]!=0||p[1]!=1||p[2]!=1)return -1;
    unsigned int result=0;for(size_t i=3;i<p.size();i++)result|=(unsigned int)p[i]<<((i-3)*8);return (int)result;
}
static int battery(const std::vector<unsigned char>& p,int part) {
    if(p.size()<2||p[0]!=0||p.size()<2+(size_t)p[1]*2)return -1;
    for(size_t i=0;i<p[1];i++)if(p[2+i*2]==part){int n=p[3+i*2]&127;return n>0&&n<=100?n:-1;}return -1;
}
int wmain(int argc,wchar_t** argv) {
    try {
        if(argc>=2&&wcscmp(argv[1],L"selftest")==0){auto v=packet(0x0100,1,{});if(hex(v)!="AA0700000001010000")throw std::runtime_error("packet test failed");Framer f;auto a=f.push(v.data(),3);auto b=f.push(v.data()+3,v.size()-3);if(!a.empty()||b.size()!=1||b[0].cmd!=0x0100)throw std::runtime_error("split frame test failed");if(hex(ancPayload(11))!="01010008"||hex(ancPayload(4))!="010110")throw std::runtime_error("ANC model payload test failed");if(ancMask({0,1,1,0,8})!=2048||ancMask({0,1,1})!=-1)throw std::runtime_error("ANC parse test failed");if(battery({0,2,1,0xD0,2,0},1)!=80||battery({0,2,1,0xD0,2,0},2)!=-1)throw std::runtime_error("battery test failed");printf("{\"passed\":true,\"tests\":5}\n");return 0;}
        if(argc!=3)throw std::runtime_error("Usage: EarbudControl command AA:BB:CC:DD:EE:FF");
        std::wstring command=argv[1];
        bool normal=command==L"inspect"||command==L"enable-hires"||command==L"enable-lhdc-hires";
        bool toggle=command==L"hires-off"||command==L"spatial-on"||command==L"spatial-off"||command==L"game-on"||command==L"game-off"||command==L"dual-on"||command==L"dual-off";
        int slot=ancSlot(command);if(!normal&&!toggle&&slot<0)throw std::runtime_error("Unsupported command");
        std::wstring address=argv[2];address.erase(std::remove(address.begin(),address.end(),L':'),address.end());
        if(address.size()!=12||address.find_first_not_of(L"0123456789aAbBcCdDeEfF")!=std::wstring::npos)throw std::runtime_error("Invalid Bluetooth address");
        BTH_ADDR addr=_wcstoui64(address.c_str(),nullptr,16);WSADATA wsa{};if(WSAStartup(MAKEWORD(2,2),&wsa))throw std::runtime_error("WSAStartup failed");
        Link link;bool connected=link.connectTo(addr,L"{0000079A-D102-11E1-9B23-00025B00A5A5}",0);
        if(!connected)connected=link.connectTo(addr,L"{00001107-D102-11E1-9B23-00025B00A5A5}",0);
        if(!connected)connected=link.connectTo(addr,nullptr,15);
        if(!connected)throw std::runtime_error("Cannot open HeyMelody RFCOMM service; use Windows Bluetooth and connect earbuds first");
        Sleep(300);auto caps=link.request(0x0100);Sleep(200);auto product=link.request(0x0103);Sleep(200);
        if(caps.empty()||caps[0]!=0||product.size()!=4||product[0]!=0)throw std::runtime_error("Invalid HeyMelody handshake or product ID");
        unsigned int productId=product[1]|product[2]<<8|product[3]<<16;
        auto status=link.request(0x010D,{8,5,4,0x0b,0x11,0x18,6,0x1b,0x1c});int hi=feature(status,0x18),spatial=feature(status,0x1b);
        int codec=-1;if(bit(caps,17)){Sleep(200);auto c=link.request(0x0114);if(c.size()>=2&&c[0]==0)codec=c[1];}
        if(toggle||slot>=0) {
            if(productId!=0x068c10&&productId!=0x06c010)throw std::runtime_error("Write refused: unverified model");
            unsigned short cmd=0x0403;std::vector<unsigned char> payload;
            if(slot>=0) {if(!bit(caps,8))throw std::runtime_error("Noise control is not advertised");cmd=0x0404;payload=ancPayload(slot);}
            else {
                unsigned char id=command==L"hires-off"?0x18:command.find(L"spatial-")==0?0x1b:command.find(L"game-")==0?6:0x11;
                unsigned char value=command.size()>=3&&command.substr(command.size()-3)==L"-on"?1:0;
                if(!bit(caps,7)||feature(status,id)<0)throw std::runtime_error("Feature switch is not advertised");
                if(id==0x1b&&bit(caps,47))throw std::runtime_error("Spatial type protocol variant is not supported by this helper");
                if(id==0x1b&&value==1&&hi==1)throw std::runtime_error("Disable Hi-Res before enabling spatial audio");
                payload={id,value};
            }
            auto ack=link.request(cmd,payload);
            if(ack.empty()||ack[0]!=0){printf("{\"ok\":false,\"refused_status\":%d,\"write_sent\":true}\n",ack.empty()?-1:ack[0]);return 2;}
            printf("{\"ok\":true,\"accepted\":true,\"verified\":false,\"write_sent\":true}\n");return 0;
        }
        bool selectLhdc=wcscmp(argv[1],L"enable-lhdc-hires")==0;
        bool write=wcscmp(argv[1],L"enable-hires")==0||selectLhdc;
        if(write){
            if(productId!=0x068c10&&productId!=0x06c010)throw std::runtime_error("Write refused: product is not verified Enco Free4 / Dynaudio");
            if(hi<0||!bit(caps,7))throw std::runtime_error("Write refused: Hi-Res feature is not advertised");
            if(hi==1){printf("{\"ok\":true,\"product_id\":\"%06X\",\"high_res\":1,\"already_enabled\":true,\"write_sent\":false,\"codec\":%d}\n",productId,codec);return 0;}
            if(spatial==1)throw std::runtime_error("Spatial audio is enabled; turn it off in HeyMelody before enabling Hi-Res");
            if(selectLhdc&&!bit(caps,37))throw std::runtime_error("Codec-select command is not advertised by this firmware");
            auto ack=selectLhdc?link.request(0x041a,{8,1,0}):link.request(0x0403,{0x18,1});
            if(ack.empty()||ack[0]!=0){printf("{\"ok\":false,\"refused_status\":%d,\"high_res\":%d,\"codec\":%d,\"write_sent\":true}\n",ack.empty()?-1:ack[0],hi,codec);return 2;}
            printf("{\"ok\":true,\"accepted\":true,\"verified\":false,\"reconnect_required\":true,\"write_sent\":true}\n");return 0;
        }
        int anc=-1;std::vector<unsigned char> bat;
        if(bit(caps,8)) {try {anc=ancMask(link.request(0x010c,{1,1}));}catch(const std::exception&) {}}
        try {bat=link.request(0x0106);}catch(const std::exception&) {}
        printf("{\"ok\":true,\"product_id\":\"%06X\",\"high_res\":%d,\"spatial\":%d,\"game_mode\":%d,\"dual_connection\":%d,\"codec\":%d,\"anc_mask\":%d,\"anc_supported\":%s,\"feature_write_supported\":%s,\"spatial_type_variant\":%s,\"battery_left\":%d,\"battery_right\":%d,\"battery_case\":%d,\"capabilities\":\"%s\",\"raw_status\":\"%s\",\"write_sent\":false}\n",productId,hi,spatial,feature(status,6),feature(status,0x11),codec,anc,bit(caps,8)?"true":"false",bit(caps,7)?"true":"false",bit(caps,47)?"true":"false",battery(bat,1),battery(bat,2),battery(bat,3),hex(caps).c_str(),hex(status).c_str());return 0;
    }catch(const std::exception& ex){printf("{\"ok\":false,\"error\":\"%s\"}\n",ex.what());return 1;}
}
