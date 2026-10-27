/* Enigmax CinePlayer - read-only MVC bridge. GPL-3.0-or-later.
 * Demuxes with FFmpeg, decodes actual eye pairs with edge264-mvc, exposes a
 * bounded virtual raw-video stream. No movie is converted or written to disk.
 * The raw-video transport is CFR: nonuniform timestamps are rejected.
 */
#include "cine_mvc.h"
#include "annexb.h"
#include "edge264mvc.h"
#include "udfread.h"
#include "blockinput.h"
#include <libavformat/avformat.h>
#include <libavcodec/bsf.h>
#include <libavutil/mathematics.h>
#include <windows.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <limits.h>

#define MAX_CLIPS 1024
#define IO_BUFFER 65536
#define MAX_MPLS (2*1024*1024)
#define DECODE_TIMEOUT 25

typedef struct {udfread_block_input api; AVIOContext *io; SRWLOCK lock;} Blocks;
typedef struct {char *path;udfread *udf;Blocks *blocks;int iso;} Source;
typedef struct {Source *source;UDFFILE *udf;AVIOContext *file;int64_t size;} File;
typedef struct {char id[6];double in,out,begin;} Clip;
typedef struct {
    Source source;EC_Info info;Clip *clips;int clip_count,clip;
    AVFormatContext *format;AVIOContext *custom;File *file;
    AVFormatContext *dependent_format;AVIOContext *dependent_custom;File *dependent_file;
    AVPacket *base_pending,*dependent_pending;
    AVPacket *dependent_chunk;
    EC_AnnexB dependent_annexb;
    size_t dependent_offset;
    int dependent_eof;
    int64_t dependent_pts,dependent_dts;
    AVBSFContext *bsf[2];int video[2];int video_count;
    AVPacket *packet,*filtered;size_t nal_pos;
    Edge264MvcDecoder *decoder;
    uint8_t *frame;size_t frame_size,frame_offset;
    int frame_ready,eof,draining;
    double first_pts,current_pts,deadline;int64_t frame_number,position;
    atomic_int cancelled;
    char error[512];
} Engine;

