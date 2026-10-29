// SPDX-License-Identifier: GPL-3.0-or-later
// Event-driven Win32 cards. Resources live only for the modal lifetime.
#define UNICODE
#define _UNICODE
#define COBJMACROS
#include <windows.h>
#include <windowsx.h>
#include <commctrl.h>
#include <gdiplus.h>
#include <shlwapi.h>
#include <dwmapi.h>
#include <math.h>

typedef struct { int id; const wchar_t *title, *description; int image; } CineChoice;
typedef struct {
    HWND window, owner, buttons[5], close, back;
    const wchar_t *heading, *subtitle;
    const CineChoice *choices;
    int count, result, width, height, glasses, hover;
    float scale;
    HFONT heading_font, title_font, body_font, brand_font;
    HBITMAP background;
    IStream *stream;
    GpImage *atlas;
} CineDialog;

static int px(CineDialog *d, float v) { return (int)(v * d->scale + 0.5f); }
static void text(HDC dc, HFONT font, RECT r, const wchar_t *s, COLORREF color, UINT flags) {
    HGDIOBJ old=SelectObject(dc,font); SetBkMode(dc,TRANSPARENT); SetTextColor(dc,color);
    DrawTextW(dc,s,-1,&r,flags|DT_NOPREFIX); SelectObject(dc,old);
}
static void tint(HDC dc, RECT r, COLORREF color, BYTE alpha) {
    HDC source=CreateCompatibleDC(dc); HBITMAP bitmap=CreateCompatibleBitmap(dc,1,1);
    if(!source || !bitmap) { if(source)DeleteDC(source);
    if(bitmap)DeleteObject(bitmap);
        return; }
    HGDIOBJ old=SelectObject(source,bitmap); SetPixelV(source,0,0,color);
    BLENDFUNCTION blend={AC_SRC_OVER,0,alpha,0};
    AlphaBlend(dc,r.left,r.top,r.right-r.left,r.bottom-r.top,source,0,0,1,1,blend);
    SelectObject(source,old); DeleteObject(bitmap); DeleteDC(source);
}
static void image(CineDialog *d,HDC dc,RECT r,int which) {
    // Source rectangles address only photographs in the embedded design atlas.
    static const int crops[5][4]={{43,184,466,242},{552,184,431,242},{1035,185,458,241},
                                 {44,690,390,236},{790,690,388,236}};
    if(!d->atlas) return;
    int i=(which>=0 && which<5)?which:1; const int *c=crops[i]; GpGraphics *g=NULL;
    if(GdipCreateFromHDC(dc,&g)==Ok) {
        GdipSetInterpolationMode(g,InterpolationModeHighQualityBicubic);
        GdipDrawImageRectRectI(g,d->atlas,r.left,r.top,r.right-r.left,r.bottom-r.top,
                             c[0],c[1],c[2],c[3],UnitPixel,NULL,NULL,NULL);
        GdipDeleteGraphics(g);
    }
}
static void rectangle(HDC dc,int x,int y,int w,int h) { RoundRect(dc,x,y,x+w,y+h,4,4); }
static void line(HDC dc,int x,int y,int a,int b) { MoveToEx(dc,x,y,NULL); LineTo(dc,a,b); }
static void icon(CineDialog *d,HDC dc,int x,int y,int mode,int color) {
    int w=px(d,78),h=px(d,48),t=max(1,px(d,2));
    HPEN pen=CreatePen(PS_SOLID,t,RGB(226,235,237)); HGDIOBJ op=SelectObject(dc,pen);
    HGDIOBJ ob=SelectObject(dc,GetStockObject(NULL_BRUSH));
    if(mode==0) {
        rectangle(dc,x,y,w*2/3,h*3/4); HPEN mint=CreatePen(PS_SOLID,t,RGB(88,218,185));
        SelectObject(dc,mint); rectangle(dc,x+w/3,y+h/4,w*2/3,h*3/4); SelectObject(dc,pen); DeleteObject(mint);
    } else if(mode==1) {
        rectangle(dc,x,y,w,h*4/5); line(dc,x+w/3,y+h,x+w*2/3,y+h);
        POINT triangle[3]={{x+w*2/5,y+h/5},{x+w*2/5,y+h*3/5},{x+w*3/5,y+h*2/5}};
        HBRUSH b=CreateSolidBrush(RGB(88,218,185)); SelectObject(dc,b); Polygon(dc,triangle,3);
        SelectObject(dc,ob); DeleteObject(b); SelectObject(dc,GetStockObject(NULL_BRUSH));
    } else if(mode==2) {
        static const COLORREF lenses[5][2]={{RGB(225,35,52),RGB(20,185,205)},
            {RGB(65,201,94),RGB(206,65,194)},{RGB(238,184,45),RGB(45,108,227)},
            {RGB(225,35,52),RGB(45,108,227)},{RGB(225,35,52),RGB(65,201,94)}};
        color=max(0,min(4,color));
        for(int i=0;i<2;i++) { HBRUSH b=CreateSolidBrush(lenses[color][i]);
            HGDIOBJ old=SelectObject(dc,b); rectangle(dc,x+i*w*3/5,y,w*2/5,h*3/4);
            SelectObject(dc,old); DeleteObject(b); }
        line(dc,x+w*2/5,y+h/4,x+w*3/5,y+h/4);
    } else if(mode==3) {
        rectangle(dc,x+w/4,y,w/4,h); rectangle(dc,x+w*3/5,y,w/4,h);
        line(dc,x,y+3,x,y+h-3); line(dc,x,y+3,x-px(d,5),y+px(d,8));
        line(dc,x,y+3,x+px(d,5),y+px(d,8));
        line(dc,x,y+h-3,x-px(d,5),y+h-px(d,8)); line(dc,x,y+h-3,x+px(d,5),y+h-px(d,8));
    } else { rectangle(dc,x,y,w,h*2/5); rectangle(dc,x,y+h*3/5,w,h*2/5); }
    SelectObject(dc,ob); SelectObject(dc,op); DeleteObject(pen);
}
static HFONT font(int size,int weight) {
    return CreateFontW(-size,0,0,0,weight,0,0,0,DEFAULT_CHARSET,0,0,CLEARTYPE_QUALITY,0,L"Segoe UI");
}
#include "cinema_menu.h"
static void free_fonts(CineDialog *d) {
    if(d->heading_font)DeleteObject(d->heading_font);
    if(d->title_font)DeleteObject(d->title_font);
    if(d->body_font)DeleteObject(d->body_font);
    if(d->brand_font)DeleteObject(d->brand_font);
}
static void background(CineDialog *d) {
    if(d->background) { DeleteObject(d->background); d->background=NULL; }
    HDC dc=GetDC(d->window),mem=CreateCompatibleDC(dc);
    d->background=CreateCompatibleBitmap(dc,d->width,d->height);
    if(!mem || !d->background) { if(mem)DeleteDC(mem); ReleaseDC(d->window,dc); return; }
    HGDIOBJ old=SelectObject(mem,d->background);
    RECT full={0,0,d->width,d->height}; HBRUSH base=CreateSolidBrush(RGB(18,24,29));
    FillRect(mem,&full,base); DeleteObject(base);
    // Fixed illustrative backdrop, blurred once. Never capture the desktop,
    // occluding apps, or decoded video; it does not affect the playback chain.
    {
        HDC small=CreateCompatibleDC(dc); HBITMAP bmp=CreateCompatibleBitmap(dc,64,40);
        if(small && bmp) { HGDIOBJ prev=SelectObject(small,bmp);
            RECT miniature={0,0,64,40};image(d,small,miniature,1);
            SetStretchBltMode(mem,HALFTONE);StretchBlt(mem,0,0,d->width,d->height,small,0,0,64,40,SRCCOPY);
            SelectObject(small,prev); }
        if(bmp)DeleteObject(bmp);
    if(small)DeleteDC(small);
    }
    tint(mem,full,RGB(17,23,29),215);
    SelectObject(mem,old); DeleteDC(mem); ReleaseDC(d->window,dc);
}
static void layout(CineDialog *d) {
    RECT r; GetClientRect(d->window,&r); d->width=max(1,r.right); d->height=max(1,r.bottom);
    d->scale=min(d->width/1200.0f,d->height/840.0f);
    free_fonts(d); d->heading_font=font(max(18,px(d,34)),600);
    d->title_font=font(max(12,px(d,21)),600); d->body_font=font(max(11,px(d,16)),400);
    d->brand_font=font(max(11,px(d,16)),500);
    int margin=px(d,24),gap=px(d,16),top=px(d,152),cw=(d->width-2*margin-2*gap)/3;
    int upper=px(d,352),lower=px(d,230),bottom=top+upper+gap;
    for(int i=0;i<d->count;i++) {
        int x,y,w,h;
        if(i<3) { x=margin+i*(cw+gap); y=top; w=cw; h=upper; }
        else { w=(d->width-2*margin-gap)/2; x=margin+(i-3)*(w+gap); y=bottom; h=lower; }
        SetWindowPos(d->buttons[i],NULL,x,y,max(1,w),max(1,h),SWP_NOZORDER|SWP_NOACTIVATE);
    }
    SetWindowPos(d->close,NULL,d->width-px(d,76),px(d,10),px(d,56),px(d,44),SWP_NOZORDER|SWP_NOACTIVATE);
    if(d->back)SetWindowPos(d->back,NULL,margin,px(d,63),px(d,104),px(d,42),SWP_NOZORDER|SWP_NOACTIVATE);
    background(d); InvalidateRect(d->window,NULL,FALSE);
    for(int i=0;i<d->count;i++)InvalidateRect(d->buttons[i],NULL,FALSE);
}
static void paint_card(CineDialog *d,DRAWITEMSTRUCT *di) {
    if(di->CtlID==9001 || di->CtlID==9002) {
        HDC bg=CreateCompatibleDC(di->hDC);HGDIOBJ old=SelectObject(bg,d->background);
        RECT screen;GetWindowRect(di->hwndItem,&screen);POINT p={screen.left,screen.top};ScreenToClient(d->window,&p);
        BitBlt(di->hDC,0,0,di->rcItem.right,di->rcItem.bottom,bg,p.x,p.y,SRCCOPY);
        SelectObject(bg,old);DeleteDC(bg);
        BOOL close=di->CtlID==9001,active=(di->itemState&(ODS_FOCUS|ODS_SELECTED))!=0;
        HBRUSH brush=CreateSolidBrush(close?(active?RGB(172,49,62):RGB(91,39,49)):
            (active?RGB(44,84,80):RGB(38,54,61)));
        HPEN pen=CreatePen(PS_SOLID,max(1,px(d,1)),active?RGB(161,235,217):RGB(151,161,173));
        HGDIOBJ ob=SelectObject(di->hDC,brush),op=SelectObject(di->hDC,pen);
        RECT r=di->rcItem;RoundRect(di->hDC,0,0,r.right,r.bottom,px(d,10),px(d,10));
        if(close) {
            // Draw the cross with strokes, independent of font metrics and DPI.
            SelectObject(di->hDC,op);DeleteObject(pen);
            pen=CreatePen(PS_SOLID,max(2,px(d,2)),RGB(255,255,255));SelectObject(di->hDC,pen);
            int cx=r.right/2,cy=r.bottom/2,a=max(6,px(d,9));
            line(di->hDC,cx-a,cy-a,cx+a,cy+a);line(di->hDC,cx+a,cy-a,cx-a,cy+a);
        } else text(di->hDC,d->body_font,r,L"← Volver",RGB(243,250,249),DT_SINGLELINE|DT_CENTER|DT_VCENTER);
        SelectObject(di->hDC,op);SelectObject(di->hDC,ob);DeleteObject(pen);DeleteObject(brush);
        if(di->itemState&ODS_FOCUS){InflateRect(&r,-3,-3);DrawFocusRect(di->hDC,&r);}
        return;
    }
    int index=(int)di->CtlID-200;
    if(index<0 || index>=d->count)return;
    RECT r=di->rcItem; int w=r.right,h=r.bottom; HDC dc=CreateCompatibleDC(di->hDC);
    HBITMAP bmp=CreateCompatibleBitmap(di->hDC,max(1,w),max(1,h));
    if(!dc || !bmp){if(dc)DeleteDC(dc);if(bmp)DeleteObject(bmp);return;}
    HGDIOBJ old=SelectObject(dc,bmp); RECT wr; GetWindowRect(di->hwndItem,&wr);
    POINT p={wr.left,wr.top}; ScreenToClient(d->window,&p);
    HDC bg=CreateCompatibleDC(dc); HGDIOBJ prev=SelectObject(bg,d->background);
    BitBlt(dc,0,0,w,h,bg,p.x,p.y,SRCCOPY); SelectObject(bg,prev); DeleteDC(bg);
    tint(dc,r,RGB(29,36,42),170);
    int pad=px(d,12); RECT photo={pad,pad,w-pad,px(d,202)};
    BOOL lower=index>=3;
    if(lower) { photo.right=w*55/100;photo.bottom=h-pad; }
    const CineChoice *c=&d->choices[index];
    image(d,dc,photo,d->glasses?1:c->image);
    if(d->glasses) {
        tint(dc,photo,RGB(10,15,20),105);
        icon(d,dc,(photo.left+photo.right-px(d,78))/2,(photo.top+photo.bottom-px(d,48))/2,2,index);
    }
    int tx=lower?photo.right+px(d,24):px(d,20);
    int iy=lower?px(d,35):px(d,221);
    icon(d,dc,lower?tx:(w-px(d,78))/2,iy,d->glasses?2:c->image,d->glasses?index:0);
    RECT title={tx,iy+px(d,58),w-px(d,12),iy+px(d,96)};
    text(dc,d->title_font,title,c->title,RGB(239,243,245),DT_SINGLELINE|DT_VCENTER|(lower?DT_LEFT:DT_CENTER)|DT_END_ELLIPSIS);
    if(c->description && c->description[0]) {
        RECT desc={tx,title.bottom+px(d,4),w-px(d,12),h-px(d,8)};
        text(dc,d->body_font,desc,c->description,RGB(183,199,208),(lower?DT_LEFT:DT_CENTER)|DT_WORDBREAK);
    }
    HPEN pen=CreatePen(PS_SOLID,max(1,px(d,1)),(di->itemState&(ODS_FOCUS|ODS_SELECTED))?RGB(100,211,184):RGB(73,85,91));
    HGDIOBJ op=SelectObject(dc,pen),ob=SelectObject(dc,GetStockObject(NULL_BRUSH));
    RoundRect(dc,0,0,w,h,px(d,16),px(d,16)); SelectObject(dc,ob);SelectObject(dc,op);DeleteObject(pen);
    BitBlt(di->hDC,0,0,w,h,dc,0,0,SRCCOPY);SelectObject(dc,old);DeleteObject(bmp);DeleteDC(dc);
}
static LRESULT CALLBACK procedure(HWND window,UINT msg,WPARAM wp,LPARAM lp) {
    CineDialog *d=(CineDialog*)GetWindowLongPtrW(window,GWLP_USERDATA);
    if(msg==WM_NCCREATE) { d=(CineDialog*)((CREATESTRUCTW*)lp)->lpCreateParams;
        d->window=window;SetWindowLongPtrW(window,GWLP_USERDATA,(LONG_PTR)d); }
    if(!d)return DefWindowProcW(window,msg,wp,lp);
    switch(msg) {
    case WM_SIZE: if(d->buttons[0])layout(d);return 0;
    case WM_DPICHANGED: {
        RECT *r=(RECT*)lp;SetWindowPos(window,NULL,r->left,r->top,r->right-r->left,r->bottom-r->top,SWP_NOZORDER|SWP_NOACTIVATE);
        return 0; }
    case WM_NCHITTEST: {
        LRESULT hit=DefWindowProcW(window,msg,wp,lp);
        POINT p={GET_X_LPARAM(lp),GET_Y_LPARAM(lp)};ScreenToClient(window,&p);
        if(hit==HTCLIENT && p.y<px(d,50) && p.x<d->width-px(d,80))return HTCAPTION;
        return hit; }
    case WM_ERASEBKGND:return 1;
    case WM_DRAWITEM:paint_card(d,(DRAWITEMSTRUCT*)lp);return TRUE;
    case WM_COMMAND: {
        int id=LOWORD(wp);
        if(id>=200 && id<200+d->count) { d->result=d->choices[id-200].id;DestroyWindow(window); }
        else if(id==9001)DestroyWindow(window);
        else if(id==9002) {d->result=1231;DestroyWindow(window);}
        return 0; }
    case WM_CLOSE:DestroyWindow(window);return 0;
    case WM_GETMINMAXINFO:((MINMAXINFO*)lp)->ptMinTrackSize=(POINT){640,460};return 0;
    case WM_PAINT: {
        PAINTSTRUCT ps;HDC dc=BeginPaint(window,&ps);
        if(d->background) {HDC bg=CreateCompatibleDC(dc);HGDIOBJ old=SelectObject(bg,d->background);
            BitBlt(dc,0,0,d->width,d->height,bg,0,0,SRCCOPY);SelectObject(bg,old);DeleteDC(bg);}
        RECT brand={px(d,25),px(d,19),d->width-px(d,60),px(d,47)};
        text(dc,d->brand_font,brand,L"ENIGMAX  CINEPLAYER",RGB(221,231,236),DT_SINGLELINE);
        RECT heading={px(d,d->back?150:25),px(d,65),d->width-px(d,25),px(d,111)};
        text(dc,d->heading_font,heading,d->heading,RGB(242,245,247),DT_SINGLELINE|DT_END_ELLIPSIS);
        RECT sub={px(d,25),px(d,117),d->width-px(d,25),px(d,145)};
        text(dc,d->body_font,sub,d->subtitle,RGB(177,196,206),DT_SINGLELINE|DT_END_ELLIPSIS);
        if(!d->glasses) {RECT note={px(d,25),d->height-px(d,37),d->width-px(d,25),d->height-px(d,8)};
            text(dc,d->body_font,note,L"Frame Packing: composición experimental; no activa HDMI 3D.",RGB(145,163,173),DT_SINGLELINE|DT_END_ELLIPSIS);}
        EndPaint(window,&ps);return 0; }
    }
    return DefWindowProcW(window,msg,wp,lp);
}

