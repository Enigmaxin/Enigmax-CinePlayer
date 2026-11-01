-- Regression scenarios run in the bundled libmpv Lua VM, with controlled input.
local real=require 'mp'
local options={client='main',source=''}
require('mp.options').read_options(options,'enigmax-test')
local function test()
    local enabled,bindings,messages,observers,events,sent={},{},{},{},{},{}
    local clock,idle=0,true
    local volume=50
    local startup="no"
    local mouse_areas={}
    local periodic={}
    local single_timer
    local timer
    local fake={}
    local cursor,properties={},{}
    local sw,sh,mx,my=1280,720,1094,710
    function fake.create_osd_overlay() cursor={remove=function(self)self.removed=true end,update=function(self)self.removed=false end};return cursor end
    function fake.set_property(k,v) properties[k]=v end
    function fake.set_property_number(k,v) properties[k]=v end
    function fake.get_osd_size() return sw,sh end
    function fake.get_mouse_pos() return mx,my end
    function fake.get_property(key,default) if key=="user-data/enigmax-startup" then return startup end;return default end
    function fake.add_timeout(_,fn) single_timer={alive=true,tick=fn,kill=function(self)self.alive=false end};return single_timer end
    function fake.get_time() return clock end
    function fake.get_property_number(key,default) return ({['time-pos']=1,duration=60,volume=volume})[key] or default end
    function fake.get_property_bool(key,default) if key=='idle-active' then return idle end;return default end
    function fake.commandv(...) sent[#sent+1]={...} end
    function fake.set_mouse_area(x1,y1,x2,y2,name) mouse_areas[name]={x1,y1,x2,y2} end
    function fake.set_key_bindings(list,section) bindings[section]=list end
    function fake.enable_key_bindings(section) enabled[section]=true end
    function fake.disable_key_bindings(section) enabled[section]=false end
    function fake.register_script_message(name,fn) messages[name]=fn end
    function fake.observe_property(name,_,fn) observers[name]=fn end
    function fake.register_event(name,fn) events[name]=fn end
    function fake.add_periodic_timer(interval,fn)
        timer={alive=true,tick=fn,kill=function(self)self.alive=false end,resume=function(self)self.alive=true end}
        periodic[interval]=timer
        return timer
    end
    package.loaded.mp=fake
    package.loaded['mp.options']={read_options=function()end}
    dofile(options.source)
    assert(not enabled.enigmax_bottom,'Idle must not capture bottom clicks')
    idle=false;observers['idle-active']('idle-active',false);events['file-loaded']()
    assert(enabled.enigmax_bottom,'First hidden-bar press was disabled')
    assert(not timer.alive,'Hidden controls must not poll')
    local function press()
        bindings.enigmax_bottom[1][3]() -- real mp.set_key_bindings: down callback
        bindings.enigmax_bottom[1][2]() -- mouse press callback runs on release
        local inputs={}
        for _,m in ipairs(sent) do if m[3]=='controls-input' then inputs[#inputs+1]=m[4] end end
        assert(inputs[#inputs-1]=='down' and inputs[#inputs]=='up','A complete first click must be delivered')
    end
    local count_before=#sent
    bindings.enigmax_show[1][2]()
    assert(single_timer.alive,'Single click must be pending')
    bindings.enigmax_show[2][2]()
    assert(not single_timer.alive,'Double click must cancel pause')
    assert(sent[#sent][1]=='cycle' and sent[#sent][2]=='fullscreen','Double click must retain fullscreen')
    bindings.enigmax_show[1][2]();single_timer.tick()
    assert(sent[#sent][3]=='toggle-pause','Single click must toggle pause')
    press()
    assert(timer.alive,'The bar should become visible')
    messages.popup('yes')
    assert(not enabled.enigmax_bottom and not enabled.enigmax_show and not timer.alive,'Popup must suspend overlay input and painting')
    local count=#sent
    clock=30;bindings.enigmax_show[4][2]();observers.pause()
    assert(#sent==count,'Mouse leave/property changes must not hide or paint underneath a native popup')
    messages.popup('no')
    assert(enabled.enigmax_bottom and enabled.enigmax_show and timer.alive,'Popup close must restore controls')
    count=#sent;clock=31;timer.tick()
    for i=count+1,#sent do assert(sent[i][3]~='controls-hide','Popup close must reset inactivity') end
    clock=34;timer.tick()
    assert(not timer.alive and enabled.enigmax_bottom,'Auto-hide must retain first-click activation')
    press()
    messages.configure('ou')
    assert(enabled.enigmax_top and enabled.enigmax_bottom,'OU must retain both eye hit regions')
    messages.configure('framepack')
    assert(enabled.enigmax_top and enabled.enigmax_bottom,'Framepack needs both eye panels')
    local top,bottom=mouse_areas.enigmax_top,mouse_areas.enigmax_bottom
    assert(top[1]==327 and top[2]==309 and top[3]==953 and top[4]==352,'Framepack top coordinates')
    assert(bottom[1]==327 and bottom[2]==677 and bottom[3]==953 and bottom[4]==720,'Framepack bottom coordinates')
    volume=52;observers.volume('volume',volume);clock=clock+0.11;timer.tick()
    local function last_draw()
        for i=#sent,1,-1 do if sent[i][3]=='controls-draw' then return sent[i][4] end end
        return ''
    end
    assert(last_draw():match('|1$'),'Volume up must carry transient direction')
    clock=clock+2;timer.tick()
    assert(last_draw():match('|0$'),'Volume feedback must expire without another key press')
    volume=49;observers.volume('volume',volume);clock=clock+0.11;timer.tick()
    assert(last_draw():match('|%-1$'),'Volume down must carry transient direction')
    messages['stereo-popup']('qa-popup-client')
    assert(enabled.enigmax_popup and not enabled.enigmax_bottom and not enabled.enigmax_show and not timer.alive,'Stereo popup must own input and suspend bar painting')
    bindings.enigmax_popup[1][3]();bindings.enigmax_popup[1][2]()
    assert(sent[#sent][2]=='qa-popup-client' and sent[#sent][3]=='popup-input' and sent[#sent][4]=='up','Stereo popup must receive pointer events')
    messages['stereo-popup']('')
    assert(not enabled.enigmax_popup and enabled.enigmax_show and enabled.enigmax_bottom,'Stereo popup close must restore the bar')
    startup='yes';observers['user-data/enigmax-startup']('user-data/enigmax-startup',startup)
    assert(enabled.enigmax_startup and not enabled.enigmax_bottom and not timer.alive,'Startup must wait without bar polling')
    bindings.enigmax_show[1][2]()
    assert(sent[#sent][3]=='startup-click','Startup clicks must not toggle pause')
    bindings.enigmax_startup[1][2]()
    assert(sent[#sent][3]=='startup-choice' and sent[#sent][4]=='continue','Space must explicitly choose continue')
    startup='no';observers['user-data/enigmax-startup']('user-data/enigmax-startup',startup)
    assert(not enabled.enigmax_startup and enabled.enigmax_bottom,'Finishing startup must restore normal controls')
    for _,case in ipairs({{'sbs',3840,1080},{'sbs',1920,1080},{'ou',1920,2160},{'ou',1920,1080},{'framepack',1920,2205},{'anaglyph',1920,1080}}) do
        sw,sh=case[2],case[3];mx,my=sw*0.2,sh*0.2;clock=clock+1
        messages.configure(case[1]);bindings.enigmax_show[3][2]()
        local payload=properties['user-data/enigmax-cursor']
        if case[1]=='anaglyph' then
            assert(payload=='' and properties['cursor-autohide']~='always','Anaglyph must retain the native pointer')
        else
            assert(properties['cursor-autohide']=='always','Stereo must replace native cursor')
            local v={};for n in payload:gmatch('[^|]+') do v[#v+1]=tonumber(n) end
            assert(#v==8,'Exactly two eye cursors must be supplied')
            if case[1]=='sbs' then assert(v[5]-v[1]==sw/2 and v[2]==v[6],'SBS cursor disparity must be zero') end
            assert(v[3]>=6 and v[4]>=6,'Cursor must remain visible')
            clock=clock+0.02;bindings.enigmax_bottom[3][2]()
            assert(properties['user-data/enigmax-cursor']~='','Cursor must update over controls')
        end
        messages['stereo-cursor']('no');assert(properties['user-data/enigmax-cursor']=='','Disabling must remove stereo cursors')
        messages['stereo-cursor']('yes')
    end
    messages.configure('mono');assert(properties['user-data/enigmax-cursor']=='' and properties['cursor-autohide']~='always','2D must restore native cursor')
    -- Property-style input generation: dimensions, eye edges and invalid coordinates.
    for i=1,180 do
        sw=320+(i*127)%3700;sh=200+(i*83)%2200
        for _,mode in ipairs({'sbs','ou','framepack'}) do
            messages.configure(mode)
            mx,my=sw*0.17,sh*0.21;clock=clock+0.02;bindings.enigmax_show[3][2]()
            local payload=properties['user-data/enigmax-cursor']
            if payload~='' then
                local v={};for n in payload:gmatch('[^|]+') do v[#v+1]=tonumber(n) end
                assert(#v==8,'Malformed cursor payload')
                for j=1,8,4 do assert(v[j]>=0 and v[j+1]>=0 and v[j]+v[j+2]<=sw+1 and v[j+1]+v[j+3]<=sh+1,'Cursor escaped output') end
            end
            mx,my=-1,-1;clock=clock+0.02;bindings.enigmax_show[3][2]()
            assert(properties['user-data/enigmax-cursor']=='','Outside pointer must disappear')
        end
    end
    messages.configure('mono')
    local before=#sent
    for _=1,2000 do periodic[2].tick();periodic[5].tick() end
    assert(#sent==before,'Background health/checkpoints must not flood the main client queue during a modal dialog')
    assert(properties['user-data/enigmax-health']==clock and properties['user-data/enigmax-checkpoint'],'Latest background state must remain available')
    idle=true;observers['idle-active']('idle-active',true)
    assert(not enabled.enigmax_top and not enabled.enigmax_bottom,'Unloaded video must release controls')
end
local ok,error=pcall(test)
real.commandv('script-message-to',options.client,'menu-controls-test',ok and 'ok' or tostring(error))