static double now(void){return (double)GetTickCount64()/1000.0;}
static void set_error(char *dst,int cap,const char *message){if(dst&&cap>0){snprintf(dst,(size_t)cap,"%s",message);}}
static int block_read(udfread_block_input *p,uint32_t lba,void *buf,uint32_t n,int flags){
    (void)flags;Blocks *b=(Blocks*)p;if(n>INT_MAX/2048)return 0;
    AcquireSRWLockExclusive(&b->lock);int got=0;
    if(avio_seek(b->io,(int64_t)lba*2048,SEEK_SET)>=0){int r=avio_read(b->io,buf,(int)n*2048);if(r>0)got=r/2048;}
    ReleaseSRWLockExclusive(&b->lock);return got;
}
static uint32_t block_size(udfread_block_input *p){Blocks *b=(Blocks*)p;int64_t s=avio_size(b->io);return s>0&&s/2048<=UINT32_MAX?(uint32_t)(s/2048):0;}
static int block_close(udfread_block_input *p){Blocks *b=(Blocks*)p;avio_closep(&b->io);free(b);return 0;}
static void source_close(Source *s){if(s->udf){udfread_close(s->udf);s->udf=NULL;s->blocks=NULL;}else if(s->blocks){block_close(&s->blocks->api);s->blocks=NULL;}free(s->path);s->path=NULL;}
static int source_open(Source *s,const char *path,char *err,int cap){
    memset(s,0,sizeof(*s));if(!path||strlen(path)>32760){set_error(err,cap,"Ruta no válida");return -1;}
    s->path=strdup(path);if(!s->path)return -1;
    size_t n=strlen(path);s->iso=n>=4&&!_stricmp(path+n-4,".iso");
    if(!s->iso)return 0;
    s->blocks=calloc(1,sizeof(Blocks));if(!s->blocks)goto fail;
    s->blocks->api.read=block_read;s->blocks->api.size=block_size;s->blocks->api.close=block_close;InitializeSRWLock(&s->blocks->lock);
    if(avio_open(&s->blocks->io,path,AVIO_FLAG_READ)<0){set_error(err,cap,"No se pudo abrir la ISO en lectura");goto fail;}
    s->udf=udfread_init();if(!s->udf)goto fail;udfread_set_log_level(s->udf,UDFREAD_LOG_NONE);
    if(udfread_open_input(s->udf,&s->blocks->api)<0){
        /* udfread_open_input owns the callbacks only on success. */
        udfread_close(s->udf);s->udf=NULL;set_error(err,cap,"La imagen no contiene un UDF legible");goto fail;
    }
    return 0;
fail:source_close(s);return -1;
}
static File *file_open(Source *s,const char *relative){
    File *f=calloc(1,sizeof(*f));if(!f)return NULL;f->source=s;
    if(s->iso){f->udf=udfread_file_open(s->udf,relative);if(!f->udf)goto fail;f->size=udfread_file_size(f->udf);}
    else {size_t n=strlen(s->path)+strlen(relative)+2;char *p=malloc(n);if(!p)goto fail;snprintf(p,n,"%s/%s",s->path,relative);int r=avio_open(&f->file,p,AVIO_FLAG_READ);free(p);if(r<0)goto fail;f->size=avio_size(f->file);}
    return f;
fail:free(f);return NULL;
}
static void file_close(File *f){if(!f)return;if(f->udf)udfread_file_close(f->udf);if(f->file)avio_closep(&f->file);free(f);}
static int file_read(File *f,uint8_t *buf,int size){if(f->udf){ssize_t r=udfread_file_read(f->udf,buf,(size_t)size);return r>0?(int)r:AVERROR_EOF;}return avio_read(f->file,buf,size);}
static int64_t file_seek(File *f,int64_t offset,int whence){if(whence==AVSEEK_SIZE)return f->size;whence&=~AVSEEK_FORCE;if(f->udf)return udfread_file_seek(f->udf,offset,whence);return avio_seek(f->file,offset,whence);}
static int avio_read_cb(void *o,uint8_t *buf,int n){return file_read(o,buf,n);}
EC_API int ec_disc_read(const char *path,const char *relative,void *buffer,int capacity){
    if(!relative||strstr(relative,"..")||relative[0]=='/'||relative[0]=='\\'||strchr(relative,':')||!buffer||capacity<1||capacity>16*1024*1024)return -1;
    Source s;char error[128];if(source_open(&s,path,error,sizeof(error))<0)return -1;File *f=file_open(&s,relative);int total=0;
    if(f){while(total<capacity){int r=file_read(f,(uint8_t*)buffer+total,capacity-total);if(r<=0)break;total+=r;}file_close(f);}else total=-1;source_close(&s);return total;
}
static int64_t avio_seek_cb(void *o,int64_t off,int whence){return file_seek(o,off,whence);}
static uint16_t be16(const uint8_t *p){return (uint16_t)(p[0]<<8|p[1]);}
static uint32_t be32(const uint8_t *p){return (uint32_t)p[0]<<24|(uint32_t)p[1]<<16|(uint32_t)p[2]<<8|p[3];}
static int playlist(Source *s,int id,Clip **clips,int *count,EC_Title *info){
    char name[80];snprintf(name,sizeof(name),"BDMV/PLAYLIST/%05d.mpls",id);File *f=file_open(s,name);if(!f)return -1;
    if(f->size<32||f->size>MAX_MPLS){file_close(f);return -1;}size_t n=(size_t)f->size;uint8_t *buf=malloc(n);if(!buf){file_close(f);return -1;}
    size_t pos=0;while(pos<n){int r=file_read(f,buf+pos,(int)(n-pos));if(r<=0)break;pos+=(size_t)r;}file_close(f);
    int result=-1;Clip *out=NULL;if(pos!=n||memcmp(buf,"MPLS",4))goto done;
    size_t start=be32(buf+8);if(start>n||n-start<10)goto done;
    unsigned number=be16(buf+start+6);if(number==0||number>MAX_CLIPS)goto done;
    out=calloc(number,sizeof(Clip));if(!out)goto done;pos=start+10;double duration=0.0;int stereo=0;
    for(unsigned i=0;i<number;i++){
        if(pos>n||n-pos<2)goto done;size_t len=be16(buf+pos);pos+=2;if(len<20||pos>n||len>n-pos)goto done;
        for(int j=0;j<5;j++){if(buf[pos+j]<'0'||buf[pos+j]>'9')goto done;out[i].id[j]=(char)buf[pos+j];}
        out[i].in=(double)be32(buf+pos+12)/45000.0;out[i].out=(double)be32(buf+pos+16)/45000.0;out[i].begin=duration;
        if(out[i].out<=out[i].in)goto done;duration+=out[i].out-out[i].in;
        if(i==0){snprintf(name,sizeof(name),"BDMV/STREAM/SSIF/%s.ssif",out[i].id);File *ssif=file_open(s,name);if(ssif){stereo=1;file_close(ssif);}}
        pos+=len;
    }
    int repeated=0;
    for(unsigned i=0;i<number&&!repeated;i++)for(unsigned j=0;j<i;j++)
        if(!strcmp(out[i].id,out[j].id)&&out[i].in==out[j].in&&out[i].out==out[j].out){repeated=1;break;}
    if(info){*info=(EC_Title){(uint32_t)id,number,duration,stereo,repeated};}
    if(clips){*clips=out;out=NULL;}if(count)*count=(int)number;result=0;
done:free(out);free(buf);return result;
}
EC_API int ec_disc_titles(const char *path,EC_Title *titles,int cap,char *error,int error_size){
    if(!titles||cap<1||cap>10000)return -1;Source s;if(source_open(&s,path,error,error_size)<0)return -1;int count=0;
    if(s.iso){UDFDIR *dir=udfread_opendir(s.udf,"BDMV/PLAYLIST");if(!dir){set_error(error,error_size,"No existe BDMV/PLAYLIST en la ISO");source_close(&s);return -1;}struct udfread_dirent entry;while(udfread_readdir(dir,&entry)&&count<cap){
            if(strlen(entry.d_name)!=10||_stricmp(entry.d_name+5,".mpls"))continue;char *end=NULL;long id=strtol(entry.d_name,&end,10);if(end!=entry.d_name+5||id<0||id>99999)continue;
            if(playlist(&s,(int)id,NULL,NULL,&titles[count])==0)count++;
        }udfread_closedir(dir);
    }else{
        /* UTF-8-safe enumeration of a selected folder via FindFirstFileW. */
        size_t bytes=strlen(path)+32;char *p=malloc(bytes);if(!p){source_close(&s);return -1;}snprintf(p,bytes,"%s/BDMV/PLAYLIST/*.mpls",path);
        int len=MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,p,-1,NULL,0);wchar_t *wp=len>0?malloc((size_t)len*sizeof(wchar_t)):NULL;free(p);
        if(wp){p=malloc(bytes);if(!p){free(wp);source_close(&s);return -1;}snprintf(p,bytes,"%s/BDMV/PLAYLIST/*.mpls",path);MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,p,-1,wp,len);free(p);WIN32_FIND_DATAW data;HANDLE h=FindFirstFileW(wp,&data);free(wp);if(h!=INVALID_HANDLE_VALUE){do{wchar_t *end=NULL;long id=wcstol(data.cFileName,&end,10);if(end==data.cFileName+5&&!_wcsicmp(end,L".mpls")&&id>=0&&id<=99999&&playlist(&s,(int)id,NULL,NULL,&titles[count])==0)count++;}while(count<cap&&FindNextFileW(h,&data));FindClose(h);}}
    }
    source_close(&s);if(!count)set_error(error,error_size,"No hay títulos Blu-ray válidos");return count;
}
static int interrupted(void *p){Engine *e=p;return atomic_load(&e->cancelled)||now()>e->deadline;}
static void dependent_reset(Engine *e){
    ec_annexb_reset(&e->dependent_annexb);e->dependent_offset=0;e->dependent_eof=0;
    e->dependent_pts=e->dependent_dts=AV_NOPTS_VALUE;
    if(e->dependent_chunk)av_packet_unref(e->dependent_chunk);
}
static void input_close(Engine *e){for(int i=0;i<2;i++)av_bsf_free(&e->bsf[i]);avformat_close_input(&e->format);avformat_close_input(&e->dependent_format);if(e->custom){av_freep(&e->custom->buffer);avio_context_free(&e->custom);}if(e->dependent_custom){av_freep(&e->dependent_custom->buffer);avio_context_free(&e->dependent_custom);}file_close(e->file);file_close(e->dependent_file);e->file=NULL;e->dependent_file=NULL;if(e->packet)av_packet_unref(e->packet);if(e->filtered)av_packet_unref(e->filtered);if(e->base_pending)av_packet_unref(e->base_pending);if(e->dependent_pending)av_packet_unref(e->dependent_pending);e->nal_pos=0;e->eof=0;e->draining=0;}
static int open_input(Engine *e){
    dependent_reset(e);
    input_close(e);e->format=avformat_alloc_context();if(!e->format)return -1;e->format->interrupt_callback=(AVIOInterruptCB){interrupted,e};e->format->probesize=8*1024*1024;e->format->max_analyze_duration=5*AV_TIME_BASE;
    const char *path=e->source.path;char name[96];
    if(e->clip_count){snprintf(name,sizeof(name),"BDMV/STREAM/%s.m2ts",e->clips[e->clip].id);e->file=file_open(&e->source,name);if(!e->file){snprintf(e->error,sizeof(e->error),"Falta la vista base del clip %s",e->clips[e->clip].id);return -1;}
        uint8_t *buf=av_malloc(IO_BUFFER);if(!buf)return -1;e->custom=avio_alloc_context(buf,IO_BUFFER,0,e->file,avio_read_cb,NULL,avio_seek_cb);if(!e->custom){av_free(buf);return -1;}e->format->pb=e->custom;e->format->flags|=AVFMT_FLAG_CUSTOM_IO;path=NULL;
    }
    if(avformat_open_input(&e->format,path,NULL,NULL)<0||avformat_find_stream_info(e->format,NULL)<0){snprintf(e->error,sizeof(e->error),"No se pudo analizar el flujo MVC");return -1;}
    e->video_count=0;
    for(unsigned i=0;i<e->format->nb_streams&&e->video_count<2;i++){
        AVStream *st=e->format->streams[i];if(st->codecpar->codec_id!=AV_CODEC_ID_H264&&st->id!=0x1012)continue;
        int n=e->video_count;e->video[n]=(int)i;const AVBitStreamFilter *filter=av_bsf_get_by_name("h264_mp4toannexb");if(!filter||av_bsf_alloc(filter,&e->bsf[n])<0)return -1;
        if(avcodec_parameters_copy(e->bsf[n]->par_in,st->codecpar)<0)return -1;e->bsf[n]->par_in->codec_id=AV_CODEC_ID_H264;e->bsf[n]->par_in->codec_type=AVMEDIA_TYPE_VIDEO;e->bsf[n]->time_base_in=st->time_base;
        if(av_bsf_init(e->bsf[n])<0)return -1;e->video_count++;
    }
    if(!e->video_count){snprintf(e->error,sizeof(e->error),"MVC requiere una vista base H.264");return -1;}
    if(e->clip_count){
        snprintf(name,sizeof(name),"BDMV/STREAM/SSIF/%s.ssif",e->clips[e->clip].id);e->dependent_file=file_open(&e->source,name);if(!e->dependent_file){snprintf(e->error,sizeof(e->error),"Falta la vista dependiente SSIF");return -1;}
        uint8_t *buf=av_malloc(IO_BUFFER);if(!buf)return -1;e->dependent_custom=avio_alloc_context(buf,IO_BUFFER,0,e->dependent_file,avio_read_cb,NULL,avio_seek_cb);if(!e->dependent_custom){av_free(buf);return -1;}
        e->dependent_format=avformat_alloc_context();if(!e->dependent_format)return -1;e->dependent_format->pb=e->dependent_custom;e->dependent_format->flags|=AVFMT_FLAG_CUSTOM_IO|AVFMT_FLAG_NOPARSE|AVFMT_FLAG_NOFILLIN;e->dependent_format->interrupt_callback=(AVIOInterruptCB){interrupted,e};
        if(avformat_open_input(&e->dependent_format,NULL,av_find_input_format("mpegts"),NULL)<0)return -1;
        int found=0;for(int attempts=0;attempts<50000&&!interrupted(e);attempts++){
            if(av_read_frame(e->dependent_format,e->dependent_pending)<0)break;AVStream *st=e->dependent_format->streams[e->dependent_pending->stream_index];
            if(st->id==0x1012){e->video[1]=e->dependent_pending->stream_index;const AVBitStreamFilter *filter=av_bsf_get_by_name("h264_mp4toannexb");if(!filter||av_bsf_alloc(filter,&e->bsf[1])<0)return -1;if(avcodec_parameters_copy(e->bsf[1]->par_in,st->codecpar)<0)return -1;e->bsf[1]->par_in->codec_id=AV_CODEC_ID_H264;e->bsf[1]->par_in->codec_type=AVMEDIA_TYPE_VIDEO;e->bsf[1]->time_base_in=st->time_base;if(av_bsf_init(e->bsf[1])<0)return -1;e->video_count=2;found=1;break;}av_packet_unref(e->dependent_pending);
        }
        if(!found){snprintf(e->error,sizeof(e->error),"No se encontró el PID MVC dependiente 0x1012 en el SSIF");return -1;}
        av_packet_move_ref(e->dependent_chunk,e->dependent_pending);
        e->dependent_pts=e->dependent_chunk->pts;e->dependent_dts=e->dependent_chunk->dts;
    }
    AVStream *v=e->format->streams[e->video[0]];AVRational fps=av_guess_frame_rate(e->format,v,NULL);
    if(fps.num<=0||fps.den<=0||av_q2d(fps)<1||av_q2d(fps)>240){snprintf(e->error,sizeof(e->error),"No se conoce la frecuencia fija de este MVC");return -1;}
    if(e->info.fps_num&&(e->info.fps_num!=fps.num||e->info.fps_den!=fps.den)){snprintf(e->error,sizeof(e->error),"El título cambia de frecuencia entre clips");return -1;}
    e->info.fps_num=fps.num;e->info.fps_den=fps.den;
    if(!e->clip_count){if(v->duration!=AV_NOPTS_VALUE)e->info.duration=v->duration*av_q2d(v->time_base);else if(e->format->duration!=AV_NOPTS_VALUE)e->info.duration=(double)e->format->duration/AV_TIME_BASE;}
    return 0;
}
/* MPEG-TS splits a long MVC slice over several PES packets. NOPARSE is
 * necessary for type-20 MVC slices, but leaves these fragments unassembled.
 * Read a complete Annex B NAL before interleaving it with the base view. */
