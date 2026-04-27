local numbers = {1, 2, 3, 4}
local matrix = {{1, 2}, {3, 4}}
local result = {kind = "ok", value = 2}
local _comp0 = {}
for _, x in numbers do
    if ((x % 2) == 0) then
        table.insert(_comp0, (x * 2))
    end
end
local doubled = _comp0
local _comp1 = {}
for _, x in numbers do
    _comp1[x] = (x ^ 2)
end
local squared = _comp1
local _comp2 = {}
for _, row in matrix do
    for _, value in row do
        table.insert(_comp2, value)
    end
end
local flat = _comp2
local _matched3 = false
do
    local _mbind4 = {}
    _mbind4["value"] = result.value
    local _mcond5 = (not _matched3) and (type(result) == "table" and result.kind == "ok" and true)
    if _mcond5 then
        local value = _mbind4["value"]
        _matched3 = true
        print(value, doubled[1], squared[2], flat[4])
    end
end
do
    local _mbind6 = {}
    _mbind6["message"] = result.error
    local _mcond7 = (not _matched3) and (type(result) == "table" and result.kind == "err" and true)
    if _mcond7 then
        local message = _mbind6["message"]
        _matched3 = true
        print(message)
    end
end
