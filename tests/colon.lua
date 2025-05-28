local function skibidi(self, x)
    self.value = x
end

local v = ({ ["value"] = 69 })
v:skibidi(42)
console.log("skibidi", v.value)
