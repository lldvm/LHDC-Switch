#include <windows.h>
#include <mmdeviceapi.h>
#include <audioclient.h>
#include <endpointvolume.h>
#include <ksmedia.h>
#include <wrl/client.h>
#include <cmath>
#include <cstdio>
#include <stdexcept>
using Microsoft::WRL::ComPtr;
static void ok(HRESULT h) { if (FAILED(h)) { char b[64]; sprintf_s(b,"HRESULT 0x%08lx",(unsigned long)h); throw std::runtime_error(b); } }
struct MuteGuard {
    ComPtr<IAudioEndpointVolume> vol; BOOL before = FALSE; bool armed = false;
    ~MuteGuard() { if(armed) vol->SetMute(before,nullptr); }
};
int wmain(int argc, wchar_t** argv) {
    CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    try {
        ComPtr<IMMDeviceEnumerator> e; ComPtr<IMMDevice> d;
        ok(CoCreateInstance(__uuidof(MMDeviceEnumerator),nullptr,CLSCTX_ALL,IID_PPV_ARGS(&e)));
        if(argc>1) ok(e->GetDevice(argv[1],&d));
        else ok(e->GetDefaultAudioEndpoint(eRender,eConsole,&d));
        MuteGuard guard; ok(d->Activate(__uuidof(IAudioEndpointVolume),CLSCTX_ALL,nullptr,(void**)&guard.vol));
        ok(guard.vol->GetMute(&guard.before));
        DWORD hw=0; ok(guard.vol->QueryHardwareSupport(&hw));
        ComPtr<IAudioClient> rc,cc;
        ok(d->Activate(__uuidof(IAudioClient),CLSCTX_ALL,nullptr,(void**)&rc));
        ok(d->Activate(__uuidof(IAudioClient),CLSCTX_ALL,nullptr,(void**)&cc));
        WAVEFORMATEX* fmt=nullptr; ok(rc->GetMixFormat(&fmt));
        bool floating = fmt->wFormatTag==WAVE_FORMAT_IEEE_FLOAT ||
            (fmt->wFormatTag==WAVE_FORMAT_EXTENSIBLE && IsEqualGUID(((WAVEFORMATEXTENSIBLE*)fmt)->SubFormat,KSDATAFORMAT_SUBTYPE_IEEE_FLOAT));
        if(!floating || fmt->wBitsPerSample!=32) { CoTaskMemFree(fmt); throw std::runtime_error("probe requires float32 endpoint"); }
        ok(rc->Initialize(AUDCLNT_SHAREMODE_SHARED,0,1000000,0,fmt,nullptr));
        ok(cc->Initialize(AUDCLNT_SHAREMODE_SHARED,AUDCLNT_STREAMFLAGS_LOOPBACK,1000000,0,fmt,nullptr));
        ComPtr<IAudioRenderClient> render; ComPtr<IAudioCaptureClient> capture;
        ok(rc->GetService(IID_PPV_ARGS(&render))); ok(cc->GetService(IID_PPV_ARGS(&capture)));
        UINT32 buffer=0; ok(rc->GetBufferSize(&buffer));
        ok(cc->Start()); ok(rc->Start());
        guard.armed=true;
        double rms[2]={}; unsigned long long frames[2]={}; unsigned long long pos=0;
        for(int stage=0; stage<2; stage++) {
            ok(guard.vol->SetMute(stage==1,nullptr));
            ULONGLONG began=GetTickCount64(); double sum=0; unsigned long long samples=0;
            while(GetTickCount64()-began<1200) {
                UINT32 padding=0; ok(rc->GetCurrentPadding(&padding)); UINT32 n=buffer-padding;
                if(n) { BYTE* data=nullptr; ok(render->GetBuffer(n,&data)); float* fp=(float*)data;
                    for(UINT32 i=0;i<n;i++,pos++) for(UINT32 ch=0;ch<fmt->nChannels;ch++) *fp++=(float)(0.004*sin(6.283185307179586*440*pos/fmt->nSamplesPerSec));
                    ok(render->ReleaseBuffer(n,0)); }
                UINT32 packet=0; ok(capture->GetNextPacketSize(&packet));
                while(packet) {
                    BYTE* data=nullptr; UINT32 count=0; DWORD flags=0;
                    ok(capture->GetBuffer(&data,&count,&flags,nullptr,nullptr));
                    if(GetTickCount64()-began>250) {
                        for(UINT32 i=0;i<count*fmt->nChannels;i++) { double v=(flags&AUDCLNT_BUFFERFLAGS_SILENT)?0:((float*)data)[i]; sum+=v*v; samples++; }
                        frames[stage]+=count;
                    }
                    ok(capture->ReleaseBuffer(count)); ok(capture->GetNextPacketSize(&packet));
                }
                Sleep(5);
            }
            rms[stage]=samples?sqrt(sum/samples):0;
        }
        ok(rc->Stop()); ok(cc->Stop()); CoTaskMemFree(fmt);
        ok(guard.vol->SetMute(guard.before,nullptr)); guard.armed=false;
        printf("{\"unmuted_rms\":%.10f,\"muted_rms\":%.10f,\"ratio\":%.6f,\"hardware_support\":%lu,\"original_mute\":%s,\"mute_restored\":true,\"frames_before\":%llu,\"frames_after\":%llu}\n",
            rms[0],rms[1],rms[0]>0?rms[1]/rms[0]:0,hw,guard.before?"true":"false",frames[0],frames[1]);
        return 0;
    } catch(const std::exception& ex) { printf("{\"error\":\"%s\"}\n",ex.what()); return 1; }
}
