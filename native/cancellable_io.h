/* Owned read-only Windows IO. Keep OVERLAPPED and its buffer alive until the
 * kernel completes a cancelled request. A broken driver can still delay that
 * completion; never free pending IO to pretend a hard timeout was achieved. */
#ifndef EC_CANCELLABLE_IO_H
#define EC_CANCELLABLE_IO_H
#include <windows.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdlib.h>
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>
typedef struct { atomic_int cancelled, io_error; } EC_Cancel;
typedef struct { HANDLE handle,event; int64_t size,position; EC_Cancel *cancel; } EC_FileIO;
static EC_Cancel *ec_stop_new(void) {
    EC_Cancel *s=calloc(1,sizeof(*s));
    if(s){atomic_init(&s->cancelled,0);atomic_init(&s->io_error,0);}return s;
}
static int ec_stopped(EC_Cancel *s){return s&&atomic_load(&s->cancelled);}
static int ec_io_open(EC_FileIO *f,const char *path,EC_Cancel *cancel){
    memset(f,0,sizeof(*f));f->handle=INVALID_HANDLE_VALUE;f->cancel=cancel;
    int n=MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,path,-1,NULL,0);
    wchar_t *wide=n>0?malloc((size_t)n*sizeof(*wide)):NULL;if(!wide)return -1;
    MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,path,-1,wide,n);
    f->handle=CreateFileW(wide,GENERIC_READ,FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE,NULL,OPEN_EXISTING,FILE_FLAG_OVERLAPPED,NULL);free(wide);
    if(f->handle==INVALID_HANDLE_VALUE)return -1;
    LARGE_INTEGER size;if(!GetFileSizeEx(f->handle,&size)||size.QuadPart<0){CloseHandle(f->handle);f->handle=INVALID_HANDLE_VALUE;return -1;}
    f->size=size.QuadPart;f->event=CreateEventW(NULL,TRUE,FALSE,NULL);
    if(!f->event){CloseHandle(f->handle);f->handle=INVALID_HANDLE_VALUE;return -1;}return 0;
}
static void ec_io_close(EC_FileIO *f){if(f->event)CloseHandle(f->event);if(f->handle&&f->handle!=INVALID_HANDLE_VALUE)CloseHandle(f->handle);f->event=NULL;f->handle=INVALID_HANDLE_VALUE;}
static int ec_io_read_at(EC_FileIO *f,void *buffer,int bytes,int64_t offset){
    if(bytes<0||offset<0)return -EINVAL;
    if(ec_stopped(f->cancel))return -ECANCELED;
    if(offset>=f->size||!bytes)return 0;
    OVERLAPPED o={0};o.Offset=(DWORD)(uint64_t)offset;o.OffsetHigh=(DWORD)((uint64_t)offset>>32);o.hEvent=f->event;ResetEvent(f->event);
    DWORD got=0;BOOL ok=ReadFile(f->handle,buffer,(DWORD)bytes,&got,&o);DWORD error=ok?ERROR_SUCCESS:GetLastError();
    if(!ok&&error==ERROR_IO_PENDING){
        ULONGLONG deadline=GetTickCount64()+25000;
        for(;;){DWORD wait=WaitForSingleObject(f->event,50);
            if(wait==WAIT_OBJECT_0){ok=GetOverlappedResult(f->handle,&o,&got,FALSE);error=ok?ERROR_SUCCESS:GetLastError();break;}
            if(wait==WAIT_FAILED||ec_stopped(f->cancel)||GetTickCount64()>=deadline){
                CancelIoEx(f->handle,&o);
                /* Drain kernel ownership before the caller may reuse/free buffer. */
                GetOverlappedResult(f->handle,&o,&got,TRUE);
                error=ec_stopped(f->cancel)?ERROR_OPERATION_ABORTED:ERROR_TIMEOUT;ok=FALSE;break;
            }
        }
    }
    if(!ok&&error!=ERROR_HANDLE_EOF){if(f->cancel)atomic_store(&f->cancel->io_error,(int)error);return error==ERROR_OPERATION_ABORTED?-ECANCELED:-EIO;}
    int64_t expected=f->size-offset;if(expected>bytes)expected=bytes;
    if(got<(uint64_t)expected){if(f->cancel)atomic_store(&f->cancel->io_error,ERROR_READ_FAULT);return -EIO;}
    return (int)got;
}
static int ec_io_read(EC_FileIO *f,void *buffer,int n){int r=ec_io_read_at(f,buffer,n,f->position);if(r>0)f->position+=r;return r;}
static int64_t ec_io_seek(EC_FileIO *f,int64_t offset,int whence){
    int64_t base=whence==SEEK_SET?0:whence==SEEK_CUR?f->position:whence==SEEK_END?f->size:-1;
    if(base<0||(offset>0&&base>INT64_MAX-offset)||(offset<0&&offset< -base))return -1;
    f->position=base+offset;return f->position;
}
#endif
