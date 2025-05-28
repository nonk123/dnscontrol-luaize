function Add2(a, b)
    return a + b
end

function Add()
    local a = 5
    local b = 7
    return Add2(a, b)
end

console.log("Add", Add())
