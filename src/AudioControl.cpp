// Core Audio routing helper. Snapshot/list/defaults are read-only.
#include <windows.h>
#include <mmdeviceapi.h>
#include <endpointvolume.h>
#include <functiondiscoverykeys_devpkey.h>
#include <wrl/client.h>
#include <cstdio>
#include <stdexcept>
#include <string>
using Microsoft::WRL::ComPtr;
static void checked(HRESULT h) {
    if(FAILED(h)) { char s[64]; sprintf_s(s,"CoreAudio HRESULT 0x%08lx",(unsigned long)h); throw std::runtime_error(s); }
}
// Same Windows 7+ IPolicyConfig ABI used by A2DPWB audio_policy.rs.
struct __declspec(uuid("f8679f50-850a-41cf-9c72-430f290290c8")) IPolicyConfig: IUnknown {
    virtual HRESULT STDMETHODCALLTYPE GetMixFormat(LPCWSTR,void**)=0;
    virtual HRESULT STDMETHODCALLTYPE GetDeviceFormat(LPCWSTR,INT,void**)=0;
    virtual HRESULT STDMETHODCALLTYPE ResetDeviceFormat(LPCWSTR)=0;
    virtual HRESULT STDMETHODCALLTYPE SetDeviceFormat(LPCWSTR,void*,void*)=0;
    virtual HRESULT STDMETHODCALLTYPE GetProcessingPeriod(LPCWSTR,INT,INT64*,INT64*)=0;
    virtual HRESULT STDMETHODCALLTYPE SetProcessingPeriod(LPCWSTR,INT64*)=0;
    virtual HRESULT STDMETHODCALLTYPE GetShareMode(LPCWSTR,void*)=0;
    virtual HRESULT STDMETHODCALLTYPE SetShareMode(LPCWSTR,void*)=0;
    virtual HRESULT STDMETHODCALLTYPE GetPropertyValue(LPCWSTR,BOOL,const PROPERTYKEY&,PROPVARIANT*)=0;
    virtual HRESULT STDMETHODCALLTYPE SetPropertyValue(LPCWSTR,BOOL,const PROPERTYKEY&,const PROPVARIANT*)=0;
    virtual HRESULT STDMETHODCALLTYPE SetDefaultEndpoint(LPCWSTR,ERole)=0;
    virtual HRESULT STDMETHODCALLTYPE SetEndpointVisibility(LPCWSTR,BOOL)=0;
};
static const CLSID PolicyClass={0x870af99c,0x171d,0x4f9e,{0xaf,0x0d,0xe6,0x3d,0xf4,0x0c,0x2b,0xc9}};
static std::string jsonString(const std::wstring& text) {
    std::string out="\"";
    for(wchar_t c:text) { char escaped[8]; sprintf_s(escaped,"\\u%04x",(unsigned int)c); out+=escaped; }
    return out+"\"";
}
static std::wstring deviceId(IMMDevice* d) {
    LPWSTR wide=nullptr; checked(d->GetId(&wide)); std::wstring id(wide); CoTaskMemFree(wide); return id;
}
static void snapshot(IMMDevice* d) {
    DWORD state=0; checked(d->GetState(&state));
    ComPtr<IPropertyStore> props; checked(d->OpenPropertyStore(STGM_READ,&props));
    PROPVARIANT friendly{}; checked(props->GetValue(PKEY_Device_FriendlyName,&friendly));
    std::wstring name=(friendly.vt==VT_LPWSTR&&friendly.pwszVal)?friendly.pwszVal:L"";
    PropVariantClear(&friendly);
    BOOL muted=FALSE; float level=0; DWORD hardware=0; ComPtr<IAudioEndpointVolume> volume;
    if(SUCCEEDED(d->Activate(__uuidof(IAudioEndpointVolume),CLSCTX_ALL,nullptr,(void**)&volume))) {
        volume->GetMute(&muted); volume->GetMasterVolumeLevelScalar(&level); volume->QueryHardwareSupport(&hardware);
    }
    printf("{\"id\":%s,\"name\":%s,\"state\":%lu,\"muted\":%s,\"level\":%.9f,\"hardware_support\":%lu}",
        jsonString(deviceId(d)).c_str(),jsonString(name).c_str(),state,muted?"true":"false",level,hardware);
}
int wmain(int argc,wchar_t** argv) {
    CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED);
    try {
        if(argc<2) throw std::runtime_error("Usage: AudioControl snapshot [id] | list | defaults | set-default id role");
        ComPtr<IMMDeviceEnumerator> e; checked(CoCreateInstance(__uuidof(MMDeviceEnumerator),nullptr,CLSCTX_ALL,IID_PPV_ARGS(&e)));
        std::wstring command=argv[1];
        if(command==L"list") {
            ComPtr<IMMDeviceCollection> devices; checked(e->EnumAudioEndpoints(eRender,DEVICE_STATE_ACTIVE,&devices));
            UINT count=0; checked(devices->GetCount(&count)); printf("{\"devices\":[");
            for(UINT i=0;i<count;i++) { ComPtr<IMMDevice> d; checked(devices->Item(i,&d)); if(i) printf(","); snapshot(d.Get()); }
            printf("]}\n"); return 0;
        }
        if(command==L"defaults") {
            printf("{");
            for(int role=0;role<3;role++) {
                ComPtr<IMMDevice> d; HRESULT hr=e->GetDefaultAudioEndpoint(eRender,(ERole)role,&d);
                if(role) printf(","); printf("\"%d\":%s",role,SUCCEEDED(hr)?jsonString(deviceId(d.Get())).c_str():"null");
            }
            printf("}\n"); return 0;
        }
        ComPtr<IMMDevice> d;
        if(command==L"snapshot") {
            if(argc>=3) checked(e->GetDevice(argv[2],&d)); else checked(e->GetDefaultAudioEndpoint(eRender,eConsole,&d));
            snapshot(d.Get()); printf("\n"); return 0;
        }
        if(command!=L"set-default"||argc!=4) throw std::runtime_error("Unknown command or invalid arguments");
        wchar_t* end=nullptr; long role=wcstol(argv[3],&end,10);
        if(!end||*end||role<0||role>2) throw std::runtime_error("Audio role must be 0, 1 or 2");
        checked(e->GetDevice(argv[2],&d)); DWORD state=0; checked(d->GetState(&state));
        if(state!=DEVICE_STATE_ACTIVE) throw std::runtime_error("Target output device is not active");
        ComPtr<IPolicyConfig> policy; checked(CoCreateInstance(PolicyClass,nullptr,CLSCTX_ALL,IID_PPV_ARGS(&policy)));
        checked(policy->SetDefaultEndpoint(argv[2],(ERole)role));
        ComPtr<IMMDevice> actual; checked(e->GetDefaultAudioEndpoint(eRender,(ERole)role,&actual));
        if(deviceId(actual.Get())!=argv[2]) throw std::runtime_error("Default endpoint verification failed");
        printf("{\"ok\":true,\"role\":%ld,\"id\":%s}\n",role,jsonString(deviceId(actual.Get())).c_str()); return 0;
    } catch(const std::exception& ex) { printf("{\"error\":\"%s\"}\n",ex.what()); return 1; }
}
