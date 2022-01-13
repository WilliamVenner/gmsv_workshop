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