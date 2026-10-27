-- Regression scenarios run in the bundled libmpv Lua VM, with controlled input.
local real=require 'mp'
local options={client='main',source=''}
require('mp.options').read_options(options,'enigmax-test')
local function test()
    local enabled,bindings,messages,observers,events,sent={},{},{},{},{},{}
    local clock,idle=0,true
    local timer
    local fake={}
    function fake.get_osd_size() return 1280,720 end
    function fake.get_mouse_pos() return 1094,710 end
    function fake.get_time() return clock end
    function fake.get_property_number(key,default) return ({['time-pos']=1,duration=60,volume=50})[key] or default end
    function fake.get_property_bool(key,default) if key=='idle-active' then return idle end;return default end
    function fake.commandv(...) sent[#sent+1]={...} end
    function fake.set_mouse_area(...) end
    function fake.set_key_bindings(list,section) bindings[section]=list end
    function fake.enable_key_bindings(section) enabled[section]=true end
    function fake.disable_key_bindings(section) enabled[section]=false end
    function fake.register_script_message(name,fn) messages[name]=fn end
    function fake.observe_property(name,_,fn) observers[name]=fn end
    function fake.register_event(name,fn) events[name]=fn end
    function fake.add_periodic_timer(_,fn)
        timer={alive=true,tick=fn,kill=function(self)self.alive=false end,resume=function(self)self.alive=true end}
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
    press()
    assert(timer.alive,'The bar should become visible')
    messages.popup('yes')
    assert(not enabled.enigmax_bottom and not enabled.enigmax_show and not timer.alive,'Popup must suspend overlay input and painting')
    local count=#sent
    clock=30;bindings.enigmax_show[2][2]();observers.pause()
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
    idle=true;observers['idle-active']('idle-active',true)
    assert(not enabled.enigmax_top and not enabled.enigmax_bottom,'Unloaded video must release controls')
end
local ok,error=pcall(test)
real.commandv('script-message-to',options.client,'menu-controls-test',ok and 'ok' or tostring(error))
