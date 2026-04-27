local DEFAULT_TIMEOUT = 30
local config = {filters = {1, 2, 3, 4, 5}}
local stats = {total = 0}
local data = {items = {}}
local fallback = "fallback-item"
local function normalize(value: number): number
    return (value * 10)
end
local function isValid(value: number): boolean
    return (value >= 20)
end
local function map(list, fn)
    local result = {}
    for _, value in list do
        table.insert(result, fn(value))
    end
    return result
end
local function filter(list, fn)
    local result = {}
    for _, value in list do
        local _cond0 = fn(value)
        if _cond0 then
            table.insert(result, value)
        end
    end
    return result
end
local _v1 = config
local _d2 = _v1
local timeout = if _d2.timeout ~= nil then _d2.timeout else DEFAULT_TIMEOUT
local filters = _d2.filters
if stats.total == nil then
    stats.total = 0
end
stats.total = stats.total + 1
local _opt3 = nil
do
    local _cur4 = data
    if _cur4 ~= nil then
        _cur4 = _cur4.items
        if _cur4 ~= nil then
            _cur4 = _cur4[1]
            _opt3 = _cur4
        end
    end
end
local _lhs5 = _opt3
local selected = (if _lhs5 ~= nil then _lhs5 else fallback)
local _pipe6 = map(filters, normalize)
local normalized = filter(_pipe6, isValid)
local _cond7 = ((selected :: any))
local label = (if _cond7 then "loaded" else "empty")
function summarize(_param0: any)
    local _d8 = _param0
    local name = _d8.name
    local score = _d8.score
    local safeScore = (if score ~= nil then score else 0)
    return name, safeScore
end
print(timeout, stats.total, selected, label, (#normalized))
print(summarize({name = "Eve", score = nil}))
