-- Enigmax CinePlayer: input/timing for the bounded native stereo control panel.
-- Rendering and hit testing share one Rust layout; no subtitle track is changed.
local mp = require 'mp'
local options={client='main'}
require('mp.options').read_options(options,'enigmax')
local target=options.client
local projection = 'mono'
local visible, dragging, pinned = false, false, false
local popup_open = false
local last_activity, last_paint, last_move = 0, 0, 0
local previous = ''
local timer
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
        mp.get_property_bool('pause',false) and 1 or 0,mp.get_property_bool('mute',false) and 1 or 0}, '|')
end
local function areas()
    local w, h = mp.get_osd_size()
    if not w or not h then return end
    mp.set_mouse_area(0,0,w,h,'enigmax_show')
    if popup_open then
        mp.disable_key_bindings('enigmax_show')
        mp.disable_key_bindings('enigmax_bottom');mp.disable_key_bindings('enigmax_top');return
    end
    mp.enable_key_bindings('enigmax_show','allow-vo-dragging+allow-hide-cursor')
    -- Hidden does not mean unclickable: otherwise a fast first press can
    -- become a VO window drag before Lua has enabled the bottom section.
    if mp.get_property_bool('idle-active',true) then
        mp.disable_key_bindings('enigmax_bottom');mp.disable_key_bindings('enigmax_top');return
    end
    local ph = math.min(h, math.max(64, math.min(160, math.floor(h*0.1+0.5))))
    if dragging then
        mp.set_mouse_area(0,0,w,h,'enigmax_bottom');mp.disable_key_bindings('enigmax_top')
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
    if not visible or popup_open then return end
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
    if mp.get_property_bool('idle-active',true) then return end
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
    {'mouse_move',function()
        show()
        if dragging and mp.get_time()-last_move>0.05 then last_move=mp.get_time();input('move') end
    end},
    {'mouse_leave',function() if popup_open then return end;if dragging then input('cancel') end;if not pinned then hide() end end},
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
    if mode~='mono' and mode~='anaglyph' and mode~='sbs' and mode~='ou' then return end
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
        timer:kill();areas()
    else
        areas()
        if visible then timer:resume();paint(true) end
    end
end)
mp.observe_property('osd-dimensions','native',function() previous='';areas();paint(true) end)
for _,key in ipairs({'pause','volume','mute','duration'}) do
    mp.observe_property(key,'native',function() paint(false) end)
end
mp.observe_property('idle-active','bool',function(_,idle) if idle then hide() else areas() end end)
mp.register_event('start-file',function() hide() end)
mp.register_event('file-loaded',function() areas();if pinned then show();paint(true) end end)
mp.register_event('shutdown',function() hide() end)
areas()
mp.commandv('script-message-to',target,'controls-ready')
