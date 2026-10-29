-- Integration driver: real libmpv, real overlays, deterministic pointer events.
local mp=require 'mp'
local json=require 'mp.utils'
local cfg
local last
mp.observe_property('user-data/enigmax-popup-test','native',function(_,value)
    if value and value~='' then cfg=json.parse_json(value);last=nil end
end)
mp.observe_property('user-data/enigmax-popup-state','native',function(_,value)
    if not cfg or not value or value=='' or value==last then return end
    local state=json.parse_json(value);if not state then return end
    last=value
    local plan=cfg.plan or {}
    local wanted=cfg.sequence and cfg.sequence[cfg.step or 1] or plan[state.depth]
    if not wanted and not cfg.close_button then return end
    mp.add_timeout(cfg.delay or 0.08,function()
        if not cfg or last~=value then return end
        local w,h=mp.get_osd_size()
        if w~=state.w or h~=state.h then
            mp.commandv('script-message-to',state.client,'popup-input','move',table.concat({w,h,0,0,0,30,50,1,0,0},'|'))
            return
        end
        if cfg.capture then mp.commandv('screenshot-to-file',cfg.capture,'window') end
        local body
        if cfg.close_button then body=cfg.close_button
        else
            local index
            for i,id in ipairs(state.ids)do if id==wanted then index=i-1;break end end
            if not index then mp.commandv('script-message-to',state.client,'popup-input','close');cfg=nil;return end
            if index<state.offset or index>=state.offset+state.visible then
                mp.commandv('script-message-to',state.client,'popup-input',index<state.offset and 'wheelup'or'wheeldown')
                return
            end
            local x=state.x+state.width/2
            local y=state.y+state.header+(index-state.offset+0.5)*state.row
            local depth=math.max(2,math.min(8,state.w/480))
            local second=cfg.eye==1
            local sign=second and -1 or 1
            if state.mode=='sbs' then
                local half=math.floor(state.w/2)
                local span=second and state.w-half or half
                x=(second and half or 0)+(x+sign*depth)*span/state.w
            elseif state.mode=='ou' then
                local half=math.floor(state.h/2)
                local span=second and state.h-half or half
                x=x+sign*depth;y=(second and half or 0)+y*span/state.h
            elseif state.mode=='framepack' then
                local scale=math.min(state.w/1920,state.h/2205)
                local ew=math.floor(1920*scale);local eh=math.floor(1080*scale)
                x=(state.w-ew)/2+(x+sign*depth)*ew/state.w
                y=math.floor((state.h-2205*scale)/2)+(second and math.floor(1125*scale)or 0)+y*eh/state.h
            end
            body=table.concat({state.w,state.h,x,y,0,30,50,1,0,0},'|')
        end
        local done=cfg.close_button or (cfg.sequence and (cfg.step or 1)>=#cfg.sequence) or (not cfg.sequence and state.depth>=#plan)
        if cfg.sequence and not done then cfg.step=(cfg.step or 1)+1 end
        if done then cfg=nil end
        mp.commandv('script-message-to',state.client,'popup-input','down',body)
        mp.commandv('script-message-to',state.client,'popup-input','up',body)
    end)
end)
mp.commandv('script-message','popup-qa-ready')
