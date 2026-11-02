/* Opt-in native menu QA. All input is supplied by the tester. */
#define UNICODE
#define _UNICODE
#include <windows.h>
#include <commctrl.h>
#include <assert.h>
#include <stdio.h>
#include <stdint.h>
static HFONT font(int size,int weight){return CreateFontW(-size,0,0,0,weight,0,0,0,DEFAULT_CHARSET,0,0,CLEARTYPE_QUALITY,0,L"Segoe UI");}
static void line(HDC dc,int x,int y,int x2,int y2){MoveToEx(dc,x,y,NULL);LineTo(dc,x2,y2);}
#include "cinema_menu.h"
static HMENU root,position;static int pass,failed;
static LRESULT CALLBACK procedure(HWND host,UINT message,WPARAM wp,LPARAM lp){
 if(message==WM_CONTEXTMENU){
  int x=(short)LOWORD(lp),y=(short)HIWORD(lp);if(x==-1&&y==-1){RECT r;GetWindowRect(host,&r);x=r.left+30;y=r.top+60;}
  cine_menu_begin(root,host,host);UINT result=TrackPopupMenu(root,TPM_RETURNCMD|TPM_RIGHTBUTTON,x,y,0,host,NULL);cine_menu_end();
  printf("Pass %d result=%u foregroundOwner=%d\n",pass,result,GetForegroundWindow()==host);fflush(stdout);
  if(pass<2&&result){if(result!=(pass==0?1145:1152))failed=1;CheckMenuRadioItem(position,1140,1152,result,MF_BYCOMMAND);pass++;}
  else if(pass==2&&result==0){puts("Native percentage selection, checked state, hierarchy, cancel and cleanup: OK");DestroyWindow(host);}
  return 0;
 }
 if(message==WM_DESTROY){PostQuitMessage(failed);return 0;}
 return DefWindowProcW(host,message,wp,lp);
}
int main(int argc,char **argv){
 if(argc!=2||strcmp(argv[1],"--interactive"))return 2;
 root=CreatePopupMenu();HMENU subs=CreatePopupMenu(),appearance=CreatePopupMenu();position=CreatePopupMenu();
 for(int i=0;i<13;i++){wchar_t text[32];_snwprintf(text,32,L"%d %%",50+i*5);AppendMenuW(position,MF_STRING|(i==10?MF_CHECKED:0),1140+i,text);}
 AppendMenuW(appearance,MF_POPUP,(UINT_PTR)position,L"Posición de subtítulos");AppendMenuW(subs,MF_POPUP,(UINT_PTR)appearance,L"Aspecto de subtítulos");AppendMenuW(root,MF_POPUP,(UINT_PTR)subs,L"Pista de subtítulos");
 WNDCLASSW cls={0};cls.lpfnWndProc=procedure;cls.hInstance=GetModuleHandleW(NULL);cls.lpszClassName=L"CinePlayerNativeQA";assert(RegisterClassW(&cls));
 HWND host=CreateWindowExW(0,cls.lpszClassName,L"CinePlayer - Native menu QA",WS_OVERLAPPEDWINDOW,100,100,1200,850,NULL,NULL,NULL,NULL);assert(host);
 ShowWindow(host,SW_SHOW);ShowWindow(host,SW_SHOW);
 MSG msg;while(GetMessageW(&msg,NULL,0,0)>0){TranslateMessage(&msg);DispatchMessageW(&msg);}
 DestroyMenu(root);return failed;
}
