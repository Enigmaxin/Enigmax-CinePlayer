-- Enigmax CinePlayer: input/timing for the bounded native stereo control panel.
-- Rendering and hit testing share one Rust layout; no subtitle track is changed.
local mp = require 'mp'
local options={client='main'}
require('mp.options').read_options(options,'enigmax')
local target=options.client
-- Low-frequency resume checkpoints, independent of whether the controls are visible.
mp.add_periodic_timer(5,function()
    if mp.get_property_bool('idle-active',true) then return end
    local pos=mp.get_property_number('time-pos')
    local duration=mp.get_property_number('duration')
    if pos and duration then mp.commandv('script-message-to',target,'checkpoint',tostring(pos),tostring(duration),mp.get_property('path','')) end
end)
local projection = 'mono'
local visible, dragging, pinned = false, false, false
local popup_open = false
local modal_target
local volume_until, volume_feedback = 0, 0
local last_volume=mp.get_property_number('volume')
local last_mute=mp.get_property_bool('mute',false)
local last_activity, last_paint, last_move = 0, 0, 0
local previous = ''
local timer
local single_click
local function cancel_click()
    if single_click then single_click:kill();single_click=nil end
end
local startup_choice=false
local function choosing() return startup_choice end
local function screen_click()
    cancel_click()
    if popup_open then return end
    if choosing() then
        local x,y=mp.get_mouse_pos()
        mp.commandv('script-message-to',target,'startup-click',tostring(math.floor(x or -1)),tostring(math.floor(y or -1)))
        return
    end
    if mp.get_property_bool('idle-active',true) then
        local x,y=mp.get_mouse_pos()
        mp.commandv('script-message-to',target,'idle-click',tostring(math.floor(x or -1)),tostring(math.floor(y or -1)))
        return
    end
    local delay=mp.get_property_number('options/input-doubleclick-time',300)/1000
    single_click=mp.add_timeout(math.max(0.15,math.min(1,delay+0.03)),function()
        single_click=nil
        if not popup_open and not mp.get_property_bool('idle-active',true) then
            mp.commandv('script-message-to',target,'toggle-pause')
        end
    end)
end
local function number(key)
    local n = mp.get_property_number(key, 0) or 0
    if n ~= n or n == math.huge or n == -math.huge then return 0 end
    return math.max(0, n)
end
local function payload()
    local w, h = mp.get_osd_size()
    local x, y = mp.get_mouse_pos()
    if not w or not h or w < 160 or h < 100 then return nil end
    x, y = math.max(-1, math.min(x or -1, w)), math.max(-1, math.min(y or -1, h))
    return table.concat({w,h,x,y,number('time-pos'),number('duration'),math.min(100,number('volume')),
        mp.get_property_bool('pause',false) and 1 or 0,mp.get_property_bool('mute',false) and 1 or 0,mp.get_time()<volume_until and volume_feedback or 0}, '|')
