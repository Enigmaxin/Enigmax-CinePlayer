#ifndef CINE_MVC_H
#define CINE_MVC_H
#include <stdint.h>
#include <stddef.h>
#ifdef _WIN32
#define EC_API __declspec(dllexport)
#else
#define EC_API __attribute__((visibility("default")))
#endif
typedef struct {
    int32_t width,height,format,fps_num,fps_den;
    int32_t playlist,clips,paired;
    int64_t frames;
    double duration,audio_delay;
} EC_Info;
typedef struct {uint32_t playlist,clips;double duration;int32_t stereo,reserved;} EC_Title;
EC_API int ec_disc_titles(const char *path, EC_Title *titles,int capacity,char *error,int error_size);
EC_API int ec_disc_read(const char *path,const char *relative,void *buffer,int capacity);
EC_API void *ec_mvc_open(const char *path,int playlist,int threads,EC_Info *info,char *error,int error_size);
EC_API int64_t ec_mvc_read(void *context,void *buffer,uint64_t bytes);
EC_API int64_t ec_mvc_seek(void *context,int64_t offset);
EC_API int64_t ec_mvc_size(void *context);
EC_API void ec_mvc_cancel(void *context);
EC_API const char *ec_mvc_error(void *context);
EC_API void ec_mvc_close(void *context);
/* Clear, single-clip UDF fallback when stale AACS metadata rejects bd://. */
EC_API void *ec_disc_stream_open(const char*,int,int,EC_Info*,char*,int);
EC_API int64_t ec_disc_stream_read(void*,void*,uint64_t);
EC_API int64_t ec_disc_stream_seek(void*,int64_t);
EC_API int64_t ec_disc_stream_size(void*);
EC_API void ec_disc_stream_cancel(void*);
EC_API const char *ec_disc_stream_error(void*);
EC_API void ec_disc_stream_close(void*);
#endif
