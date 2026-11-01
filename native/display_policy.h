/* Optional refresh switching. Never changes resolution, orientation or registry. */
typedef struct { WCHAR device[32]; DEVMODEW original, applied; int changed; HANDLE token; } CineDisplay;
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
static void cine_display_restore(CineDisplay *d) {
    if(d->changed){
        DEVMODEW current;ZeroMemory(&current,sizeof(current));current.dmSize=sizeof(current);
        /* Do not overwrite a mode the user or another application changed. */
        if(EnumDisplaySettingsExW(d->device,ENUM_CURRENT_SETTINGS,&current,0) && cine_same_mode(&current,&d->applied))
            ChangeDisplaySettingsExW(d->device,&d->original,NULL,0,NULL);
    }
    d->changed=0;
    if(d->token){ReleaseMutex(d->token);CloseHandle(d->token);d->token=NULL;}
}
int cine_display_update(void *context,uintptr_t hwnd,int enabled,double fps) {
    CineDisplay *d=context;if(!d)return -1;
    WCHAR device[32];if(!enabled || !cine_monitor_name(hwnd,device) || fps<10 || fps>240) {cine_display_restore(d);return 0;}
    if(d->changed && lstrcmpW(device,d->device))cine_display_restore(d);
    DEVMODEW current;ZeroMemory(&current,sizeof(current));current.dmSize=sizeof(current);
    if(!EnumDisplaySettingsExW(device,ENUM_CURRENT_SETTINGS,&current,0))return -1;
    DEVMODEW best=current;double score=1e6;
    for(DWORD i=0;i<4096;i++) {
        DEVMODEW mode;ZeroMemory(&mode,sizeof(mode));mode.dmSize=sizeof(mode);
        if(!EnumDisplaySettingsExW(device,i,&mode,0))break;
        if(mode.dmPelsWidth!=current.dmPelsWidth||mode.dmPelsHeight!=current.dmPelsHeight||mode.dmBitsPerPel!=current.dmBitsPerPel||mode.dmDisplayFlags!=current.dmDisplayFlags||mode.dmDisplayOrientation!=current.dmDisplayOrientation)continue;
        /* Windows DEVMODE rounds fractional frequencies (23/59/119). Do not
         * advertise exact 23.976 negotiation through this legacy API. */
        double hz=mode.dmDisplayFrequency;
        if(hz==23)hz=24000.0/1001.0;else if(hz==59)hz=60000.0/1001.0;else if(hz==119)hz=120000.0/1001.0;
        double multiple=floor(hz/fps+0.5);if(multiple<1 || fabs(hz-multiple*fps)>0.08)continue;
        double rank=fabs(hz-multiple*fps)*1000+multiple;
        if(rank<score){score=rank;best=mode;}
    }
    if(score==1e6)return 1;
    if(cine_same_mode(&current,&best))return 0;
    if(!d->token){
        WCHAR name[96]=L"Local\\EnigmaxCinePlayer.Refresh.";int n=lstrlenW(name);
        for(int i=0;device[i]&&i<31;i++){WCHAR c=device[i];name[n++]=((c>='0'&&c<='9')||(c>='A'&&c<='Z')||(c>='a'&&c<='z'))?c:'_';}name[n]=0;
        HANDLE token=CreateMutexW(NULL,FALSE,name);if(!token)return 3;
        DWORD result=WaitForSingleObject(token,0);if(result!=WAIT_OBJECT_0&&result!=WAIT_ABANDONED){CloseHandle(token);return 3;}d->token=token;
    }
    if(ChangeDisplaySettingsExW(device,&best,NULL,CDS_TEST,NULL)!=DISP_CHANGE_SUCCESSFUL){if(!d->changed)cine_display_restore(d);return -2;}
    if(!d->changed){d->original=current;lstrcpynW(d->device,device,32);}
    if(ChangeDisplaySettingsExW(device,&best,NULL,0,NULL)!=DISP_CHANGE_SUCCESSFUL){if(!d->changed)cine_display_restore(d);return -3;}
    DEVMODEW actual;ZeroMemory(&actual,sizeof(actual));actual.dmSize=sizeof(actual);
    if(!EnumDisplaySettingsExW(device,ENUM_CURRENT_SETTINGS,&actual,0) || actual.dmPelsWidth!=current.dmPelsWidth || actual.dmPelsHeight!=current.dmPelsHeight) {
        ChangeDisplaySettingsExW(device,&d->original,NULL,0,NULL);d->changed=0;cine_display_restore(d);return -4;
    }
    d->applied=actual;d->changed=1;
    return 2;
}
void cine_display_destroy(void *context) {CineDisplay *d=context;if(d){cine_display_restore(d);free(d);}}