end
local function areas()
    local w, h = mp.get_osd_size()
    if not w or not h then return end
    mp.set_mouse_area(0,0,w,h,'enigmax_show')
    if popup_open then
        mp.disable_key_bindings('enigmax_show')
        if modal_target then
            mp.set_mouse_area(0,0,w,h,'enigmax_popup')
            mp.enable_key_bindings('enigmax_popup')
        end
        mp.disable_key_bindings('enigmax_bottom');mp.disable_key_bindings('enigmax_top');return
    end
    mp.enable_key_bindings('enigmax_show','allow-vo-dragging+allow-hide-cursor')
    -- Hidden does not mean unclickable: otherwise a fast first press can
    -- become a VO window drag before Lua has enabled the bottom section.
    if choosing() or mp.get_property_bool('idle-active',true) then
        mp.disable_key_bindings('enigmax_bottom');mp.disable_key_bindings('enigmax_top');return
    end
    local ph = math.min(h, math.max(88, math.min(160, math.floor(h*0.1+0.5))))
    if dragging then
        mp.set_mouse_area(0,0,w,h,'enigmax_bottom');mp.disable_key_bindings('enigmax_top')
    elseif projection == 'framepack' then
        local scale=math.min(w/1920,h/2205)
        local width=math.max(1,math.floor(1920*scale+1e-7))
        local x=math.max(0,math.floor((w-width)/2))
        local top=math.floor(math.max(0,(h-2205*scale)/2))
        local bh=math.min(math.max(1,math.floor(ph*width/w+0.5)),math.max(1,math.floor(1080*scale+1e-7)))
        local first=math.floor(top+1080*scale+1e-7)
        local second=math.floor(top+2205*scale+1e-7)
        mp.set_mouse_area(x,second-bh,x+width,second,'enigmax_bottom')
        mp.set_mouse_area(x,first-bh,x+width,first,'enigmax_top')
        mp.enable_key_bindings('enigmax_top')
    elseif projection == 'ou' then
        local bh = math.max(1,math.floor(ph/2))
        mp.set_mouse_area(0,h-bh,w,h,'enigmax_bottom')
        mp.set_mouse_area(0,math.floor(h/2)-bh,w,math.floor(h/2),'enigmax_top')
        mp.enable_key_bindings('enigmax_top')
    else
        mp.set_mouse_area(0,h-ph,w,h,'enigmax_bottom');mp.disable_key_bindings('enigmax_top')
    end
    mp.enable_key_bindings('enigmax_bottom')
end
local function paint(force)
    if not visible or popup_open or choosing() then return end
    local now=mp.get_time()
    if not force and now-last_paint<0.09 then return end
    local data=payload()
    if data and (force or data~=previous) then
        mp.commandv('script-message-to',target,'controls-draw',data)
        previous,last_paint=data,now
    end
end
local function hide()
    visible,dragging=false,false;previous=''
    if timer then timer:kill() end
    areas();mp.commandv('script-message-to',target,'controls-hide')
end
local function show()
    if popup_open then return end
    if choosing() or mp.get_property_bool('idle-active',true) then return end
    last_activity=mp.get_time()
    if not visible then visible=true;previous='';areas();timer:resume();paint(true) end
end
local function input(kind)
    if popup_open then return end
    show()
    if not visible then return end
    local data=payload()
    if not data then return end
    if kind=='down' then dragging=true;areas() end
    mp.commandv('script-message-to',target,'controls-input',kind,data)
    if kind=='up' or kind=='cancel' then dragging=false;areas() end
    last_activity=mp.get_time();previous=''
end
timer=mp.add_periodic_timer(0.1,function()
    if not pinned and not dragging and mp.get_time()-last_activity>2.5 then hide() else paint(false) end
end)
timer:kill()
mp.set_key_bindings({
    {'mbtn_left',screen_click},
    {'mbtn_left_dbl',function() cancel_click();mp.commandv('cycle','fullscreen') end},
    {'mouse_move',function()
        show()
        if dragging and mp.get_time()-last_move>0.05 then last_move=mp.get_time();input('move') end
    end},
    {'mouse_leave',function() if popup_open then return end;if dragging then input('cancel') end;if not pinned then hide() end end},
    {'wheel_up',function() input('wheelup') end},
    {'wheel_down',function() input('wheeldown') end},
},'enigmax_show','force')
mp.enable_key_bindings('enigmax_show','allow-vo-dragging+allow-hide-cursor')
for _,name in ipairs({'enigmax_bottom','enigmax_top'}) do
    mp.set_key_bindings({
        {'mbtn_left',function() input('up') end,function() input('down') end},
        {'mbtn_left_dbl','ignore'},
        {'wheel_up',function() input('wheelup') end},
        {'wheel_down',function() input('wheeldown') end},
    },name,'force')
