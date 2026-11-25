# Data Model: `z-controller` V1

## Core Types (`crates/z-core`)

### `HardwareKey` (`core-input-battery`, `profile-mapping`)
- `vid: u16`
- `pid: u16`
- `discriminator: String` — serial number if non-empty, else normalized PnP instance prefix.
- `Display` / canonical string key used in `config.toml` `[bindings]`.

### `ControllerFamily` & `Transport` (`core-input-battery`)
- `ControllerFamily`: `Nintendo | Sony | Xbox | Generic`
- `Transport`: `Bluetooth | Usb | Unknown`

### `BatteryReading` (`core-input-battery`)
- `percent: Option<u8>` (`0..=100`)
- `status: BatteryStatus` (`OnBattery | Charging | Charged | Wired | Unknown`)
- `source: &'static str`

### `Profile` (`profile-mapping`)
- `id: String` (slug/UUID-like stable ID)
- `name: String`
- `target: VirtualTargetKind` (`Xbox360 | DualShock4`)
- `cloak_physical: bool` (default `true`)
- `disconnect_policy: DisconnectPolicy` (`KeepNeutral | DropVirtual`, default `KeepNeutral`)
- `swap_ab_xy: bool` (default `false`; swaps South<->East and West<->North)
- `left_deadzone: f32` (`0.0..=0.5`, default `0.10`)
- `right_deadzone: f32` (`0.0..=0.5`, default `0.10`)
- `rumble_enabled: bool` (default `true`)
- `button_remap: BTreeMap<LogicalButton, Option<LogicalButton>>` (Level-2 1:1 remap or unbind; no macros/turbo)

### `LogicalButton` (15 Core Mappable Controls)
`South`, `East`, `West`, `North`, `LeftShoulder`, `RightShoulder`, `Back`, `Start`, `Guide`, `LeftStick`, `RightStick`, `DpadUp`, `DpadDown`, `DpadLeft`, `DpadRight`.

### `VirtualOutputReport` (`virtual-pad-output`)
- `Xbox360(X360Report)`: `buttons: u16`, `left_trigger: u8`, `right_trigger: u8`, `thumb_lx/ly/rx/ry: i16` (`+32767` = Up).
- `DualShock4(Ds4Report)`: `thumb_lx/ly/rx/ry: u8` (`128` = center, `0` = Up, `255` = Down), `buttons: u16` (low nibble `0x08` = Neutral D-pad), `special: u8`, `trigger_left/right: u8`.
