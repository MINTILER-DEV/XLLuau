local DEFAULT_TIMEOUT = 30
local _v0 = config
local _d1 = _v0
local timeout = if _d1.timeout ~= nil then _d1.timeout else DEFAULT_TIMEOUT
local filters = _d1.filters
if stats.total == nil then
    stats.total = 0
end
stats.total = stats.total + 1
local _opt2 = nil
do
    local _cur3 = data
    if _cur3 ~= nil then
        _cur3 = _cur3.items
        if _cur3 ~= nil then
            _cur3 = _cur3[1]
            _opt2 = _cur3
        end
    end
end
local _lhs4 = _opt2
local selected = (if _lhs4 ~= nil then _lhs4 else fallback)
local _pipe5 = map(filters, normalize)
local normalized = filter(_pipe5, isValid)
local _cond6 = ((selected :: any))
local label = (if _cond6 then "loaded" else "empty")
function summarize(_param0: PlayerRecord)
    local _d7 = _param0
    local name = _d7.name
    local score = _d7.score
    local safeScore = (if score ~= nil then score else 0)
    return name, safeScore
end
