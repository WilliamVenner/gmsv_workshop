<h1 align="center">gmsv_workshop</h1>

This module allows for servers to use the [`steamworks.DownloadUGC`](https://wiki.facepunch.com/gmod/steamworks.DownloadUGC) and [`steamworks.FileInfo`](https://wiki.facepunch.com/gmod/steamworks.FileInfo) functions, enabling runtime downloading & mounting of Workshop addons and GMA files through [`game.MountGMA`](https://wiki.facepunch.com/gmod/game.MountGMA)

No additional configuration is needed, just install the module to the server and scripts can make use of it.

## Installation

First, run this command in your server console to determine the correct module to download:

```lua
lua_run print("gmsv_workshop_" .. ((system.IsLinux() and "linux" .. (jit.arch == "x86" and "" or "64")) or (system.IsWindows() and "win" .. (jit.arch == "x86" and "32" or "64")) or "UNSUPPORTED") .. ".dll")
```

Next, download the module from the [releases page](https://github.com/WilliamVenner/gmsv_workshop/releases)

Finally, drop the DLL file in `garrysmod/lua/bin` in your server files. **If the `lua/bin` folder doesn't exist, create it.**

# "Couldn't load module library!" error

Either:

1. **(Most likely)** Your server is running the x86-64 branch in 32-bit. If you start your x86-64 branch server using the `srcds_run` binary, this is the problem. Start it using `srcds_run_x64` to launch it in 64-bit.
2. Garry's Mod updated the Steamworks version and this now needs to be recompiled against it, [open an issue](https://github.com/WilliamVenner/gmsv_workshop/issues) if this is the case.