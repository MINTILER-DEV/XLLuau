type Comparable = number
type User = { id: number, name: string }
type Result<T, E> = { ok: true, value: T }
    | { ok: false, error: E }
type Config = {
    read host: string,
    port: number,
    timeout: number?,
}
type PartialConfig = {
    read host: string?,
    port: number?,
    timeout: number?,
}
type HostConfig = {
    read host: string,
    port: number,
}
type Flags = { debug: boolean, verbose: boolean }
type Present = "ok" | "err"
local function max<T>(a: (T & Comparable), b: (T & Comparable)): T
    local _ifexpr0
    local _ifcond1 = (a > b)
    if _ifcond1 then
        _ifexpr0 = a
    else
        _ifexpr0 = b
    end
    return _ifexpr0
end
local function makeEmpty<T>(): T
    return (nil :: any)
end
local function fetch<T, Err>(url: string): Result<T, Err>
    return (nil :: any)
end
local scoreA = 8
local scoreB = 12
local best = max(scoreA, scoreB)
local empty = ((makeEmpty :: () -> { x: number, y: number }))()
local user = ((fetch :: (string) -> Result<User, string>))("/api/user")
local DEFAULTS = table.freeze({host = "localhost", port = 80, timeout = 30})
type Defaults = {
    read host: string,
    read port: number,
    read timeout: number,
}
return best, empty, user, DEFAULTS
