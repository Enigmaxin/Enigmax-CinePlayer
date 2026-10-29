#ifndef EC_PROBE_NAL_H
#define EC_PROBE_NAL_H
#include <stdint.h>
#include <stddef.h>
static inline int ec_mvc_nals(const uint8_t *p,size_t n){
    for(size_t i=0;i+5<n;i++){
        size_t start=0;
        if(p[i]==0&&p[i+1]==0&&p[i+2]==1)start=i+3;
        else if(i+6<n&&p[i]==0&&p[i+1]==0&&p[i+2]==0&&p[i+3]==1)start=i+4;
        if(!start||start+3>=n)continue;
        unsigned type=p[start]&31;
        if(type==15&&(p[start+1]==118||p[start+1]==128))return 1;
        if(type==20&&!(p[start+1]&128)&&(p[start+3]&1))return 1;
    }
    return 0;
}
#endif
