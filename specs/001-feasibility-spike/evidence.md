# Evidence Report: `feasibility-spike`

**Date**: 2026-11-25  
**Target Toolchain**: `rustc 1.100.0` (`stable-x86_64-pc-windows-gnu`) using `windows-sys = "=0.59.0"`  
**Test Binaries**:
- Whitelisted feeder: `target/debug/spike_runner.exe` (NT path `\Device\HarddiskVolume1\Repos\z-controller\target\debug\spike_runner.exe`)
- Un-whitelisted game/OS probe: `target/debug/probe_game_view.exe`

## Toolchain Decision Log
- `vigem-rust 0.2.1` failed to compile on `x86_64-pc-windows-gnu` because its `windows 0.62` dependency triggers `raw-dylib` -> `dlltool.exe` -> `as.exe` (which is absent from rustup's `self-contained` GNU binaries), and the user cancelled installing Visual Studio Build Tools (`"stop installing, I will do it manually"`).
- `crates/z-platform-windows/src/vigem.rs` implements the `ViGEmBus` kernel IOCTL ABI directly using `windows-sys = "=0.59.0"` (`windows_x86_64_gnu` import libraries), compiling and linking cleanly in `1.44s–2.24s` with zero external SDK installs.

## Live Hardware Spike Results

| Spike Item | Result | Proving Evidence |
|---|---|---|
| 1. SDL3 reads Aolion Pro Controller + battery | **PASS** | `SDL3 3.4.20` (`SDL_JOYSTICK_HIDAPI_SWITCH=1` + `assets/gamecontrollerdb.txt`) opened `id=1 name='Nintendo Switch Pro Controller' VID=057E PID=2009 type=NintendoSwitchPro conn=Wireless`, `sdl_power=(OnBattery, Some(100)%)`, PnP `HID\{00001124-0000-1000-8000-00805F9B34FB}_VID&0002057E_PID&2009\9&1A2DBC92&1&0000`. |
| 2. ViGEm creates virtual Xbox 360 pad visible in `joy.cpl` and readable by games | **PASS** | Plugged ViGEm X360 `serial=1` (`VID=045E PID=028E`, `vigem=true`). Un-whitelisted `probe_game_view.exe` detected `[WinMM] slot=1 VID=045E PID=028E name='Microsoft PC-joystick driver'`, `[DirectInput8] DI8 name='Controller (XBOX 360 For Windows)' usage=01:05`, and read exact injected state via `[XInput] slot=0 pkt=2 wButtons=0x1200 LT=128 RT=255 sThumbLX=-12000 sThumbLY=24000 sThumbRX=8000 sThumbRY=-16000`. |
| 3. HidHide cloaks physical pad from `joy.cpl` / games while app still reads it | **PASS** | `\\.\HidHide` (`0x80016004`, `0x8001600C`, `0x80016014`) whitelisted `spike_runner.exe` and cloaked `HID\{00001124-0000-1000-8000-00805F9B34FB}_VID&0002057E_PID&2009\9&1A2DBC92&1&0000`. Un-whitelisted `probe_game_view.exe` dropped `[WinMM] active_slots` from `2` to `1` (virtual X360 only), `[DirectInput8] count` from `2` to `1` (`'Wireless Gamepad'` vanished), and `[HID] VID=057E PID=2009` returned `status=AccessDenied`. Simultaneously, whitelisted `spike_runner.exe` retained `pwr=(OnBattery, Some(100)%)` and `raw_hid_open_ok=true`. Cleanup restored `[DirectInput8] count=1 ('Wireless Gamepad')`, `[XInput] connected_slots=0`, and `[HID] 057E:2009 status=Ok`. |
