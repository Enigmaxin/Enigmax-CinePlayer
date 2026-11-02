/* Read-only inventory. Does not call ChangeDisplaySettings or SetDisplayConfig. */
#include <windows.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include "display_policy.h"
int main(void){
 for(DWORD i=0;i<32;i++){
  DISPLAY_DEVICEW device;ZeroMemory(&device,sizeof(device));device.cb=sizeof(device);
  if(!EnumDisplayDevicesW(NULL,i,&device,0))break;
  if(!(device.StateFlags&DISPLAY_DEVICE_ATTACHED_TO_DESKTOP))continue;
  DEVMODEW current;ZeroMemory(&current,sizeof(current));current.dmSize=sizeof(current);
  if(!EnumDisplaySettingsExW(device.DeviceName,ENUM_CURRENT_SETTINGS,&current,0))continue;
  printf("Display %lu: %lux%lu, physical CCD %.6f Hz; native-resolution legacy aliases:",
   (unsigned long)i,(unsigned long)current.dmPelsWidth,(unsigned long)current.dmPelsHeight,cine_display_hz(device.DeviceName));
  DWORD seen[256];unsigned count=0;
  for(DWORD j=0;j<4096;j++){
   DEVMODEW mode;ZeroMemory(&mode,sizeof(mode));mode.dmSize=sizeof(mode);
   if(!EnumDisplaySettingsExW(device.DeviceName,j,&mode,0))break;
   if(mode.dmPelsWidth!=current.dmPelsWidth||mode.dmPelsHeight!=current.dmPelsHeight||mode.dmBitsPerPel!=current.dmBitsPerPel||mode.dmDisplayFlags!=current.dmDisplayFlags)continue;
   unsigned k;for(k=0;k<count;k++)if(seen[k]==mode.dmDisplayFrequency)break;
   if(k==count&&count<256){seen[count++]=mode.dmDisplayFrequency;printf(" %lu",(unsigned long)mode.dmDisplayFrequency);}
  }
  puts("");
 }
 return 0;
}
