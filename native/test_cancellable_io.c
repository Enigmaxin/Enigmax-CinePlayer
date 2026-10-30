#include "cancellable_io.h"
#include <assert.h>
#include <stdio.h>
static DWORD WINAPI cancel_later(void *p){Sleep(30);atomic_store(&((EC_Cancel*)p)->cancelled,1);return 0;}
int main(void){
    DWORD before=0,after=0;assert(GetProcessHandleCount(GetCurrentProcess(),&before));
    char name[128];snprintf(name,sizeof(name),"\\\\.\\pipe\\Enigmax-io-test-%lu",(unsigned long)GetCurrentProcessId());
    HANDLE pipe=CreateNamedPipeA(name,PIPE_ACCESS_INBOUND|FILE_FLAG_OVERLAPPED,PIPE_TYPE_BYTE|PIPE_WAIT,1,1024,1024,0,NULL);assert(pipe!=INVALID_HANDLE_VALUE);
    HANDLE peer=CreateFileA(name,GENERIC_WRITE,0,NULL,OPEN_EXISTING,0,NULL);assert(peer!=INVALID_HANDLE_VALUE);
    EC_Cancel *stop=ec_stop_new();assert(stop);
    EC_FileIO io={0};io.handle=pipe;io.event=CreateEventW(NULL,TRUE,FALSE,NULL);io.size=1024;io.cancel=stop;assert(io.event);
    char data[64];HANDLE worker=CreateThread(NULL,0,cancel_later,stop,0,NULL);assert(worker);
    ULONGLONG started=GetTickCount64();assert(ec_io_read_at(&io,data,sizeof(data),0)==-ECANCELED);assert(GetTickCount64()-started<2000);
    assert(WaitForSingleObject(worker,2000)==WAIT_OBJECT_0);CloseHandle(worker);ec_io_close(&io);CloseHandle(peer);free(stop);
    wchar_t dir[MAX_PATH],file[MAX_PATH];assert(GetTempPathW(MAX_PATH,dir));assert(GetTempFileNameW(dir,L"ecq",0,file));
    HANDLE h=CreateFileW(file,GENERIC_WRITE,FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE,NULL,OPEN_EXISTING,0,NULL);assert(h!=INVALID_HANDLE_VALUE);
    DWORD written=0;memset(data,42,sizeof(data));assert(WriteFile(h,data,sizeof(data),&written,NULL)&&written==sizeof(data));CloseHandle(h);
    char utf8[MAX_PATH*4];assert(WideCharToMultiByte(CP_UTF8,0,file,-1,utf8,sizeof(utf8),NULL,NULL));
    stop=ec_stop_new();assert(ec_io_open(&io,utf8,stop)==0);assert(ec_io_read(&io,data,64)==64);assert(ec_io_read(&io,data,1)==0);assert(ec_io_seek(&io,-1,SEEK_SET)<0);assert(ec_io_seek(&io,0,SEEK_SET)==0);
    h=CreateFileW(file,GENERIC_WRITE,FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE,NULL,OPEN_EXISTING,0,NULL);assert(h!=INVALID_HANDLE_VALUE);assert(SetEndOfFile(h));CloseHandle(h);
    assert(ec_io_read(&io,data,64)==-EIO);ec_io_close(&io);free(stop);assert(DeleteFileW(file));
    assert(GetProcessHandleCount(GetCurrentProcess(),&after));assert(after<=before+1);
    printf("Cancellable IO: pending read cancelled, EOF distinct from truncation, seek bounds OK; handles %lu -> %lu\n",(unsigned long)before,(unsigned long)after);return 0;
}
