-- Real popup client; exercises pointer mapping from either eye.
local mp=require 'mp';local json=require 'mp.utils';local cfg;local last;local step=1
mp.observe_property('user-data/enigmax-slider-test','native',function(_,v) cfg=v and json.parse_json(v) or nil;step=1;last=nil end)
mp.observe_property('user-data/enigmax-slider-state','native',function(_,text)
 if not cfg or not text or text==last then return end
 local s=json.parse_json(text);if not s then return end;last=text
 mp.add_timeout(0.08,function()
  if not cfg or text~=last then return end
  local x,y
  if step==1 then x=s.barx+s.barw*0.2;y=s.bary+10
  elseif step==2 then x=s.barx+s.barw*0.8;y=s.bary+10
  elseif step==3 then x=s.x+35;y=s.y+s.height-25
  else mp.commandv('script-message-to',s.client,'popup-input','close');cfg=nil;return end
  local d=math.max(2,math.min(8,s.w/480));local second=cfg.eye==1;local sign=second and -1 or 1
  if s.mode=='sbs' then local half=math.floor(s.w/2);local span=second and s.w-half or half;x=(second and half or 0)+(x+sign*d)*span/s.w
  elseif s.mode=='ou' then local half=math.floor(s.h/2);local span=second and s.h-half or half;x=x+sign*d;y=(second and half or 0)+y*span/s.h
  elseif s.mode=='framepack' then local sc=math.min(s.w/1920,s.h/2205);local ew=math.floor(1920*sc);local eh=math.floor(1080*sc);x=(s.w-ew)/2+(x+sign*d)*ew/s.w;y=math.floor((s.h-2205*sc)/2)+(second and math.floor(1125*sc)or 0)+y*eh/s.h end
  step=step+1
  local body=table.concat({s.w,s.h,x,y},'|');mp.commandv('script-message-to',s.client,'popup-input','down',body);mp.commandv('script-message-to',s.client,'popup-input','up',body)
 end)
end)
mp.commandv('script-message-to',mp.get_opt('enigmax-client') or 'enigmax','slider-qa-ready')
