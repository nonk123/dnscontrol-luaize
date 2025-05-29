# dnscontrol-luaize

A [dnscontrol](https://dnscontrol.org/) wrapper that generates the `dnsconfig.js` file given a Lua source.

The Lua part mirrors the [original JS DSL](https://docs.dnscontrol.org/language-reference/js) APIs, only offering a slightly more bearable Lua syntax.

## Usage

Just run `dnscontrol` commands as usual, replacing the binary name with `dnscontrol-luaize`, using `dnsconfig.lua` as your entrypoint. All this does is re-export your `dnsconfig.lua` to `dnsconfig.js` and pass all arguments back to `dnscontrol`. For example:

```sh
dnscontrol-luaize preview
dnscontrol-luaize push
```
