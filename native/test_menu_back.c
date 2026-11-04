// Native cascade regression. All input is targeted at this test's own window.
#define UNICODE
#define _UNICODE
#include <windows.h>
#include <commctrl.h>
#include <assert.h>
#include <stdio.h>
static HFONT font(int size,int weight){return CreateFontW(-size,0,0,0,weight,0,0,0,DEFAULT_CHARSET,0,0,CLEARTYPE_QUALITY,0,L"Segoe UI");}
static void line(HDC dc,int x,int y,int x2,int y2){MoveToEx(dc,x,y,NULL);LineTo(dc,x2,y2);}
#include "cinema_menu.h"
static HMENU root,child;static int step,failed;
static LRESULT CALLBACK procedure(HWND host,UINT msg,WPARAM wp,LPARAM lp){
    if(msg==WM_TIMER && wp==77){
        step++;
        if(step==1)PostMessageW(host,WM_KEYDOWN,VK_DOWN,0);
        if(step==2)PostMessageW(host,WM_KEYDOWN,VK_RIGHT,0);
        if(step==3){
            if(m_theme.active!=child){failed=1;EndMenu();return 0;}
            RECT r;assert(GetMenuItemRect(host,child,0,&r));
            assert(r.bottom-r.top>=m_px(48));
            MSG event={0};event.hwnd=host;event.pt.x=(r.left+r.right)/2;event.pt.y=(r.top+r.bottom)/2;
            event.message=WM_LBUTTONDOWN;assert(m_input(MSGF_MENU,0,(LPARAM)&event)==1);
            event.message=WM_LBUTTONUP;assert(m_input(MSGF_MENU,0,(LPARAM)&event)==1);
        }
        if(step==4){
            if(m_theme.active!=root){failed=2;EndMenu();return 0;}
            PostMessageW(host,WM_KEYDOWN,VK_RIGHT,0);
        }
        if(step==5){
            if(m_theme.active!=child){failed=3;EndMenu();return 0;}
            PostMessageW(host,WM_KEYDOWN,VK_ESCAPE,0);
        }
        if(step==6)PostMessageW(host,WM_KEYDOWN,VK_ESCAPE,0);
        if(step>12){failed=4;EndMenu();}
        return 0;
    }
    return DefWindowProcW(host,msg,wp,lp);
}
int main(void){
    root=CreatePopupMenu();child=CreatePopupMenu();
    AppendMenuW(child,MF_STRING,101,L"Ajuste");AppendMenuW(root,MF_POPUP,(UINT_PTR)child,L"Submenu");
    WNDCLASSW cls={0};cls.lpfnWndProc=procedure;cls.hInstance=GetModuleHandleW(NULL);cls.lpszClassName=L"CineBackRegression";
    assert(RegisterClassW(&cls));HWND host=CreateWindowW(cls.lpszClassName,L"CinePlayer navigation test",WS_OVERLAPPEDWINDOW,100,100,640,480,NULL,NULL,cls.hInstance,NULL);assert(host);
    ShowWindow(host,SW_SHOW);SetForegroundWindow(host);
    for(int pass=0;pass<3;pass++){
        step=0;SetTimer(host,77,120,NULL);cine_menu_begin(root,host,host);
        assert(GetMenuItemCount(child)==2);assert(GetMenuItemID(child,0)==CINE_MENU_BACK);
        UINT selected=TrackPopupMenu(root,TPM_RETURNCMD,150,180,0,host,NULL);
        cine_menu_end();KillTimer(host,77);
        assert(!selected && !failed);assert(GetMenuItemCount(child)==1);assert(GetMenuItemID(child,0)==101);
        assert(!m_theme.items && !m_theme.input_hook);
    }
    DestroyWindow(host);DestroyMenu(root);
    puts("3 native cascades: large clickable Back, reopen, Escape and exact cleanup OK");return 0;
}
