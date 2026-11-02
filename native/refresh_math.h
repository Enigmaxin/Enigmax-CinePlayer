/* Nominal rate matching: never conflate 24 with 24000/1001 Hz. */
#ifndef CINE_REFRESH_MATH_H
#define CINE_REFRESH_MATH_H
#include <math.h>
static double cine_nominal_fps(double fps) {
    const double rates[]={24000.0/1001.0,30000.0/1001.0,60000.0/1001.0,120000.0/1001.0};
    for(unsigned i=0;i<sizeof(rates)/sizeof(rates[0]);i++)
        if(fabs(fps-rates[i])<0.0001)return rates[i];
    return fps;
}
static int cine_rate_matches(double hz,double wanted) {
    return isfinite(hz)&&isfinite(wanted)&&hz>0&&wanted>0&&fabs(hz-wanted)<0.001;
}
static int cine_rate_candidate(unsigned hz,double wanted) {
    return hz==(unsigned)floor(wanted)||hz==(unsigned)floor(wanted+0.5);
}
#endif
