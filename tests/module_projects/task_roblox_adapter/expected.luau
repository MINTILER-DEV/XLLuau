local function animate(model: Model): thread
    return coroutine.create(function()
        coroutine.yield(waitForFrame())
        return model
    end)
end
task.spawn(function()
    local _co0 = animate(workspace.Model)
    local _ok1, _result2 = coroutine.resume(_co0)
    while _ok1 and coroutine.status(_co0) ~= "dead" do
        _ok1, _result2 = coroutine.resume(_co0, _result2)
    end
    if _ok1 then
        local model = _result2
        print(model)
    else
        local err = _result2
        warn(err)
    end
end)