int cine_choose(HWND owner,const wchar_t *heading,const wchar_t *subtitle,const CineChoice *choices,int count,int glasses) {
    if(!heading || !subtitle || !choices || count!=5)return -1;
    CineDialog d={0}; d.owner=IsWindow(owner)?owner:NULL;d.heading=heading;d.subtitle=subtitle;
    d.choices=choices;d.count=count;d.glasses=glasses;
    ULONG_PTR token=0;GdiplusStartupInput input={1,NULL,FALSE,TRUE};
    if(GdiplusStartup(&token,&input,NULL)!=Ok)return -2;
    HINSTANCE instance=GetModuleHandleW(NULL);HRSRC res=FindResourceW(instance,MAKEINTRESOURCEW(201),RT_RCDATA);
    HGLOBAL data=res?LoadResource(instance,res):NULL;
    if(data) {d.stream=SHCreateMemStream(LockResource(data),SizeofResource(instance,res));
        if(d.stream)GdipLoadImageFromStream(d.stream,&d.atlas);}
    WNDCLASSEXW wc={0};wc.cbSize=sizeof(wc);wc.lpfnWndProc=procedure;wc.hInstance=instance;
    wc.hCursor=LoadCursorW(NULL,IDC_ARROW);wc.lpszClassName=L"EnigmaxCinemaChoice";
    if(!RegisterClassExW(&wc) && GetLastError()!=ERROR_CLASS_ALREADY_EXISTS){d.result=-3;goto cleanup;}
    MONITORINFO mi={sizeof(mi),{0},{0},0};GetMonitorInfoW(MonitorFromWindow(d.owner,MONITOR_DEFAULTTONEAREST),&mi);
    int aw=mi.rcWork.right-mi.rcWork.left,ah=mi.rcWork.bottom-mi.rcWork.top;
    UINT dpi=d.owner?GetDpiForWindow(d.owner):96;
    float scale=min(dpi/96.0f,min(aw*0.92f/1200,ah*0.92f/840));
    int width=(int)(1200*scale),height=(int)(840*scale);
    d.window=CreateWindowExW(WS_EX_CONTROLPARENT,wc.lpszClassName,L"Enigmax CinePlayer — Vídeo 3D",
        WS_POPUP|WS_THICKFRAME|WS_SYSMENU,mi.rcWork.left+(aw-width)/2,mi.rcWork.top+(ah-height)/2,
        width,height,d.owner,NULL,instance,&d);
    if(!d.window){d.result=-4;goto cleanup;}
    int dark=1,round=2;DwmSetWindowAttribute(d.window,20,&dark,sizeof(dark));DwmSetWindowAttribute(d.window,33,&round,sizeof(round));
    for(int i=0;i<count;i++)d.buttons[i]=CreateWindowExW(0,L"BUTTON",choices[i].title,
        WS_CHILD|WS_VISIBLE|WS_TABSTOP|BS_OWNERDRAW,0,0,1,1,d.window,(HMENU)(INT_PTR)(200+i),instance,NULL);
    d.close=CreateWindowExW(0,L"BUTTON",L"Cerrar",WS_CHILD|WS_VISIBLE|WS_TABSTOP|BS_OWNERDRAW,0,0,1,1,d.window,(HMENU)9001,instance,NULL);
    if(glasses)d.back=CreateWindowExW(0,L"BUTTON",L"← Volver",WS_CHILD|WS_VISIBLE|WS_TABSTOP|BS_OWNERDRAW,0,0,1,1,d.window,(HMENU)9002,instance,NULL);
    layout(&d);if(d.owner)EnableWindow(d.owner,FALSE);
    ShowWindow(d.window,SW_SHOW);SetForegroundWindow(d.window);SetFocus(d.buttons[0]);
    MSG msg;while(IsWindow(d.window)) {
        int status=GetMessageW(&msg,NULL,0,0);if(status<=0){if(status==0)PostQuitMessage((int)msg.wParam);break;}
        if(msg.message==WM_KEYDOWN && msg.wParam==VK_ESCAPE) {DestroyWindow(d.window);break;}
        if(msg.message==WM_KEYDOWN && msg.wParam==VK_RETURN) {
            HWND focus=GetFocus();if(IsChild(d.window,focus)){SendMessageW(d.window,WM_COMMAND,GetDlgCtrlID(focus),0);continue;}
        }
        if(!IsDialogMessageW(d.window,&msg)){TranslateMessage(&msg);DispatchMessageW(&msg);}
    }
    if(IsWindow(d.window))DestroyWindow(d.window);
    if(d.owner){EnableWindow(d.owner,TRUE);SetForegroundWindow(d.owner);}
cleanup:
    free_fonts(&d);if(d.background)DeleteObject(d.background);
    if(d.atlas)GdipDisposeImage(d.atlas);
    if(d.stream)IStream_Release(d.stream);
    GdiplusShutdown(token);
    return d.result;
}