end
mp.register_script_message('configure',function(mode)
    if mode~='mono' and mode~='anaglyph' and mode~='sbs' and mode~='ou' and mode~='framepack' then return end
    projection=mode;previous='';areas();paint(true)
end)
mp.register_script_message('show',function()
    show();paint(true)
end)
mp.register_script_message('pin',function(value)
    pinned=value=='yes';if pinned then show();paint(true) else hide() end
end)
mp.register_script_message('popup',function(value)
    popup_open=value=='yes'
    dragging=false;previous='';last_activity=mp.get_time()
    if popup_open then
        cancel_click();timer:kill();areas()
    else
        areas()
        if visible then timer:resume();paint(true) end
    end
end)
local function modal_input(kind)
    if not modal_target then return end
    mp.commandv('script-message-to',modal_target,'popup-input',kind,payload() or '')
end
local modal_move=0
mp.set_key_bindings({
    {'mbtn_left',function()modal_input('up')end,function()modal_input('down')end},
    {'mbtn_left_dbl','ignore'},
    {'mbtn_right',function()modal_input('close')end},
    {'mouse_move',function()local now=mp.get_time();if now-modal_move>0.03 then modal_move=now;modal_input('move')end end},
    {'wheel_up',function()modal_input('wheelup')end},
    {'wheel_down',function()modal_input('wheeldown')end},
    {'ESC',function()modal_input('close')end},
    {'Ctrl+3',function()modal_input('close')end},
    {'q',function()modal_input('close')end},
    {'UP',function()modal_input('previous')end},
    {'DOWN',function()modal_input('next')end},
    {'LEFT',function()modal_input('left')end},
    {'RIGHT',function()modal_input('right')end},
    {'ENTER',function()modal_input('enter')end},
},'enigmax_popup','force')
mp.disable_key_bindings('enigmax_popup')
mp.register_script_message('stereo-popup',function(client)
    modal_target=client~='' and client or nil
    popup_open=modal_target~=nil
    dragging=false;cancel_click();previous='';last_activity=mp.get_time()
    if modal_target then timer:kill();areas();modal_input('resize')
    else mp.disable_key_bindings('enigmax_popup');areas();if visible then timer:resume();paint(true)end end
end)
mp.observe_property('osd-dimensions','native',function() previous='';areas();if modal_target then modal_input('resize')end;paint(true);if mp.get_property_bool('idle-active',true) then mp.commandv('script-message-to',target,'idle-draw')end end)
mp.observe_property('volume','number',function(_,value)
    if type(value)~='number' then return end
    if last_volume~=nil and value~=last_volume then
        volume_feedback=value>last_volume and 1 or -1
        volume_until=mp.get_time()+1.8
        show();paint(false)
    end
    last_volume=value
end)
mp.observe_property('mute','bool',function(_,value)
    if value~=last_mute then
        volume_feedback=value and 2 or 3
        volume_until=mp.get_time()+1.8
        show();paint(false)
    end
    last_mute=value
end)
for _,key in ipairs({'pause','duration'}) do
    mp.observe_property(key,'native',function() paint(false) end)
end
mp.observe_property('idle-active','bool',function(_,idle) if idle then hide();mp.commandv('script-message-to',target,'idle-draw') else mp.commandv('overlay-remove',63);areas() end end)
mp.observe_property('osd-dimensions','native',function()if choosing()then mp.commandv('script-message-to',target,'startup-draw')end end)
mp.register_event('start-file',function() volume_until=0;volume_feedback=0;cancel_click();hide();mp.commandv('overlay-remove',63) end)
mp.register_event('file-loaded',function() areas();if pinned then show();paint(true) end end)
mp.set_key_bindings({
    {'SPACE',function()mp.commandv('script-message-to',target,'startup-choice','continue')end},
    {'ENTER',function()mp.commandv('script-message-to',target,'startup-choice','continue')end},
},'enigmax_startup','force')
mp.disable_key_bindings('enigmax_startup')
mp.observe_property('user-data/enigmax-startup','native',function(_,value)
    startup_choice=value=='yes'
    if startup_choice then hide();mp.enable_key_bindings('enigmax_startup');mp.commandv('script-message-to',target,'startup-draw')
    else mp.disable_key_bindings('enigmax_startup') end
    areas()
end)
mp.register_event('shutdown',function() hide() end)
areas()
mp.commandv('script-message-to',target,'controls-ready')
