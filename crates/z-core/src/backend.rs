use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::Receiver;
use thiserror::Error;

use crate::model::{
    DisconnectPolicy, HardwareKey, RumbleCommand, VirtualOutputReport, VirtualTargetKind,
};

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("output backend is read-only on this platform")]
    ReadOnly,
    #[error("virtual gamepad driver not installed or unavailable: {0}")]
    DriverUnavailable(String),
    #[error("virtual pad target not ready yet")]
    TargetNotReady,
    #[error("backend operation failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BackendCapabilities {
    pub can_emulate_x360: bool,
    pub can_emulate_ds4: bool,
    pub can_cloak_physical: bool,
    pub can_forward_rumble: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DriverStatus {
    pub vigem_installed: bool,
    pub hidhide_installed: bool,
    pub hidhide_accessible: bool,
    pub hidhide_error: Option<String>,
    pub reboot_recommended: bool,
}

pub trait VirtualPadHandle: Send {
    fn kind(&self) -> VirtualTargetKind;
    fn serial_no(&self) -> u32;
    fn xinput_user_index(&self) -> Option<u32> {
        None
    }
    fn send_report(&self, report: &VirtualOutputReport) -> Result<(), BackendError>;
    fn rumble_receiver(&self) -> Option<&Receiver<RumbleCommand>>;
}

pub trait OutputBackend: Send {
    fn capabilities(&self) -> BackendCapabilities;
    fn driver_status(&self) -> DriverStatus;
    fn spawn_virtual_pad(
        &mut self,
        kind: VirtualTargetKind,
        on_rumble_wake: Box<dyn Fn() + Send + Sync>,
    ) -> Result<Box<dyn VirtualPadHandle>, BackendError>;
    fn sync_cloaked_instances(&mut self, instances: &BTreeSet<String>) -> Result<(), BackendError>;
    fn uncloak_all(&mut self) -> Result<(), BackendError>;
}

/// Non-emulating stub proving shared `z-core` architecture compiles cleanly for
/// future read-only platforms (e.g. macOS battery + input viewer).
pub struct ReadOnlyNullBackend;

impl OutputBackend for ReadOnlyNullBackend {
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::default()
    }

    fn driver_status(&self) -> DriverStatus {
        DriverStatus::default()
    }

    fn spawn_virtual_pad(
        &mut self,
        _kind: VirtualTargetKind,
        _on_rumble_wake: Box<dyn Fn() + Send + Sync>,
    ) -> Result<Box<dyn VirtualPadHandle>, BackendError> {
        Err(BackendError::ReadOnly)
    }

    fn sync_cloaked_instances(&mut self, _instances: &BTreeSet<String>) -> Result<(), BackendError> {
        Ok(())
    }

    fn uncloak_all(&mut self) -> Result<(), BackendError> {
        Ok(())
    }
}

/// Platform-independent runtime slot manager for multi-controller virtual output & disconnect policy.
pub struct SlotCoordinator {
    slots: BTreeMap<String, VirtualSlot>,
}

pub struct VirtualSlot {
    pub key: HardwareKey,
    pub pad: Box<dyn VirtualPadHandle>,
    pub last_sent: Option<VirtualOutputReport>,
    pub connected: bool,
    pub instance_id: Option<String>,
}

impl SlotCoordinator {
    pub fn new() -> Self {
        Self {
            slots: BTreeMap::new(),
        }
    }

    pub fn ensure_pad(
        &mut self,
        key: &HardwareKey,
        target: VirtualTargetKind,
        instance_id: Option<String>,
        backend: &mut dyn OutputBackend,
        wake: Box<dyn Fn() + Send + Sync>,
    ) -> Result<&mut VirtualSlot, BackendError> {
        let id = key.canonical_id();
        let needs_spawn = match self.slots.get(&id) {
            Some(s) => s.pad.kind() != target,
            None => true,
        };
        if needs_spawn {
            self.slots.remove(&id);
            let pad = backend.spawn_virtual_pad(target, wake)?;
            self.slots.insert(
                id.clone(),
                VirtualSlot {
                    key: key.clone(),
                    pad,
                    last_sent: None,
                    connected: true,
                    instance_id,
                },
            );
        } else if let Some(s) = self.slots.get_mut(&id) {
            s.connected = true;
            if instance_id.is_some() {
                s.instance_id = instance_id;
            }
        }
        Ok(self.slots.get_mut(&id).unwrap())
    }

