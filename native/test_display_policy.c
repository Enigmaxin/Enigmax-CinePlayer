/* Deterministic display-driver simulation: never changes the real display. */
#include <windows.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <assert.h>
#include <string.h>
typedef struct {DWORD alias;double hz;} Mode;
static Mode available[16];static unsigned total;
static double active=60;static int switches,query_error,buffer_retry,wrong_size;
static BOOL fake_IsWindow(HWND w){return w!=NULL;}
static HMONITOR fake_MonitorFromWindow(HWND w,DWORD flags){(void)w;(void)flags;return (HMONITOR)1;}
static BOOL fake_GetMonitorInfoW(HMONITOR mon,LPMONITORINFO info){(void)mon;lstrcpyW(((MONITORINFOEXW*)info)->szDevice,L"CINEPLAYER_QA");return TRUE;}
static BOOL fake_EnumDisplaySettingsExW(LPCWSTR device,DWORD index,DEVMODEW *m,DWORD flags){
 (void)device;(void)flags;if(index!=ENUM_CURRENT_SETTINGS&&index>=total)return FALSE;
 m->dmPelsWidth=wrong_size?1280:1920;m->dmPelsHeight=1080;m->dmBitsPerPel=32;
 m->dmDisplayFrequency=index==ENUM_CURRENT_SETTINGS?(DWORD)floor(active+1e-6):available[index].alias;return TRUE;
}
static LONG fake_ChangeDisplaySettingsExW(LPCWSTR device,DEVMODEW *m,HWND hwnd,DWORD flags,LPVOID data){
 (void)device;(void)hwnd;(void)data;
 for(unsigned i=0;i<total;i++)if(available[i].alias==m->dmDisplayFrequency){if(!(flags&CDS_TEST)){active=available[i].hz;switches++;}return DISP_CHANGE_SUCCESSFUL;}
 return DISP_CHANGE_BADMODE;
}
static LONG fake_GetDisplayConfigBufferSizes(UINT32 flags,UINT32 *np,UINT32 *nm){(void)flags;*np=1;*nm=1;return query_error?ERROR_NOT_SUPPORTED:ERROR_SUCCESS;}
static LONG fake_QueryDisplayConfig(UINT32 flags,UINT32 *np,DISPLAYCONFIG_PATH_INFO *p,UINT32 *nm,DISPLAYCONFIG_MODE_INFO *m,DISPLAYCONFIG_TOPOLOGY_ID *top){
 (void)flags;(void)np;(void)nm;(void)top;
 if(buffer_retry){buffer_retry=0;return ERROR_INSUFFICIENT_BUFFER;}
 ZeroMemory(p,sizeof(*p));ZeroMemory(m,sizeof(*m));p->targetInfo.modeInfoIdx=0;m->infoType=DISPLAYCONFIG_MODE_INFO_TYPE_TARGET;
 m->targetMode.targetVideoSignalInfo.vSyncFreq.Numerator=(UINT32)llround(active*1000000);
 m->targetMode.targetVideoSignalInfo.vSyncFreq.Denominator=1000000;return ERROR_SUCCESS;
}
static LONG fake_DisplayConfigGetDeviceInfo(DISPLAYCONFIG_DEVICE_INFO_HEADER *head){lstrcpyW(((DISPLAYCONFIG_SOURCE_DEVICE_NAME*)head)->viewGdiDeviceName,L"CINEPLAYER_QA");return ERROR_SUCCESS;}
#define IsWindow fake_IsWindow
#define MonitorFromWindow fake_MonitorFromWindow
#define GetMonitorInfoW fake_GetMonitorInfoW
#define EnumDisplaySettingsExW fake_EnumDisplaySettingsExW
#define ChangeDisplaySettingsExW fake_ChangeDisplaySettingsExW
#define GetDisplayConfigBufferSizes fake_GetDisplayConfigBufferSizes
#define QueryDisplayConfig fake_QueryDisplayConfig
#define DisplayConfigGetDeviceInfo fake_DisplayConfigGetDeviceInfo
#include "display_policy.h"
static void setup(void){total=0;active=60;switches=query_error=buffer_retry=wrong_size=0;available[total++]=(Mode){60,60};}
static void add(DWORD alias,double hz){available[total++]=(Mode){alias,hz};}
static void run(double fps,double expected){CineDisplay *d=cine_display_create();assert(d);assert(cine_display_update(d,1,1,fps)==2);assert(cine_rate_matches(active,expected));assert(cine_rate_matches(cine_display_rate(d,1),expected));cine_display_destroy(d);assert(cine_rate_matches(active,60));}
int main(void){
 assert(!cine_rate_matches(24,24000.0/1001));assert(!cine_rate_matches(48,48000.0/1001));assert(!cine_rate_matches(120,120000.0/1001));
 setup();add(24,24);add(23,24000.0/1001);run(24,24);run(23.976,24000.0/1001);
 setup();add(25,25);add(50,50);run(25,25);
 setup();add(50,50);run(25,50);
 setup();add(120,120);run(24,120);
 setup();add(24,24);add(47,48000.0/1001);run(23.976,48000.0/1001);
 setup();add(24,24);CineDisplay *d=cine_display_create();assert(cine_display_update(d,1,1,23.976)==1);assert(active==60);cine_display_destroy(d);
 setup();query_error=1;d=cine_display_create();assert(cine_display_update(d,1,1,24)==1);assert(!switches);cine_display_destroy(d);
 setup();add(24,24);buffer_retry=1;run(24,24);
 setup();add(24,24);d=cine_display_create();assert(cine_display_update(d,1,1,NAN)==0);assert(cine_display_update(d,1,1,INFINITY)==0);assert(!switches);cine_display_destroy(d);
 setup();add(24,24);add(23,24000.0/1001);add(50,50);d=cine_display_create();
 assert(cine_display_update(d,1,1,24)==2);assert(active==24);
 assert(cine_display_update(d,1,1,23.976)==2);assert(cine_rate_matches(active,24000.0/1001));
 assert(cine_display_update(d,1,1,25)==2);assert(active==50);
 assert(cine_display_update(d,1,0,25)==0);assert(active==60);cine_display_destroy(d);
 setup();add(24,24);add(75,75);d=cine_display_create();assert(cine_display_update(d,1,1,24)==2);active=75;cine_display_destroy(d);assert(active==75);
 puts("Refresh policy: 24 vs 23.976, 25, exact multiples, rollback, unavailable CCD, retry, invalid input, live changes and external override: OK");return 0;
}
