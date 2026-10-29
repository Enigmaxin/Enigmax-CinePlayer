#include "probe_nal.h"
#include <assert.h>
#include <stdio.h>
int main(void){
 const uint8_t subset[]={0,0,0,1,0x6f,118,0,1};
 const uint8_t mvc[]={0,0,1,0x74,0,0,1,0};
 const uint8_t svc[]={0,0,1,0x74,128,0,1,0};
 const uint8_t bad[]={0,0,1,0x74,0,0,0,0};
 const uint8_t avc[]={0,0,1,0x67,100,0,1,0};
 assert(ec_mvc_nals(subset,sizeof(subset)));
 assert(ec_mvc_nals(mvc,sizeof(mvc)));
 assert(!ec_mvc_nals(svc,sizeof(svc)));
 assert(!ec_mvc_nals(bad,sizeof(bad)));
 assert(!ec_mvc_nals(avc,sizeof(avc)));
 for(size_t n=0;n<7;n++)assert(!ec_mvc_nals(mvc,n));
 puts("MVC structural NAL probe: MVC, subset SPS, SVC, AVC and truncation OK");return 0;
}