    pub fn submit_dedup(
        &mut self,
        key: &HardwareKey,
        report: &VirtualOutputReport,
    ) -> Result<bool, BackendError> {
        let Some(slot) = self.slots.get_mut(&key.canonical_id()) else {
            return Ok(false);
        };
        if slot.last_sent.as_ref() == Some(report) {
            return Ok(false);
        }
        match slot.pad.send_report(report) {
            Ok(()) => {
                slot.last_sent = Some(*report);
                Ok(true)
            }
            Err(BackendError::TargetNotReady) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn on_disconnect(
        &mut self,
        key: &HardwareKey,
        policy: DisconnectPolicy,
    ) -> Result<(), BackendError> {
        let id = key.canonical_id();
        match policy {
            DisconnectPolicy::DropVirtual => {
                self.slots.remove(&id);
            }
            DisconnectPolicy::KeepNeutral => {
                if let Some(slot) = self.slots.get_mut(&id) {
                    slot.connected = false;
                    let neutral = VirtualOutputReport::neutral(slot.pad.kind());
                    if slot.last_sent.as_ref() != Some(&neutral) {
                        let _ = slot.pad.send_report(&neutral);
                        slot.last_sent = Some(neutral);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, key: &HardwareKey) {
        self.slots.remove(&key.canonical_id());
    }

    pub fn get(&self, key: &HardwareKey) -> Option<&VirtualSlot> {
        self.slots.get(&key.canonical_id())
    }

    pub fn drain_rumble(&self, key: &HardwareKey) -> Option<RumbleCommand> {
        let slot = self.slots.get(&key.canonical_id())?;
        let rx = slot.pad.rumble_receiver()?;
        let mut latest = None;
        while let Ok(cmd) = rx.try_recv() {
            latest = Some(cmd);
        }
        latest
    }

    pub fn active_count(&self) -> usize {
        self.slots.len()
    }

    pub fn owns_xinput_user_index(&self, idx: u32) -> bool {
        self.slots
            .values()
            .any(|s| s.pad.kind() == VirtualTargetKind::Xbox360 && s.pad.xinput_user_index() == Some(idx))
    }

    pub fn has_active_x360(&self) -> bool {
        self.slots
            .values()
            .any(|s| s.pad.kind() == VirtualTargetKind::Xbox360)
    }
}

impl Default for SlotCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct MockPad {
        kind: VirtualTargetKind,
        serial: u32,
        sent: Arc<Mutex<Vec<VirtualOutputReport>>>,
    }

    impl VirtualPadHandle for MockPad {
        fn kind(&self) -> VirtualTargetKind {
            self.kind
        }
        fn serial_no(&self) -> u32 {
            self.serial
        }
        fn xinput_user_index(&self) -> Option<u32> {
            (self.kind == VirtualTargetKind::Xbox360).then(|| self.serial.saturating_sub(1))
        }
        fn send_report(&self, report: &VirtualOutputReport) -> Result<(), BackendError> {
            self.sent.lock().unwrap().push(*report);
            Ok(())
        }
        fn rumble_receiver(&self) -> Option<&Receiver<RumbleCommand>> {
            None
        }
    }

    struct MockBackend {
        next_serial: u32,
        sent: Arc<Mutex<Vec<VirtualOutputReport>>>,
    }

    impl OutputBackend for MockBackend {
        fn capabilities(&self) -> BackendCapabilities {
            BackendCapabilities {
                can_emulate_x360: true,
                can_emulate_ds4: true,
                can_cloak_physical: true,
                can_forward_rumble: true,
            }
        }
        fn driver_status(&self) -> DriverStatus {
            DriverStatus {
                vigem_installed: true,
                hidhide_installed: true,
                hidhide_accessible: true,
                ..Default::default()
            }
        }
        fn spawn_virtual_pad(
            &mut self,
            kind: VirtualTargetKind,
            _wake: Box<dyn Fn() + Send + Sync>,
        ) -> Result<Box<dyn VirtualPadHandle>, BackendError> {
            let serial = self.next_serial;
            self.next_serial += 1;
            Ok(Box::new(MockPad {
                kind,
                serial,
                sent: Arc::clone(&self.sent),
            }))
        }
        fn sync_cloaked_instances(&mut self, _i: &BTreeSet<String>) -> Result<(), BackendError> {
            Ok(())
        }
        fn uncloak_all(&mut self) -> Result<(), BackendError> {
            Ok(())
        }
    }

    #[test]
    fn disconnect_keep_neutral_vs_drop_virtual_and_dedup() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let mut backend = MockBackend {
            next_serial: 1,
            sent: Arc::clone(&sent),
        };
        let mut coord = SlotCoordinator::new();
        let k1 = HardwareKey::new(0x057E, 0x2009, "pad1");
        let k2 = HardwareKey::new(0x045E, 0x028E, "pad2");

        coord
            .ensure_pad(&k1, VirtualTargetKind::Xbox360, None, &mut backend, Box::new(|| {}))
            .unwrap();
        coord
            .ensure_pad(&k2, VirtualTargetKind::DualShock4, None, &mut backend, Box::new(|| {}))
            .unwrap();
        assert_eq!(coord.active_count(), 2);
        assert!(coord.has_active_x360());
        assert!(coord.owns_xinput_user_index(0));
        assert!(!coord.owns_xinput_user_index(1));

        let r1 = VirtualOutputReport::Xbox360(crate::model::X360OutputState {
            buttons: 0x1000,
            ..Default::default()
        });
        assert!(coord.submit_dedup(&k1, &r1).unwrap());
        assert!(!coord.submit_dedup(&k1, &r1).unwrap(), "duplicate report skipped");

        coord.on_disconnect(&k1, DisconnectPolicy::KeepNeutral).unwrap();
        assert_eq!(coord.active_count(), 2, "KeepNeutral preserves virtual pad");
        assert_eq!(
            sent.lock().unwrap().last(),
            Some(&VirtualOutputReport::neutral(VirtualTargetKind::Xbox360))
        );

        coord.on_disconnect(&k2, DisconnectPolicy::DropVirtual).unwrap();
        assert_eq!(coord.active_count(), 1, "DropVirtual removes virtual pad");
    }

    #[test]
    fn read_only_stub_compiles_for_future_macos_backend() {
        let mut ro = ReadOnlyNullBackend;
        assert!(!ro.capabilities().can_emulate_x360);
        assert!(matches!(
            ro.spawn_virtual_pad(VirtualTargetKind::Xbox360, Box::new(|| {})),
            Err(BackendError::ReadOnly)
        ));
    }
}
