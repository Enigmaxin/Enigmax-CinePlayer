// SPDX-License-Identifier: GPL-3.0-or-later
// Native popup navigation with an event-driven CinePlayer M appearance.
// Strings remain in HMENU for accessibility; all styling is modal and reversible.
#include <stdlib.h>
#include <wchar.h>
typedef struct CineMenuItem {
    struct CineMenuItem *next;
    HMENU menu;
    UINT position, type;
    ULONG_PTR data;
    wchar_t *label;
    BOOL submenu;
} CineMenuItem;
typedef struct {
    CineMenuItem *items;
    HWND host;
    HMENU root;
    HFONT normal, small;
    HBRUSH background;
    int dpi;
} CineMenuTheme;
static __thread CineMenuTheme m_theme;
static int m_px(int n) { return MulDiv(n,m_theme.dpi,96); }
static CineMenuItem *m_item(ULONG_PTR data) {
    for(CineMenuItem *i=m_theme.items;i;i=i->next)if((ULONG_PTR)i==data)return i;
    return NULL;
}
static LRESULT CALLBACK m_procedure(HWND window,UINT msg,WPARAM wp,LPARAM lp,
                                   UINT_PTR id,DWORD_PTR data) {
    (void)id;(void)data;
    if(msg==WM_MEASUREITEM) {
        MEASUREITEMSTRUCT *mi=(MEASUREITEMSTRUCT*)lp;
        CineMenuItem *i=mi->CtlType==ODT_MENU?m_item(mi->itemData):NULL;
        if(i) {
            HDC dc=GetDC(window);HGDIOBJ old=SelectObject(dc,m_theme.normal);
            RECT r={0};DrawTextW(dc,i->label,-1,&r,DT_CALCRECT|DT_NOPREFIX|DT_EXPANDTABS);
            SelectObject(dc,old);ReleaseDC(window,dc);
            mi->itemWidth=(UINT)min(m_px(580),max(m_px(220),r.right+m_px(66)));
            mi->itemHeight=(UINT)m_px((i->type&MFT_SEPARATOR)?9:(wcschr(i->label,L'\n')?57:38));
            return TRUE;
        }
    } else if(msg==WM_DRAWITEM) {
        DRAWITEMSTRUCT *di=(DRAWITEMSTRUCT*)lp;
        CineMenuItem *i=di->CtlType==ODT_MENU?m_item(di->itemData):NULL;
        if(i) {
            RECT r=di->rcItem;HDC dc=di->hDC;
            BOOL disabled=(di->itemState&(ODS_DISABLED|ODS_GRAYED))!=0;
            BOOL selected=!disabled && (di->itemState&ODS_SELECTED);
            FillRect(dc,&r,m_theme.background);
            if(i->type&MFT_SEPARATOR) {
                RECT rule={r.left+m_px(12),(r.top+r.bottom)/2,r.right-m_px(12),(r.top+r.bottom)/2+1};
                HBRUSH b=CreateSolidBrush(RGB(58,63,67));FillRect(dc,&rule,b);DeleteObject(b);return TRUE;
            }
            if(selected) {
                RECT hover=r;InflateRect(&hover,-m_px(5),-m_px(2));
                HBRUSH b=CreateSolidBrush(RGB(49,55,59));
                HGDIOBJ ob=SelectObject(dc,b),op=SelectObject(dc,GetStockObject(NULL_PEN));
                RoundRect(dc,hover.left,hover.top,hover.right,hover.bottom,m_px(8),m_px(8));
                SelectObject(dc,ob);SelectObject(dc,op);DeleteObject(b);
            }
            RECT label={r.left+m_px(32),r.top,r.right-m_px(30),r.bottom};
            const wchar_t *second=wcschr(i->label,L'\n');
            HGDIOBJ old=SelectObject(dc,m_theme.normal);SetBkMode(dc,TRANSPARENT);
            SetTextColor(dc,disabled?RGB(161,170,174):RGB(237,240,242));
            if(second) {
                label.top+=m_px(6);label.bottom=label.top+m_px(23);
                DrawTextW(dc,i->label,(int)(second-i->label),&label,DT_SINGLELINE|DT_NOPREFIX|DT_END_ELLIPSIS);
                SelectObject(dc,m_theme.small);SetTextColor(dc,RGB(160,171,177));
                label.top+=m_px(25);label.bottom=r.bottom;
                DrawTextW(dc,second+1,-1,&label,DT_SINGLELINE|DT_NOPREFIX|DT_END_ELLIPSIS);
            } else DrawTextW(dc,i->label,-1,&label,DT_SINGLELINE|DT_VCENTER|DT_NOPREFIX|DT_EXPANDTABS|DT_END_ELLIPSIS);
            SelectObject(dc,old);
            if(di->itemState&ODS_CHECKED) {
                HPEN pen=CreatePen(PS_SOLID,max(1,m_px(2)),RGB(149,219,201));HGDIOBJ op=SelectObject(dc,pen);
                int x=r.left+m_px(12),cy=(r.top+r.bottom)/2;
                line(dc,x,cy,x+m_px(4),cy+m_px(4));line(dc,x+m_px(4),cy+m_px(4),x+m_px(12),cy-m_px(5));
                SelectObject(dc,op);DeleteObject(pen);
            }
            if(i->submenu) {
                HPEN pen=CreatePen(PS_SOLID,max(1,m_px(1)),RGB(175,189,196));HGDIOBJ op=SelectObject(dc,pen);
                int x=r.right-m_px(15),cy=(r.top+r.bottom)/2;
                line(dc,x-m_px(4),cy-m_px(4),x,cy);line(dc,x,cy,x-m_px(4),cy+m_px(4));
                SelectObject(dc,op);DeleteObject(pen);
            }
            return TRUE;
        }
    }
    return DefSubclassProc(window,msg,wp,lp);
}
static BOOL m_decorate(HMENU menu,int depth) {
    if(depth>16)return FALSE;
    MENUINFO info={0};info.cbSize=sizeof(info);info.fMask=MIM_BACKGROUND;info.hbrBack=m_theme.background;
    SetMenuInfo(menu,&info);
    int count=GetMenuItemCount(menu);
    for(int n=0;n<count;n++) {
        MENUITEMINFOW mi={0};mi.cbSize=sizeof(mi);mi.fMask=MIIM_FTYPE|MIIM_DATA|MIIM_SUBMENU|MIIM_STRING;
        if(!GetMenuItemInfoW(menu,(UINT)n,TRUE,&mi))return FALSE;
        CineMenuItem *i=(CineMenuItem*)calloc(1,sizeof(*i));if(!i)return FALSE;
        i->label=(wchar_t*)calloc((size_t)mi.cch+1,sizeof(wchar_t));
        if(!i->label){free(i);return FALSE;}
        i->menu=menu;i->position=(UINT)n;i->type=mi.fType;i->data=mi.dwItemData;i->submenu=mi.hSubMenu!=NULL;
        HMENU child=mi.hSubMenu;
        mi.dwTypeData=i->label;mi.cch++;
        if(!GetMenuItemInfoW(menu,(UINT)n,TRUE,&mi)){free(i->label);free(i);return FALSE;}
        // Rust escapes literal ampersands for native menus. DT_NOPREFIX must
        // display the same single character while HMENU keeps its own string.
        wchar_t *read=i->label,*write=i->label;
        while(*read){*write++=*read;if(*read==L'&' && read[1]==L'&')read++;read++;}
        *write=L'\0';
        mi.fMask=MIIM_FTYPE|MIIM_DATA;mi.fType=i->type|MFT_OWNERDRAW;mi.dwItemData=(ULONG_PTR)i;
        if(!SetMenuItemInfoW(menu,(UINT)n,TRUE,&mi)){free(i->label);free(i);return FALSE;}
        i->next=m_theme.items;m_theme.items=i;
        if(child && !m_decorate(child,depth+1))return FALSE;
    }
    return TRUE;
}
static void m_reset_background(HMENU menu) {
    MENUINFO info={0};info.cbSize=sizeof(info);info.fMask=MIM_BACKGROUND;SetMenuInfo(menu,&info);
    for(int n=0;n<GetMenuItemCount(menu);n++) {
        HMENU child=GetSubMenu(menu,n);if(child)m_reset_background(child);
    }
}
void cine_menu_end(void) {
    if(m_theme.host)RemoveWindowSubclass(m_theme.host,m_procedure,1);
    if(m_theme.root)m_reset_background(m_theme.root);
    CineMenuItem *i=m_theme.items;
    while(i) {
        MENUITEMINFOW mi={0};mi.cbSize=sizeof(mi);mi.fMask=MIIM_FTYPE|MIIM_DATA;
        mi.fType=i->type;mi.dwItemData=i->data;SetMenuItemInfoW(i->menu,i->position,TRUE,&mi);
        MENUINFO info={0};info.cbSize=sizeof(info);info.fMask=MIM_BACKGROUND;SetMenuInfo(i->menu,&info);
        CineMenuItem *next=i->next;free(i->label);free(i);i=next;
    }
    if(m_theme.normal)DeleteObject(m_theme.normal);
    if(m_theme.small)DeleteObject(m_theme.small);
    if(m_theme.background)DeleteObject(m_theme.background);
    ZeroMemory(&m_theme,sizeof(m_theme));
}
void cine_menu_begin(HMENU menu,HWND host,HWND owner) {
    HIGHCONTRASTW hc={sizeof(hc),0,NULL};
    if(SystemParametersInfoW(SPI_GETHIGHCONTRAST,sizeof(hc),&hc,0) && (hc.dwFlags&HCF_HIGHCONTRASTON))return;
    m_theme.dpi=(int)GetDpiForWindow(owner);if(m_theme.dpi<96)m_theme.dpi=96;
    m_theme.root=menu;
    m_theme.normal=font(m_px(15),400);m_theme.small=font(m_px(12),400);
    m_theme.background=CreateSolidBrush(RGB(27,31,34));
    if(!m_theme.normal || !m_theme.small || !m_theme.background || !m_decorate(menu,0) ||
       !SetWindowSubclass(host,m_procedure,1,0)){cine_menu_end();return;}
    m_theme.host=host;
}
