use std::collections::BTreeSet;
use std::sync::mpsc::{self, Receiver};

use z_core::backend::{
    BackendCapabilities, BackendError, DriverStatus, OutputBackend, VirtualPadHandle,
};
use z_core::model::{RumbleCommand, VirtualOutputReport, VirtualTargetKind};

use crate::drivers;
use crate::hidhide;
use crate::vigem::{Ds4Report, VigemClient, VigemError, VigemTarget, X360Report};

pub struct WindowsVirtualPad {
    target: VigemTarget,
    rumble_rx: Receiver<RumbleCommand>,
}

impl VirtualPadHandle for WindowsVirtualPad {
    fn kind(&self) -> VirtualTargetKind {
        match self.target.kind() {
            crate::vigem::VigemTargetKind::Xbox360 => VirtualTargetKind::Xbox360,
            crate::vigem::VigemTargetKind::DualShock4 => VirtualTargetKind::DualShock4,
        }
    }

    fn serial_no(&self) -> u32 {
        self.target.serial_no()
    }

    fn xinput_user_index(&self) -> Option<u32> {
        self.target.xinput_user_index()
    }

    fn send_report(&self, report: &VirtualOutputReport) -> Result<(), BackendError> {
        let res = match report {
            VirtualOutputReport::Xbox360(x) => self.target.update_x360(&X360Report {
                buttons: x.buttons,
                left_trigger: x.left_trigger,
                right_trigger: x.right_trigger,
                thumb_lx: x.thumb_lx,
                thumb_ly: x.thumb_ly,
                thumb_rx: x.thumb_rx,
                thumb_ry: x.thumb_ry,
            }),
            VirtualOutputReport::DualShock4(d) => self.target.update_ds4(&Ds4Report {
                thumb_lx: d.thumb_lx,
                thumb_ly: d.thumb_ly,
                thumb_rx: d.thumb_rx,
                thumb_ry: d.thumb_ry,
                buttons: d.buttons,
                special: d.special,
                trigger_left: d.trigger_left,
                trigger_right: d.trigger_right,
            }),
        };
        match res {
            Ok(()) => Ok(()),
            Err(VigemError::TargetNotReady { .. }) => Err(BackendError::TargetNotReady),
            Err(e) => Err(BackendError::Failed(e.to_string())),
        }
    }

    fn rumble_receiver(&self) -> Option<&Receiver<RumbleCommand>> {
        Some(&self.rumble_rx)
    }
}

pub struct WindowsOutputBackend {
    vigem: Option<VigemClient>,
    last_cloaked: BTreeSet<String>,
    pub reboot_recommended: bool,
}

impl WindowsOutputBackend {
    pub fn new() -> Self {
        let vigem = VigemClient::connect().ok();
        let _ = hidhide::whitelist_current_exe();
        Self {
            vigem,
            last_cloaked: BTreeSet::new(),
            reboot_recommended: false,
        }
    }

    fn ensure_client(&mut self) -> Result<&VigemClient, BackendError> {
        if self.vigem.is_none() {
            self.vigem = Some(
                VigemClient::connect().map_err(|e| BackendError::DriverUnavailable(e.to_string()))?,
            );
        }
        Ok(self.vigem.as_ref().unwrap())
    }
}

impl Default for WindowsOutputBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WindowsOutputBackend {
    fn drop(&mut self) {
        let _ = hidhide::emergency_uncloak();
    }
}

impl OutputBackend for WindowsOutputBackend {
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            can_emulate_x360: true,
            can_emulate_ds4: true,
            can_cloak_physical: true,
            can_forward_rumble: true,
        }
    }

    fn driver_status(&self) -> DriverStatus {
        drivers::detect_driver_status(self.reboot_recommended)
    }

    fn spawn_virtual_pad(
        &mut self,
        kind: VirtualTargetKind,
        on_rumble_wake: Box<dyn Fn() + Send + Sync>,
    ) -> Result<Box<dyn VirtualPadHandle>, BackendError> {
        let client = self.ensure_client()?;
        let mut target = match kind {
            VirtualTargetKind::Xbox360 => client.plug_x360(),
            VirtualTargetKind::DualShock4 => client.plug_ds4(),
        }
        .map_err(|e| BackendError::Failed(e.to_string()))?;

        let (tx, rx) = mpsc::channel();
        let wake = std::sync::Arc::new(on_rumble_wake);
        let wake_cb = std::sync::Arc::clone(&wake);
        target
            .start_rumble_listener(move |fb| {
                let _ = tx.send(RumbleCommand {
                    large_motor: fb.large_motor,
                    small_motor: fb.small_motor,
                });
                wake_cb();
            })
            .map_err(|e| BackendError::Failed(e.to_string()))?;

        Ok(Box::new(WindowsVirtualPad {
            target,
            rumble_rx: rx,
        }))
    }

    fn sync_cloaked_instances(&mut self, instances: &BTreeSet<String>) -> Result<(), BackendError> {
        if &self.last_cloaked == instances {
            return Ok(());
        }
        hidhide::set_cloak_instances(instances).map_err(|e| BackendError::Failed(e.to_string()))?;
        self.last_cloaked = instances.clone();
        Ok(())
    }

    fn uncloak_all(&mut self) -> Result<(), BackendError> {
        self.last_cloaked.clear();
        hidhide::emergency_uncloak().map_err(|e| BackendError::Failed(e.to_string()))
    }
}
