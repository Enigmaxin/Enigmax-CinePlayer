-- Integration driver: real libmpv, real overlays, deterministic pointer events.
local mp=require 'mp'
local json=require 'mp.utils'
local cfg
local last
mp.observe_property('user-data/enigmax-popup-test','native',function(_,value)
    if value and value~='' then cfg=json.parse_json(value);last=nil end
end)
local function on_state(_,value)
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
        if type(wanted)=='string' and wanted:sub(1,1)=='@' and wanted~='@back' and cfg.expected_label then
            local found=false;for _,label in ipairs(state.labels)do if label==cfg.expected_label then found=true end end
            if not found then mp.set_property('user-data/enigmax-popup-test-error','Stale position label');cfg=nil;mp.commandv('script-message-to',state.client,'popup-input','close');return end
        end
        if wanted=='@back' then cfg.step=(cfg.step or 1)+1;mp.commandv('keypress','LEFT');return
        elseif wanted=='@back-key' then cfg.step=(cfg.step or 1)+1;mp.commandv('keypress','BS');return
        elseif wanted=='@escape' then
            cfg=nil;mp.commandv('keypress','ESC');return
        elseif wanted=='@outside' then body=table.concat({state.w,state.h,1,1},'|')
        elseif wanted=='@subs' then body=cfg.finish_body
        elseif cfg.close_button then body=cfg.close_button
        else
            local index,button_slot
            if wanted=='@back-button' then index=-1
            else for i,id in ipairs(state.ids)do if id==wanted or (type(wanted)=="string" and state.labels[i]==wanted) then index=i-1;break end end end
            if not index and state.controls then
                for i=1,#state.ids do local buttons=state.controls[i]
                    if type(buttons)=='table' then
                        for slot=1,3 do local id=buttons[slot];if id==wanted then index=i-1;button_slot=slot-1;break end end
                    end
                    if index then break end
                end
            end
            if not index then
                -- A resize can publish the old page after the previous click.
                -- Wait for the expected page instead of closing a valid transition.
                cfg.retries=(cfg.retries or 0)+1
                if cfg.retries<=20 then
                    mp.add_timeout(0.05,function()if cfg then last=nil;on_state(nil,mp.get_property_native('user-data/enigmax-popup-state'))end end)
                    return
                end
                mp.set_property('user-data/enigmax-popup-test-error',json.format_json({wanted=wanted,labels=state.labels,step=cfg.step}))
                mp.commandv('script-message-to',state.client,'popup-input','close');cfg=nil;return
            end
            cfg.retries=0
            if index>=0 and (index<state.offset or index>=state.offset+state.visible) then
                mp.commandv('script-message-to',state.client,'popup-input',index<state.offset and 'wheelup'or'wheeldown')
                return
            end
            local x=index<0 and state.x+20 or state.x+state.width/2
            if button_slot then
                local bw=math.max(24,math.min(44,state.row+6))
                x=state.x+state.width-12-(3-button_slot)*(bw+6)+bw/2
            end
            local y=index<0 and state.y+state.header/2 or state.y+state.header+(index-state.offset+0.5)*state.row
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
end
mp.observe_property('user-data/enigmax-popup-state','native',on_state)
mp.set_property('user-data/enigmax-popup-qa-ready','yes')
mp.commandv('script-message','popup-qa-ready')
