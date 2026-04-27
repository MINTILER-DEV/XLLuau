local _lhs0 = config.timeout
local timeout = (if _lhs0 ~= nil then _lhs0 else 30)
local role = (if isAdmin then "admin" else "user")
local _lhs1 = (if primary ~= nil then primary else secondary)
local retries = (if _lhs1 ~= nil then _lhs1 else 5)
