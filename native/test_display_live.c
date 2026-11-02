/* Explicit opt-in physical test; restore the original mode on every normal exit. */
#include <windows.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <math.h>
#include <string.h>
#include "display_policy.h"
int main(int argc,char **argv){
 if(argc!=2||strcmp(argv[1],"--allow-temporary-mode-change")){fputs("Explicit opt-in required\n",stderr);return 2;}
 HWND window=GetForegroundWindow();WCHAR device[32];
 if(!cine_monitor_name((uintptr_t)window,device))return 3;
 double original=cine_display_hz(device);if(!original)return 4;
 DEVMODEW before;ZeroMemory(&before,sizeof(before));before.dmSize=sizeof(before);
 if(!EnumDisplaySettingsExW(device,ENUM_CURRENT_SETTINGS,&before,0))return 5;
 CineDisplay *context=cine_display_create();if(!context)return 6;
 const double fps[]={24,24000.0/1001.0,25};int failed=0;
 printf("Original %.9f Hz, %lux%lu\n",original,(unsigned long)before.dmPelsWidth,(unsigned long)before.dmPelsHeight);fflush(stdout);
 for(unsigned i=0;i<3;i++){
  int code=cine_display_update(context,(uintptr_t)window,1,fps[i]);Sleep(500);
  double actual=cine_display_hz(device);
  printf("Source %.9f fps: result %d, physical CCD %.9f Hz\n",fps[i],code,actual);fflush(stdout);
  if(code!=2||!cine_rate_matches(actual,fps[i])){failed=1;break;}
 }
 int restore=cine_display_update(context,(uintptr_t)window,0,0);
 cine_display_destroy(context);Sleep(500);
 DEVMODEW after;ZeroMemory(&after,sizeof(after));after.dmSize=sizeof(after);
 double final=cine_display_hz(device);
 int restored=restore==0&&cine_rate_matches(original,final)&&EnumDisplaySettingsExW(device,ENUM_CURRENT_SETTINGS,&after,0)&&cine_same_mode(&before,&after);
 printf("Restored %.9f Hz: %s\n",final,restored?"OK":"FAILED");
 return failed||!restored;
}
