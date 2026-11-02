local mp=require 'mp'
local json=require 'mp.utils'
local armed=false
mp.observe_property('user-data/enigmax-info-audit','native',function(_,v) armed=v and v~='' end)
mp.observe_property('user-data/enigmax-popup-state','native',function(_,v)
 if not armed or not v or v=='' then return end
 local state=json.parse_json(v);if not state then return end
 armed=false
 mp.set_property('user-data/enigmax-info-labels',json.format_json(state.labels))
 mp.add_timeout(0.15,function()
  local path=mp.get_property_native('user-data/enigmax-info-audit')
  mp.commandv('screenshot-to-file',path,'window')
  mp.commandv('script-message-to',state.client,'popup-input','close')
 end)
end)
mp.commandv('script-message','info-audit-ready')
