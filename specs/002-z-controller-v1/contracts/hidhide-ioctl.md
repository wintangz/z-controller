# Driver IOCTL Contract: `hidhide-cloak` & `virtual-pad-output`

## 1. HidHide User-Mode ABI (`\\.\HidHide`)
Source: `nefarius/HidHide/Shared/HidHideIoctlContract.h` & `HidHide/src/Logic.c`

- Device symbolic link: `\\.\HidHide` (opened with `GENERIC_READ | GENERIC_WRITE`, `FILE_SHARE_READ | FILE_SHARE_WRITE`, `OPEN_EXISTING`).
- Exclusive driver rule (`WdfDeviceInitSetExclusive(TRUE)`): opened atomically per transaction and closed immediately (`< 1 ms`). Never held open across idle. `ERROR_ACCESS_DENIED (5)` means another client (e.g., `HidHideClient.exe`) currently holds the device.
- Control codes (`IoControlDeviceType = 32769`, `METHOD_BUFFERED = 0`, `FILE_READ_DATA = 1`):
  - `IOCTL_GET_WHITELIST` = `0x8001_6000` (2048) -> output `MULTI_SZ` UTF-16LE NT device paths (`\Device\HarddiskVolumeX\...\exe`)
  - `IOCTL_SET_WHITELIST` = `0x8001_6004` (2049) -> input `MULTI_SZ` (or null/0 to clear), output null/0
  - `IOCTL_GET_BLACKLIST` = `0x8001_6008` (2050) -> output `MULTI_SZ` UTF-16LE PnP DeviceInstanceIds
  - `IOCTL_SET_BLACKLIST` = `0x8001_600C` (2051) -> input `MULTI_SZ` (or null/0 to clear), output null/0
  - `IOCTL_GET_ACTIVE`    = `0x8001_6010` (2052) -> output `u8` (`BOOLEAN`)
  - `IOCTL_SET_ACTIVE`    = `0x8001_6014` (2053) -> input `u8` (`0` or `1`, length `1`), output null/0
  - `IOCTL_GET_WLINVERSE` = `0x8001_6018` (2054) -> output `u8`
  - `IOCTL_SET_WLINVERSE` = `0x8001_601C` (2055) -> input `u8` (`0` or `1`, length `1`), output null/0

## 2. ViGEmBus Kernel IOCTL Contract
- Interface GUID: `{96E42B22-F5E9-42F8-B043-ED0F932F014F}` (`FILE_FLAG_OVERLAPPED | FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH`).
- Control codes (`FILE_DEVICE_BUS_EXTENDER = 0x2A`, `METHOD_BUFFERED = 0`):
  - `PLUGIN_TARGET`: `ctl_code(0x2A, 0x801, 0, 2)` (`TargetType`: `0 = Xbox360`, `2 = DualShock4`)
  - `UNPLUG_TARGET`: `ctl_code(0x2A, 0x802, 0, 2)`
  - `CHECK_VERSION`: `ctl_code(0x2A, 0x803, 0, 2)` (`version = 0x0001`)
  - `WAIT_DEVICE_READY`: `ctl_code(0x2A, 0x804, 0, 2)`
  - `XUSB_REQUEST_NOTIFICATION`: `ctl_code(0x2A, 0xA01, 0, 3)`
  - `XUSB_SUBMIT_REPORT`: `ctl_code(0x2A, 0xA02, 0, 2)`
  - `DS4_SUBMIT_REPORT`: `ctl_code(0x2A, 0xA03, 0, 2)`
  - `DS4_REQUEST_NOTIFICATION`: `ctl_code(0x2A, 0xA04, 0, 2)`
  - `XUSB_GET_USER_INDEX`: `ctl_code(0x2A, 0xA07, 0, 3)`
