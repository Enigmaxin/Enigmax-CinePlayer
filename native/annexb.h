#ifndef ENIGMAX_ANNEXB_H
#define ENIGMAX_ANNEXB_H
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

/* A PES packet is not a NAL boundary. Keep the unfinished NAL, including
 * start codes split across packets; returned bytes live until the next call. */
#define EC_NAL_LIMIT ((size_t)64*1024*1024)
typedef struct {
    uint8_t *data;
    size_t size,capacity,zeros;
    int started,ready;
    int64_t pts,dts;
} EC_AnnexB;
typedef struct {const uint8_t *data;size_t size;int64_t pts,dts;} EC_Nal;
static void ec_annexb_reset(EC_AnnexB *b){b->size=b->zeros=0;b->started=b->ready=0;}
static int ec_annexb_append(EC_AnnexB *b,uint8_t value){
    if(b->zeros>=EC_NAL_LIMIT||b->size>EC_NAL_LIMIT-b->zeros-1)return -1;
    size_t need=b->size+b->zeros+1;
    if(need>b->capacity){
        size_t cap=b->capacity?b->capacity:4096;
        while(cap<need)cap=cap>EC_NAL_LIMIT/2?EC_NAL_LIMIT:cap*2;
        uint8_t *p=realloc(b->data,cap);if(!p)return -1;b->data=p;b->capacity=cap;
    }
    memset(b->data+b->size,0,b->zeros);b->size+=b->zeros;b->zeros=0;
    b->data[b->size++]=value;return 0;
}
static int ec_annexb_push(EC_AnnexB *b,const uint8_t *src,size_t size,size_t *used,int64_t pts,int64_t dts,EC_Nal *nal){
    *used=0;
    if(b->ready){b->size=0;b->ready=0;}
    for(size_t i=0;i<size;i++){
        uint8_t value=src[i];*used=i+1;
        if(value==0){if(b->zeros==EC_NAL_LIMIT)return -1;b->zeros++;continue;}
        if(value==1&&b->zeros>=2){
            b->zeros=0;
            if(b->started&&b->size){
                *nal=(EC_Nal){b->data,b->size,b->pts,b->dts};
                b->pts=pts;b->dts=dts;b->ready=1;return 1;
            }
            b->started=1;b->pts=pts;b->dts=dts;
        }else if(b->started){
            if(ec_annexb_append(b,value)<0)return -1;
        }else{b->zeros=0;}
    }
    return 0;
}
static int ec_annexb_finish(EC_AnnexB *b,EC_Nal *nal){
    if(b->ready){b->size=0;b->ready=0;}
    b->zeros=0;
    if(!b->started||!b->size)return 0;
    *nal=(EC_Nal){b->data,b->size,b->pts,b->dts};b->started=0;b->ready=1;return 1;
}
#endif