static int next_dependent_nal(Engine *e,AVPacket *packet){
    EC_Nal nal;int complete=0;
    for(;;){
        if(interrupted(e))return -1;
        if(e->dependent_offset<(size_t)e->dependent_chunk->size){
            size_t used=0;
            complete=ec_annexb_push(&e->dependent_annexb,e->dependent_chunk->data+e->dependent_offset,(size_t)e->dependent_chunk->size-e->dependent_offset,&used,e->dependent_pts,e->dependent_dts,&nal);
            e->dependent_offset+=used;
            if(complete<0){snprintf(e->error,sizeof(e->error),"NAL MVC demasiado grande o memoria insuficiente");return -1;}
            if(complete)break;
        }else if(e->dependent_eof){
            complete=ec_annexb_finish(&e->dependent_annexb,&nal);if(!complete)return 0;break;
        }else{
            av_packet_unref(e->dependent_chunk);e->dependent_offset=0;
            int r;do{
                r=av_read_frame(e->dependent_format,e->dependent_chunk);
                if(r<0)break;
                if(e->dependent_chunk->stream_index==e->video[1])break;
                av_packet_unref(e->dependent_chunk);
            }while(!interrupted(e));
            if(r<0){if(r!=AVERROR_EOF){snprintf(e->error,sizeof(e->error),"Error al leer la vista MVC dependiente (%d)",r);return -1;}e->dependent_eof=1;continue;}
            if(e->dependent_chunk->pts!=AV_NOPTS_VALUE)e->dependent_pts=e->dependent_chunk->pts;
            if(e->dependent_chunk->dts!=AV_NOPTS_VALUE)e->dependent_dts=e->dependent_chunk->dts;
        }
    }
    if(av_new_packet(packet,(int)nal.size+4)<0)return -1;
    memcpy(packet->data,"\0\0\0\1",4);memcpy(packet->data+4,nal.data,nal.size);
    packet->pts=nal.pts;packet->dts=nal.dts;packet->stream_index=e->video[1];return 1;
}
static int next_packet(Engine *e){
    for(;;){if(interrupted(e)){snprintf(e->error,sizeof(e->error),"Lectura MVC cancelada o agotada");return -1;}
        int r,n=-1;AVFormatContext *packet_format=e->format;
        if(e->dependent_format){
            if(!e->base_pending->size){while((r=av_read_frame(e->format,e->base_pending))>=0){if(e->base_pending->stream_index==e->video[0])break;av_packet_unref(e->base_pending);}}
            if(!e->dependent_pending->size&&next_dependent_nal(e,e->dependent_pending)<0)return -1;
            if(!e->base_pending->size&&!e->dependent_pending->size){e->eof=1;edge264mvc_send_end(e->decoder);return 0;}
            double base_time=INFINITY,dependent_time=INFINITY;
            if(e->base_pending->size){AVPacket *p=e->base_pending;int64_t t=p->dts!=AV_NOPTS_VALUE?p->dts:p->pts;base_time=t*av_q2d(e->format->streams[p->stream_index]->time_base);}
            if(e->dependent_pending->size){AVPacket *p=e->dependent_pending;int64_t t=p->dts!=AV_NOPTS_VALUE?p->dts:p->pts;dependent_time=t*av_q2d(e->dependent_format->streams[p->stream_index]->time_base);}
            if(base_time<=dependent_time&&e->base_pending->size){n=0;av_packet_move_ref(e->packet,e->base_pending);}else{n=1;packet_format=e->dependent_format;av_packet_move_ref(e->packet,e->dependent_pending);}
        }else{
            r=av_read_frame(e->format,e->packet);if(r<0){e->eof=1;edge264mvc_send_end(e->decoder);return 0;}
            for(int i=0;i<e->video_count;i++)if(e->packet->stream_index==e->video[i]){n=i;break;}
        }
        if(n<0){av_packet_unref(e->packet);continue;}
        AVStream *stream=packet_format->streams[e->video[n]];
        int64_t pts=e->packet->pts;if(pts==AV_NOPTS_VALUE)pts=e->packet->dts;
        e->current_pts=pts==AV_NOPTS_VALUE?NAN:(double)pts*av_q2d(stream->time_base);
        if(av_bsf_send_packet(e->bsf[n],e->packet)<0){av_packet_unref(e->packet);continue;}
        av_packet_unref(e->filtered);r=av_bsf_receive_packet(e->bsf[n],e->filtered);if(r==AVERROR(EAGAIN))continue;if(r<0)return -1;
        e->nal_pos=0;return 1;
    }
}
static int consume_nal(Engine *e){
    if(e->nal_pos>=(size_t)e->filtered->size){av_packet_unref(e->filtered);int r=next_packet(e);if(r<=0)return r;}
    uint8_t *data=e->filtered->data;size_t size=(size_t)e->filtered->size,pos=e->nal_pos;
    while(pos<size&&data[pos]==0)pos++;if(pos<size&&data[pos]==1)pos++;
    size_t end=pos;while(end+3<size&&!(data[end]==0&&data[end+1]==0&&(data[end+2]==1||(data[end+2]==0&&data[end+3]==1))))end++;
    if(end+3>=size)end=size;if(end<=pos){e->nal_pos=size;return 1;}
    int64_t pts=isnan(e->current_pts)?INT64_MIN:(int64_t)llround(e->current_pts*1000000.0);
    int r=edge264mvc_send_nal(e->decoder,data+pos,end-pos,pts,0);
    if(r==EDGE264MVC_AGAIN)return 1;
    if(r==EDGE264MVC_NOMEM||r==EDGE264MVC_INVALID){snprintf(e->error,sizeof(e->error),"El decodificador MVC no pudo continuar (%d)",r);return -1;}
    e->nal_pos=end;return 1;
}
static int render_frame(Engine *e,Edge264MvcFrame *f){
    if(!f->views[0].planes[0]||!f->views[1].planes[0]){snprintf(e->error,sizeof(e->error),"No se ha recuperado el segundo ojo: no se presentará 2D como MVC 3D");return -1;}
    if(f->width_Y<2||f->height_Y<2||f->width_Y>8192||f->height_Y>4352||f->width_C*2!=f->width_Y||f->height_C*2!=f->height_Y||(f->bit_depth_Y!=8&&f->bit_depth_Y!=10)||f->bit_depth_C!=f->bit_depth_Y){snprintf(e->error,sizeof(e->error),"Este transporte MVC admite YUV 4:2:0 de 8/10 bits y dimensiones constantes");return -1;}
    int width=f->width_Y*2,height=f->height_Y,bytes=f->bit_depth_Y>8?2:1;
    if(e->info.width&&(e->info.width!=width||e->info.height!=height||e->info.format!=bytes-1)){snprintf(e->error,sizeof(e->error),"El flujo MVC cambia de tamaño o profundidad");return -1;}
    e->info.width=width;e->info.height=height;e->info.format=bytes-1;e->info.paired=1;
    size_t total=(size_t)width*height*3/2*bytes;if(!e->frame){e->frame=malloc(total);if(!e->frame)return -1;e->frame_size=total;}
    uint8_t *dst=e->frame;
    for(int plane=0;plane<3;plane++){
        int w=plane?f->width_C:f->width_Y,h=plane?f->height_C:f->height_Y,stride=plane?f->stride_C:f->stride_Y;
        for(int y=0;y<h;y++){memcpy(dst,f->views[0].planes[plane]+(size_t)y*stride,(size_t)w*bytes);dst+=(size_t)w*bytes;memcpy(dst,f->views[1].planes[plane]+(size_t)y*stride,(size_t)w*bytes);dst+=(size_t)w*bytes;}
    }
    e->frame_offset=0;e->frame_ready=1;return 0;
}
static int decode_next(Engine *e,double target){
    e->deadline=now()+DECODE_TIMEOUT;
    for(int attempts=0;attempts<200000;attempts++){
        if(interrupted(e))return -1;Edge264MvcFrame frame;int r=edge264mvc_receive_frame(e->decoder,&frame);
        if(r==EDGE264MVC_OK){
            double pts=frame.views[0].pts==INT64_MIN?NAN:(double)frame.views[0].pts/1000000.0;
            double relative;if(e->clip_count){relative=e->clips[e->clip].begin+(pts-e->clips[e->clip].in);}else{if(isnan(e->first_pts))e->first_pts=isnan(pts)?0:pts;relative=isnan(pts)?e->frame_number*(double)e->info.fps_den/e->info.fps_num:pts-e->first_pts;}
            if(e->clip_count&&!isnan(pts)&&pts>=e->clips[e->clip].out-0.00001){edge264mvc_release_frame(e->decoder,&frame);if(e->clip+1<e->clip_count){e->clip++;edge264mvc_flush(e->decoder);if(open_input(e)<0)return -1;continue;}return 0;}
            if(relative+0.45*(double)e->info.fps_den/e->info.fps_num<target){edge264mvc_release_frame(e->decoder,&frame);continue;}
            if(target>=0&&!isnan(relative)&&fabs(relative-target)>1.55*(double)e->info.fps_den/e->info.fps_num){edge264mvc_release_frame(e->decoder,&frame);snprintf(e->error,sizeof(e->error),"MVC discontinuo: fotograma %.6f, solicitado %.6f",relative,target);return -1;}
            int ok=render_frame(e,&frame);edge264mvc_release_frame(e->decoder,&frame);if(ok<0)return -1;e->frame_number++;return 1;
        }
        if(r==EDGE264MVC_END){if(e->clip+1<e->clip_count){e->clip++;edge264mvc_flush(e->decoder);if(open_input(e)<0)return -1;continue;}return 0;}
        if(r!=EDGE264MVC_AGAIN)return -1;
        if(e->eof)return 0;
        if(consume_nal(e)<0)return -1;
    }
    snprintf(e->error,sizeof(e->error),"Demasiados paquetes sin obtener un par MVC válido");return -1;
}
EC_API void ec_mvc_close(void *context){Engine *e=context;if(!e)return;input_close(e);edge264mvc_close(&e->decoder);av_packet_free(&e->packet);av_packet_free(&e->filtered);av_packet_free(&e->base_pending);av_packet_free(&e->dependent_pending);av_packet_free(&e->dependent_chunk);free(e->dependent_annexb.data);free(e->frame);free(e->clips);source_close(&e->source);free(e);}
EC_API void *ec_mvc_open(const char *path,int title,int threads,EC_Info *info,char *err,int cap){
    Engine *e=calloc(1,sizeof(*e));if(!e)return NULL;e->first_pts=NAN;e->deadline=now()+DECODE_TIMEOUT;atomic_init(&e->cancelled,0);e->packet=av_packet_alloc();e->filtered=av_packet_alloc();e->base_pending=av_packet_alloc();e->dependent_pending=av_packet_alloc();e->dependent_chunk=av_packet_alloc();
    if(!e->packet||!e->filtered||!e->base_pending||!e->dependent_pending||!e->dependent_chunk||source_open(&e->source,path,e->error,sizeof(e->error))<0)goto fail;
    size_t len=strlen(path);int disc=e->source.iso||(len&&path[len-1]=='/');
    if(!disc){char p[32768];snprintf(p,sizeof(p),"%s/BDMV/index.bdmv",path);AVIOContext *test=NULL;if(avio_open(&test,p,AVIO_FLAG_READ)>=0){disc=1;avio_closep(&test);}}
    if(disc){
        if(title<0){EC_Title *list=calloc(4096,sizeof(*list));if(!list)goto fail;int n=ec_disc_titles(path,list,4096,e->error,sizeof(e->error));double best=-1;for(int i=0;i<n;i++)if(list[i].stereo&&!list[i].reserved&&list[i].duration>best){title=(int)list[i].playlist;best=list[i].duration;}free(list);}
        EC_Title t;if(title<0||playlist(&e->source,title,&e->clips,&e->clip_count,&t)<0||!t.stereo){snprintf(e->error,sizeof(e->error),"El título no contiene un SSIF 3D accesible");goto fail;}e->info.duration=t.duration;e->info.playlist=title;e->info.clips=e->clip_count;
    }else{e->info.playlist=-1;}
    Edge264MvcSettings settings;edge264mvc_default_settings(&settings);settings.n_threads=threads>0&&threads<=16?threads:4;
    if(edge264mvc_open(&e->decoder,&settings)!=EDGE264MVC_OK)goto fail;
    if(open_input(e)<0||decode_next(e,0)<1)goto fail;
    if(!e->clip_count&&e->format->start_time!=AV_NOPTS_VALUE&&!isnan(e->first_pts))e->info.audio_delay=(double)e->format->start_time/AV_TIME_BASE-e->first_pts;
    if(e->info.duration>0)e->info.frames=(int64_t)llround(e->info.duration*e->info.fps_num/e->info.fps_den);
    if(info)*info=e->info;return e;
fail:if(!e->error[0])snprintf(e->error,sizeof(e->error),"No se pudo iniciar MVC");set_error(err,cap,e->error);ec_mvc_close(e);return NULL;
}
EC_API int64_t ec_mvc_read(void *context,void *buffer,uint64_t n){Engine *e=context;if(!e||!buffer||atomic_load(&e->cancelled))return -1;if(n>1024*1024)n=1024*1024;if(n==0)return 0;
    if(e->info.frames>0&&e->position>=e->info.frames*(int64_t)e->frame_size)return 0;
    if(!e->frame_ready){double target=(double)(e->position/(int64_t)e->frame_size)*e->info.fps_den/e->info.fps_num;int r=decode_next(e,target);if(r<=0)return r;}
    size_t available=e->frame_size-e->frame_offset,take=(size_t)n<available?(size_t)n:available;memcpy(buffer,e->frame+e->frame_offset,take);e->position+=(int64_t)take;e->frame_offset+=take;if(e->frame_offset==e->frame_size)e->frame_ready=0;return (int64_t)take;
}
EC_API int64_t ec_mvc_size(void *context){Engine *e=context;if(!e||e->info.frames<=0||e->info.frames>INT64_MAX/(int64_t)e->frame_size)return -10;return e->info.frames*(int64_t)e->frame_size;}
EC_API int64_t ec_mvc_seek(void *context,int64_t offset){Engine *e=context;if(!e||offset<0||atomic_load(&e->cancelled))return -20;if(offset==e->position)return offset;
    int64_t index=offset/(int64_t)e->frame_size;size_t within=(size_t)(offset%(int64_t)e->frame_size);
    if(e->info.frames>0&&index>=e->info.frames){e->position=offset;e->frame_ready=0;e->eof=1;return offset;}
    double target=(double)index*e->info.fps_den/e->info.fps_num;e->deadline=now()+DECODE_TIMEOUT;
    if(e->clip_count){int clip=0;while(clip+1<e->clip_count&&e->clips[clip+1].begin<=target)clip++;if(clip!=e->clip){e->clip=clip;if(open_input(e)<0)return -20;}}
    double source_target=e->clip_count?target-e->clips[e->clip].begin+e->clips[e->clip].in:target+(isnan(e->first_pts)?0:e->first_pts); source_target=fmax(e->clip_count?e->clips[e->clip].in:0.0,source_target-5.0);
    if(e->clip_count&&source_target<=e->clips[e->clip].in){if(open_input(e)<0)return -20;goto reset_decoder;}
    AVStream *stream=e->format->streams[e->video[0]];int64_t ts=(int64_t)floor(source_target/av_q2d(stream->time_base));
    if(av_seek_frame(e->format,e->video[0],ts,AVSEEK_FLAG_BACKWARD)<0){snprintf(e->error,sizeof(e->error),"Este MVC no admite búsqueda en el contenedor");return -20;}
    if(e->dependent_format){AVStream *st=e->dependent_format->streams[e->video[1]];int64_t dep_ts=(int64_t)floor(source_target/av_q2d(st->time_base));if(av_seek_frame(e->dependent_format,e->video[1],dep_ts,AVSEEK_FLAG_BACKWARD)<0){snprintf(e->error,sizeof(e->error),"No se pudo buscar la vista MVC dependiente");return -20;}av_packet_unref(e->base_pending);av_packet_unref(e->dependent_pending);dependent_reset(e);}
reset_decoder:
    for(int i=0;i<e->video_count;i++)av_bsf_flush(e->bsf[i]);edge264mvc_flush(e->decoder);av_packet_unref(e->packet);av_packet_unref(e->filtered);e->nal_pos=0;e->eof=0;e->frame_ready=0;e->frame_number=index;
    if(decode_next(e,target)<1)return -20;e->frame_offset=within;e->position=offset;return offset;
}
EC_API void ec_mvc_cancel(void *context){Engine *e=context;if(e)atomic_store(&e->cancelled,1);}
EC_API const char *ec_mvc_error(void *context){Engine *e=context;return e?e->error:"MVC cerrado";}

