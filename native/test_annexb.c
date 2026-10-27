#include "annexb.h"
#include <assert.h>
#include <stdio.h>

static const uint8_t stream[]={0,0,0,1,0x74,0x42,0,0,3,1,0x51,0,0,1,0x68,0x80,0,0,0,1,0x14,0x81,0x50,0,0};
static const uint8_t first[]={0x74,0x42,0,0,3,1,0x51};
static const uint8_t second[]={0x68,0x80};
static const uint8_t third[]={0x14,0x81,0x50};
static void check(const EC_Nal *n,int i){
    const uint8_t *expected[]={first,second,third};
    size_t lengths[]={sizeof(first),sizeof(second),sizeof(third)};
    assert(i<3&&n->size==lengths[i]&&!memcmp(n->data,expected[i],n->size));
    assert(n->pts==100&&n->dts==90);
}
int main(void){
    for(size_t chunk=1;chunk<=sizeof(stream);chunk++){
        EC_AnnexB b={0};EC_Nal n;int count=0;
        for(size_t p=0;p<sizeof(stream);){
            size_t end=p+chunk;if(end>sizeof(stream))end=sizeof(stream);
            while(p<end){size_t used;int r=ec_annexb_push(&b,stream+p,end-p,&used,100,90,&n);assert(r>=0&&used);p+=used;if(r)check(&n,count++);}
        }
        if(ec_annexb_finish(&b,&n))check(&n,count++);
        assert(count==3&&!ec_annexb_finish(&b,&n));
        /* A seek discards an unfinished slice, but retains the allocation. */
        ec_annexb_reset(&b);size_t used;
        assert(ec_annexb_push(&b,stream,8,&used,1,1,&n)==0);
        ec_annexb_reset(&b);count=0;
        for(size_t p=0;p<sizeof(stream);){int r=ec_annexb_push(&b,stream+p,sizeof(stream)-p,&used,100,90,&n);assert(r>=0&&used);p+=used;if(r)check(&n,count++);}
        if(ec_annexb_finish(&b,&n))check(&n,count++);
        assert(count==3);free(b.data);
    }
    /* Long slices survive several PES-sized chunks byte for byte. */
    EC_AnnexB b={0};EC_Nal n;size_t used;uint8_t start[]={0,0,1,0x74};
    assert(ec_annexb_push(&b,start,sizeof(start),&used,8,7,&n)==0);
    uint8_t payload[65536];memset(payload,0x55,sizeof(payload));
    for(int i=0;i<5;i++)assert(ec_annexb_push(&b,payload,sizeof(payload),&used,8,7,&n)==0);
    uint8_t next[]={0,0,0,1,0x68,0x80};
    assert(ec_annexb_push(&b,next,sizeof(next),&used,9,8,&n)==1);
    assert(n.size==1+5*sizeof(payload)&&n.data[0]==0x74&&n.pts==8&&n.dts==7);
    for(size_t i=1;i<n.size;i++)assert(n.data[i]==0x55);
    size_t offset=used;assert(ec_annexb_push(&b,next+offset,sizeof(next)-offset,&used,9,8,&n)==0);
    assert(ec_annexb_finish(&b,&n)==1&&n.pts==9&&n.dts==8&&n.size==2);
    free(b.data);puts("Annex B: fragment boundaries, escape bytes, timestamps, EOF, seek reset and long NAL: OK");return 0;
}
