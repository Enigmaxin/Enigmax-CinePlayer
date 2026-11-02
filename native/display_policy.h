/* Optional refresh switching. Only advertised native-resolution modes.
 * DEVMODE is a candidate hint, never evidence of a fractional rate.
 * Confirm the physical rational timing with CCD; rollback mismatches. */
#include "refresh_math.h"
static double cine_display_hz(const WCHAR *device) {
    for(int attempt=0;attempt<3;attempt++) {
        UINT32 np=0,nm=0;
        if(GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS,&np,&nm)!=ERROR_SUCCESS||!np||np>4096||!nm||nm>16384)return 0;
        DISPLAYCONFIG_PATH_INFO *paths=calloc(np,sizeof(*paths));
        DISPLAYCONFIG_MODE_INFO *modes=calloc(nm,sizeof(*modes));
        if(!paths||!modes){free(paths);free(modes);return 0;}
        LONG result=QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS,&np,paths,&nm,modes,NULL);
        if(result==ERROR_INSUFFICIENT_BUFFER){free(paths);free(modes);continue;}
        double hz=0;int invalid=0;
        if(result==ERROR_SUCCESS)for(UINT32 i=0;i<np;i++) {
            DISPLAYCONFIG_SOURCE_DEVICE_NAME source;ZeroMemory(&source,sizeof(source));
            source.header.type=DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;source.header.size=sizeof(source);
            source.header.adapterId=paths[i].sourceInfo.adapterId;source.header.id=paths[i].sourceInfo.id;
            if(DisplayConfigGetDeviceInfo(&source.header)!=ERROR_SUCCESS||lstrcmpW(source.viewGdiDeviceName,device))continue;
            UINT32 index=paths[i].targetInfo.modeInfoIdx;
            if(index>=nm||modes[index].infoType!=DISPLAYCONFIG_MODE_INFO_TYPE_TARGET){invalid=1;break;}
            DISPLAYCONFIG_RATIONAL rate=modes[index].targetMode.targetVideoSignalInfo.vSyncFreq;
            if(!rate.Denominator){invalid=1;break;}
            double actual=(double)rate.Numerator/rate.Denominator;
            if(hz&&!cine_rate_matches(hz,actual)){invalid=1;break;} /* cloned targets differ */
            hz=actual;
        }
        free(paths);free(modes);return invalid?0:hz;
    }
    return 0;
}
typedef struct { WCHAR device[32]; DEVMODEW original, applied; double original_hz, applied_hz; int changed; HANDLE token; } CineDisplay;
static int cine_same_mode(const DEVMODEW *a,const DEVMODEW *b) {
    return a->dmPelsWidth==b->dmPelsWidth && a->dmPelsHeight==b->dmPelsHeight &&
        a->dmBitsPerPel==b->dmBitsPerPel && a->dmDisplayFrequency==b->dmDisplayFrequency &&
        a->dmDisplayOrientation==b->dmDisplayOrientation && a->dmDisplayFlags==b->dmDisplayFlags;
}
static int cine_monitor_name(uintptr_t hwnd,WCHAR *name) {
    if(!hwnd || !IsWindow((HWND)hwnd))return 0;
    MONITORINFOEXW info;ZeroMemory(&info,sizeof(info));info.cbSize=sizeof(info);
    if(!GetMonitorInfoW(MonitorFromWindow((HWND)hwnd,MONITOR_DEFAULTTONEAREST),(MONITORINFO*)&info))return 0;
    lstrcpynW(name,info.szDevice,32);return 1;
}
int cine_display_identity(uintptr_t hwnd,WCHAR *out,int cap) {
    WCHAR name[32];if(!out||cap<2||!cine_monitor_name(hwnd,name))return 0;
    DISPLAY_DEVICEW d;ZeroMemory(&d,sizeof(d));d.cb=sizeof(d);
    if(EnumDisplayDevicesW(name,0,&d,0) && d.DeviceID[0])lstrcpynW(out,d.DeviceID,cap);
    else lstrcpynW(out,name,cap);
    return lstrlenW(out);
}
void *cine_display_create(void) { return calloc(1,sizeof(CineDisplay)); }
static int cine_display_restore(CineDisplay *d) {
    int result=0;
    if(d->changed){
        DEVMODEW current;ZeroMemory(&current,sizeof(current));current.dmSize=sizeof(current);
        /* Do not overwrite a mode the user or another application changed. */
        if(EnumDisplaySettingsExW(d->device,ENUM_CURRENT_SETTINGS,&current,0) && cine_same_mode(&current,&d->applied) && cine_rate_matches(cine_display_hz(d->device),d->applied_hz)) {
            if(ChangeDisplaySettingsExW(d->device,&d->original,NULL,0,NULL)!=DISP_CHANGE_SUCCESSFUL ||
               !cine_rate_matches(cine_display_hz(d->device),d->original_hz))result=-5;
        }
    }
    d->changed=0;
    if(d->token){ReleaseMutex(d->token);CloseHandle(d->token);d->token=NULL;}
    return result;
}
int cine_display_update(void *context,uintptr_t hwnd,int enabled,double fps) {
    CineDisplay *d=context;if(!d)return -1;
    WCHAR device[32];if(!enabled || !cine_monitor_name(hwnd,device) || !isfinite(fps) || fps<10 || fps>240) {return cine_display_restore(d);}
    if(d->changed && lstrcmpW(device,d->device) && cine_display_restore(d)<0)return -5;
    DEVMODEW current;ZeroMemory(&current,sizeof(current));current.dmSize=sizeof(current);
    if(!EnumDisplaySettingsExW(device,ENUM_CURRENT_SETTINGS,&current,0))return -1;
    double current_hz=cine_display_hz(device);
    if(!current_hz)return 1; /* Cannot verify: do not guess or switch blindly. */
    fps=cine_nominal_fps(fps);
    DEVMODEW candidates[256];unsigned count=0;
    for(DWORD i=0;i<4096;i++) {
        DEVMODEW mode;ZeroMemory(&mode,sizeof(mode));mode.dmSize=sizeof(mode);
        if(!EnumDisplaySettingsExW(device,i,&mode,0))break;
        if(mode.dmPelsWidth!=current.dmPelsWidth||mode.dmPelsHeight!=current.dmPelsHeight||mode.dmBitsPerPel!=current.dmBitsPerPel||mode.dmDisplayFlags!=current.dmDisplayFlags||mode.dmDisplayOrientation!=current.dmDisplayOrientation)continue;
        unsigned j;for(j=0;j<count;j++)if(cine_same_mode(&mode,&candidates[j]))break;
        if(j==count&&count<256)candidates[count++]=mode;
    }
    if(!d->token){
        WCHAR name[96]=L"Local\\EnigmaxCinePlayer.Refresh.";int n=lstrlenW(name);
        for(int i=0;device[i]&&i<31;i++){WCHAR c=device[i];name[n++]=((c>='0'&&c<='9')||(c>='A'&&c<='Z')||(c>='a'&&c<='z'))?c:'_';}name[n]=0;
        HANDLE token=CreateMutexW(NULL,FALSE,name);if(!token)return 3;
        DWORD result=WaitForSingleObject(token,0);if(result!=WAIT_OBJECT_0&&result!=WAIT_ABANDONED){CloseHandle(token);return 3;}d->token=token;
    }
    for(unsigned multiple=1;multiple<=24&&multiple*fps<=240.001;multiple++) {
        double wanted=multiple*fps;
        if(cine_rate_matches(current_hz,wanted)){if(!d->changed)cine_display_restore(d);return 2;}
        /* Prefer fractional legacy aliases (23/47/59/119) to rounded aliases.
         * Both must pass CCD verification; 24 cannot stand in for 23.976. */
        for(unsigned pass=0;pass<2;pass++)for(unsigned i=0;i<count;i++) {
            DEVMODEW *best=&candidates[i];unsigned hz=best->dmDisplayFrequency;
            if(!cine_rate_candidate(hz,wanted)||(pass==0)!=(hz==(unsigned)floor(wanted)))continue;
            if(ChangeDisplaySettingsExW(device,best,NULL,CDS_TEST,NULL)!=DISP_CHANGE_SUCCESSFUL)continue;
            if(ChangeDisplaySettingsExW(device,best,NULL,0,NULL)!=DISP_CHANGE_SUCCESSFUL)continue;
            DEVMODEW actual;ZeroMemory(&actual,sizeof(actual));actual.dmSize=sizeof(actual);
            double actual_hz=cine_display_hz(device);
            int valid=EnumDisplaySettingsExW(device,ENUM_CURRENT_SETTINGS,&actual,0)&&
                actual.dmPelsWidth==current.dmPelsWidth&&actual.dmPelsHeight==current.dmPelsHeight&&
                actual.dmBitsPerPel==current.dmBitsPerPel&&actual.dmDisplayOrientation==current.dmDisplayOrientation&&
                actual.dmDisplayFlags==current.dmDisplayFlags&&cine_rate_matches(actual_hz,wanted);
            if(!valid) {
                if(ChangeDisplaySettingsExW(device,&current,NULL,0,NULL)!=DISP_CHANGE_SUCCESSFUL||
                   !cine_rate_matches(cine_display_hz(device),current_hz))return -4;
                continue;
            }
            if(!d->changed){d->original=current;d->original_hz=current_hz;lstrcpynW(d->device,device,32);}
            d->applied=actual;d->applied_hz=actual_hz;d->changed=1;return 2;
        }
    }
    int restored=cine_display_restore(d);
    return restored<0?restored:1;
}
void cine_display_destroy(void *context) {CineDisplay *d=context;if(d){cine_display_restore(d);free(d);}}

double cine_display_rate(void *context,uintptr_t hwnd) {
    (void)context;WCHAR device[32];return cine_monitor_name(hwnd,device)?cine_display_hz(device):0;
}
