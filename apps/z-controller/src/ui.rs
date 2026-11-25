use egui::{Color32, Pos2, Rect, RichText, Stroke, Vec2};
use z_core::battery::BatteryStatus;
use z_core::model::{DisconnectPolicy, LogicalButton, VirtualTargetKind};
use z_core::profile::Profile;
use z_platform_windows::drivers::query_process_memory_bytes;

use crate::engine::{ControllerSnapshot, EngineCommand, EngineHandle, EngineSnapshot, VirtualOutputSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainTab {
    ProfileEditor,
    LiveTest,
}

pub struct AppUiState {
    pub selected_controller: Option<String>,
    pub active_tab: MainTab,
    pub editing_profile_id: Option<String>,
    pub draft_profile: Option<Profile>,
    pub draft_dirty: bool,
}

impl Default for AppUiState {
    fn default() -> Self {
        Self {
            selected_controller: None,
            active_tab: MainTab::ProfileEditor,
            editing_profile_id: None,
            draft_profile: None,
            draft_dirty: false,
        }
    }
}

pub fn draw_app_ui(ctx: &egui::Context, state: &mut AppUiState, engine: &EngineHandle) {
    let snap = engine.snapshot();

    if state.selected_controller.is_none() || !snap.controllers.iter().any(|c| Some(&c.canonical_id) == state.selected_controller.as_ref()) {
        state.selected_controller = snap.controllers.first().map(|c| c.canonical_id.clone());
    }

    let selected_ctrl = snap
        .controllers
        .iter()
        .find(|c| Some(&c.canonical_id) == state.selected_controller.as_ref())
        .cloned();

    let desired_profile_id = selected_ctrl
        .as_ref()
        .map(|c| c.profile_id.clone())
        .or_else(|| Some(snap.config.default_profile_id.clone()));

    if let Some(pid) = desired_profile_id {
        if state.editing_profile_id.as_deref() != Some(&pid) && !state.draft_dirty {
            state.editing_profile_id = Some(pid.clone());
            state.draft_profile = Some(snap.config.profile_by_id(&pid).clone());
        }
    }

    // Bottom status bar (driver readiness, HidHide lock warnings, live RAM readout — NO popups)
    egui::TopBottomPanel::bottom("status_bar")
        .exact_height(34.0)
        .show(ctx, |ui| {
            draw_status_bar(ui, &snap, engine);
        });

    // Left sidebar: Controller list with view-only battery & manual profile selector
    egui::SidePanel::left("controllers_panel")
        .resizable(false)
        .exact_width(305.0)
        .show(ctx, |ui| {
            draw_controller_list(ui, state, &snap, engine);
        });

    // Main area: Tabs for Profile Editor and Live Input Test
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut state.active_tab, MainTab::ProfileEditor, "Profile Editor");
            ui.selectable_value(&mut state.active_tab, MainTab::LiveTest, "Live Input Test");
        });
        ui.separator();

        match state.active_tab {
            MainTab::ProfileEditor => draw_profile_editor(ui, state, &snap, engine),
            MainTab::LiveTest => draw_live_test(ui, selected_ctrl.as_ref()),
        }
    });
}

fn draw_status_bar(ui: &mut egui::Ui, snap: &EngineSnapshot, engine: &EngineHandle) {
    ui.horizontal_centered(|ui| {
        let ds = &snap.driver_status;
        let vigem_col = if ds.vigem_installed {
            Color32::from_rgb(90, 210, 120)
        } else {
            Color32::from_rgb(240, 95, 85)
        };
        ui.colored_label(
            vigem_col,
            if ds.vigem_installed { "ViGEmBus: OK" } else { "ViGEmBus: Missing" },
        );
        ui.separator();

        let (hh_text, hh_col) = if !ds.hidhide_installed {
            ("HidHide: Missing", Color32::from_rgb(240, 95, 85))
        } else if ds.hidhide_accessible {
            ("HidHide: Ready", Color32::from_rgb(90, 210, 120))
        } else {
            ("HidHide: Locked", Color32::from_rgb(245, 185, 65))
        };
        ui.colored_label(hh_col, hh_text);

        if !ds.vigem_installed || !ds.hidhide_installed {
            if ui.button("Install Missing Drivers").clicked() {
                engine.send(EngineCommand::InstallDrivers);
            }
        }
        if ds.reboot_recommended {
            ui.separator();
            ui.colored_label(
                Color32::from_rgb(245, 195, 70),
                "Reboot recommended to finish HidHide setup",
            );
        } else if let Some(err) = &ds.hidhide_error {
            ui.separator();
            ui.colored_label(Color32::from_rgb(245, 185, 65), err);
        } else if let Some(msg) = &snap.last_status_message {
            ui.separator();
            ui.label(msg);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (ws, priv_b) = query_process_memory_bytes();
            let ws_mb = ws as f64 / (1024.0 * 1024.0);
            let priv_mb = priv_b as f64 / (1024.0 * 1024.0);
            ui.small(format!("RAM: {ws_mb:.1} MB (Priv {priv_mb:.1} MB)"));
        });
    });
}

fn draw_controller_list(
    ui: &mut egui::Ui,
    state: &mut AppUiState,
    snap: &EngineSnapshot,
    engine: &EngineHandle,
) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.heading("Controllers");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Refresh").clicked() {
                engine.send(EngineCommand::RefreshStatus);
            }
        });
    });
    ui.separator();

    if snap.controllers.is_empty() {
        ui.add_space(12.0);
        ui.label("No physical controllers detected.");
        ui.small("Connect a controller via Bluetooth or USB.");
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for ctrl in &snap.controllers {
            let is_selected = state.selected_controller.as_deref() == Some(&ctrl.canonical_id);
            let frame_fill = if is_selected {
                Color32::from_rgb(42, 52, 70)
            } else {
                Color32::from_rgb(30, 34, 42)
            };
            egui::Frame::none()
                .fill(frame_fill)
                .rounding(6.0)
                .inner_margin(8.0)
                .stroke(Stroke::new(
                    1.0_f32,
                    if is_selected {
                        Color32::from_rgb(95, 155, 245)
                    } else {
                        Color32::from_rgb(52, 58, 70)
                    },
                ))
                .show(ui, |ui| {
                    let header_resp = ui.horizontal(|ui| {
                        ui.strong(&ctrl.name);
                    });
                    if ui
                        .interact(
                            header_resp.response.rect,
                            ui.id().with(&ctrl.canonical_id),
                            egui::Sense::click(),
                        )
                        .clicked()
                    {
                        state.selected_controller = Some(ctrl.canonical_id.clone());
                        state.draft_dirty = false;
                        state.editing_profile_id = Some(ctrl.profile_id.clone());
                        state.draft_profile =
                            Some(snap.config.profile_by_id(&ctrl.profile_id).clone());
                    }

                    ui.horizontal(|ui| {
                        ui.small(format!(
                            "{} | {} | {:04X}:{:04X}",
                            ctrl.family.label(),
                            ctrl.transport.label(),
                            ctrl.key.vid,
                            ctrl.key.pid
                        ));
                    });

                    // View-only battery indicator
                    ui.horizontal(|ui| {
                        let batt_color = match (ctrl.battery.percent, ctrl.battery.status) {
                            (_, BatteryStatus::Wired) => Color32::from_rgb(120, 195, 250),
                            (Some(p), _) if p <= 20 => Color32::from_rgb(245, 95, 85),
                            (Some(p), _) if p <= 50 => Color32::from_rgb(245, 195, 70),
                            _ => Color32::from_rgb(95, 215, 130),
                        };
                        ui.colored_label(
                            batt_color,
                            format!("Battery: {}", ctrl.battery.format_badge()),
                        );
                    });

                    ui.horizontal(|ui| {
                        let mut enabled = ctrl.enabled;
                        if ui.checkbox(&mut enabled, "Active").changed() {
                            engine.send(EngineCommand::SetDeviceEnabled {
                                key: ctrl.key.clone(),
                                enabled,
                            });
                        }
                        if ctrl.virtual_active {
                            ui.colored_label(
                                Color32::from_rgb(130, 210, 145),
                                format!("-> {}", ctrl.target_kind.label()),
                            );
                        }
                        ui.colored_label(
                            if ctrl.cloaked {
                                Color32::from_rgb(120, 195, 250)
                            } else {
                                Color32::GRAY
                            },
                            if ctrl.cloaked { "Cloaked" } else { "Visible" },
                        );
                    });

                    ui.horizontal(|ui| {
                        ui.small("Profile:");
                        let mut chosen = ctrl.profile_id.clone();
                        egui::ComboBox::from_id_salt(("ctrl_prof", &ctrl.canonical_id))
                            .width(195.0)
                            .selected_text(&ctrl.profile_name)
                            .show_ui(ui, |ui| {
                                for p in &snap.config.profiles {
                                    if ui
                                        .selectable_value(&mut chosen, p.id.clone(), &p.name)
                                        .changed()
                                    {
                                        engine.send(EngineCommand::SelectDeviceProfile {
                                            key: ctrl.key.clone(),
                                            profile_id: chosen.clone(),
                                        });
                                        state.draft_dirty = false;
                                        state.editing_profile_id = Some(chosen.clone());
                                        state.draft_profile = Some(p.clone());
                                    }
                                }
                            });
                    });
                });
            ui.add_space(6.0);
        }
    });
}

fn draw_profile_editor(
    ui: &mut egui::Ui,
    state: &mut AppUiState,
    snap: &EngineSnapshot,
    engine: &EngineHandle,
) {
    ui.horizontal(|ui| {
        ui.label("Editing Profile:");
        let cur_id = state
            .editing_profile_id
            .clone()
            .unwrap_or_else(|| snap.config.default_profile_id.clone());
        let cur_name = state
            .draft_profile
            .as_ref()
            .map(|p| p.name.as_str())
            .unwrap_or("Select");
        egui::ComboBox::from_id_salt("profile_selector")
            .width(240.0)
            .selected_text(cur_name)
            .show_ui(ui, |ui| {
                for p in &snap.config.profiles {
                    if ui
                        .selectable_label(p.id == cur_id, &p.name)
                        .clicked()
                    {
                        state.editing_profile_id = Some(p.id.clone());
                        state.draft_profile = Some(p.clone());
                        state.draft_dirty = false;
                    }
                }
            });

        if ui.button("+ New").clicked() {
            let n = snap.config.profiles.len() + 1;
            let mut new_prof = Profile::default_x360();
            new_prof.id = format!("custom-{n}");
            new_prof.name = format!("Custom Profile {n}");
            state.editing_profile_id = Some(new_prof.id.clone());
            state.draft_profile = Some(new_prof.clone());
            state.draft_dirty = false;
            engine.send(EngineCommand::UpsertProfile(new_prof));
        }

        if snap.config.profiles.len() > 1 {
            if let Some(id) = &state.editing_profile_id {
                if id != "x360-fifa" && id != "ds4-default" && ui.button("Delete").clicked() {
                    engine.send(EngineCommand::DeleteProfile(id.clone()));
                    state.editing_profile_id = None;
                    state.draft_profile = None;
                    state.draft_dirty = false;
                    return;
                }
            }
        }
    });

    ui.separator();

    let Some(prof) = state.draft_profile.as_mut() else {
        ui.label("Select a profile to edit.");
        return;
    };

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("profile_settings_grid")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Profile Name:");
                if ui.text_edit_singleline(&mut prof.name).changed() {
                    state.draft_dirty = true;
                }
                ui.end_row();

                ui.label("Virtual Output Target:");
                ui.horizontal(|ui| {
                    if ui
                        .radio_value(&mut prof.target, VirtualTargetKind::Xbox360, "Xbox 360")
                        .changed()
                    {
                        state.draft_dirty = true;
                    }
                    if ui
                        .radio_value(&mut prof.target, VirtualTargetKind::DualShock4, "DualShock 4")
                        .changed()
                    {
                        state.draft_dirty = true;
                    }
                });
                ui.end_row();

                ui.label("HidHide Cloaking:");
                if ui
                    .checkbox(&mut prof.cloak_physical, "Hide physical controller from games")
                    .changed()
                {
                    state.draft_dirty = true;
                }
                ui.end_row();

                ui.label("Disconnect Behaviour:");
                ui.horizontal(|ui| {
                    if ui
                        .radio_value(
                            &mut prof.disconnect_policy,
                            DisconnectPolicy::KeepNeutral,
                            "Keep virtual pad at neutral (default)",
                        )
                        .changed()
                    {
                        state.draft_dirty = true;
                    }
                    if ui
                        .radio_value(
                            &mut prof.disconnect_policy,
                            DisconnectPolicy::DropVirtual,
                            "Drop virtual pad",
                        )
                        .changed()
                    {
                        state.draft_dirty = true;
                    }
                });
                ui.end_row();

                ui.label("Force Feedback:");
                if ui
                    .checkbox(&mut prof.rumble_enabled, "Forward game rumble to physical pad")
                    .changed()
                {
                    state.draft_dirty = true;
                }
                ui.end_row();

                ui.label("Face Layout:");
                if ui
                    .checkbox(&mut prof.swap_ab_xy, "Swap A/B & X/Y (Nintendo <-> Xbox)")
                    .changed()
                {
                    state.draft_dirty = true;
                }
                ui.end_row();

                ui.label("Left Stick Deadzone:");
                let mut l_pct = (prof.left_deadzone * 100.0).round();
                if ui
                    .add(egui::Slider::new(&mut l_pct, 0.0..=50.0).suffix("%"))
                    .changed()
                {
                    prof.left_deadzone = l_pct / 100.0;
                    state.draft_dirty = true;
                }
                ui.end_row();

                ui.label("Right Stick Deadzone:");
                let mut r_pct = (prof.right_deadzone * 100.0).round();
                if ui
                    .add(egui::Slider::new(&mut r_pct, 0.0..=50.0).suffix("%"))
                    .changed()
                {
                    prof.right_deadzone = r_pct / 100.0;
                    state.draft_dirty = true;
                }
                ui.end_row();
            });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.strong("Per-Button Remapping (Level 2)");
            if ui.small_button("Reset All Remaps").clicked() {
                prof.button_remap.clear();
                state.draft_dirty = true;
            }
        });
        ui.small("Override any logical SDL3 button to another virtual button or Unbound.");
        ui.add_space(4.0);

        egui::Grid::new("remap_grid")
            .num_columns(3)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for src in LogicalButton::ALL {
                    ui.label(src.short_label());
                    let current = prof.button_remap.get(&src).copied();
                    let label = match current {
                        None => "Default (Pass)".to_string(),
                        Some(None) => "Unbound".to_string(),
                        Some(Some(dst)) => dst.short_label().to_string(),
                    };
                    egui::ComboBox::from_id_salt(("remap", src))
                        .width(155.0)
                        .selected_text(label)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(current.is_none(), "Default (Pass)").clicked() {
                                prof.button_remap.remove(&src);
                                state.draft_dirty = true;
                            }
                            if ui.selectable_label(current == Some(None), "Unbound").clicked() {
                                prof.button_remap.insert(src, None);
                                state.draft_dirty = true;
                            }
                            for dst in LogicalButton::ALL {
                                if ui
                                    .selectable_label(current == Some(Some(dst)), dst.short_label())
                                    .clicked()
                                {
                                    prof.button_remap.insert(src, Some(dst));
                                    state.draft_dirty = true;
                                }
                            }
                        });
                    ui.label("");
                    ui.end_row();
                }
            });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let btn_text = if state.draft_dirty {
                "Apply & Save Profile *"
            } else {
                "Apply & Save Profile"
            };
            if ui.button(RichText::new(btn_text).strong()).clicked() {
                engine.send(EngineCommand::UpsertProfile(prof.clone()));
                state.draft_dirty = false;
            }
        });
    });
}

fn draw_live_test(ui: &mut egui::Ui, ctrl: Option<&ControllerSnapshot>) {
    let Some(c) = ctrl else {
        ui.label("Select a connected controller from the sidebar to inspect live inputs.");
        return;
    };

    ui.horizontal(|ui| {
        ui.strong(&c.name);
        ui.separator();
        ui.label(format!("Profile: {}", c.profile_name));
        ui.separator();
        ui.label(format!("Output: {}", c.target_kind.label()));
    });
    ui.add_space(8.0);

    let (vx_l, vy_l, vx_r, vy_r, vlt, vrt) = match c.virtual_output {
        VirtualOutputSnapshot::Xbox360 {
            thumb_lx,
            thumb_ly,
            thumb_rx,
            thumb_ry,
            left_trigger,
            right_trigger,
            ..
        } => (
            thumb_lx as f32 / 32767.0,
            -(thumb_ly as f32 / 32767.0), // screen down is +Y
            thumb_rx as f32 / 32767.0,
            -(thumb_ry as f32 / 32767.0),
            left_trigger as f32 / 255.0,
            right_trigger as f32 / 255.0,
        ),
        VirtualOutputSnapshot::DualShock4 {
            thumb_lx,
            thumb_ly,
            thumb_rx,
            thumb_ry,
            trigger_left,
            trigger_right,
            ..
        } => (
            (thumb_lx as f32 - 128.0) / 127.0,
            (thumb_ly as f32 - 128.0) / 127.0,
            (thumb_rx as f32 - 128.0) / 127.0,
            (thumb_ry as f32 - 128.0) / 127.0,
            trigger_left as f32 / 255.0,
            trigger_right as f32 / 255.0,
        ),
        VirtualOutputSnapshot::None => (0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
    };

    ui.horizontal(|ui| {
        draw_stick_radar(
            ui,
            "Left Stick",
            c.physical_state.left_x as f32 / 32767.0,
            c.physical_state.left_y as f32 / 32767.0,
            vx_l,
            vy_l,
        );
        ui.add_space(18.0);
        draw_stick_radar(
            ui,
            "Right Stick",
            c.physical_state.right_x as f32 / 32767.0,
            c.physical_state.right_y as f32 / 32767.0,
            vx_r,
            vy_r,
        );
        ui.add_space(18.0);
        ui.vertical(|ui| {
            ui.strong("Triggers");
            ui.add_space(6.0);
            ui.label(format!("L2 / LT ({:.0}%)", vlt * 100.0));
            ui.add(egui::ProgressBar::new(vlt).desired_width(180.0));
            ui.add_space(8.0);
            ui.label(format!("R2 / RT ({:.0}%)", vrt * 100.0));
            ui.add(egui::ProgressBar::new(vrt).desired_width(180.0));
            ui.add_space(10.0);
            ui.small(format!(
                "Physical SDL3 L: ({}, {}) R: ({}, {})",
                c.physical_state.left_x,
                c.physical_state.left_y,
                c.physical_state.right_x,
                c.physical_state.right_y
            ));
            ui.small(format!("Virtual Output: {:?}", c.virtual_output));
        });
    });

    ui.add_space(12.0);
    ui.strong("Buttons (Physical Pressed)");
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        for btn in LogicalButton::ALL {
            let down = (c.physical_state.buttons_mask & btn.bit()) != 0;
            let fill = if down {
                Color32::from_rgb(70, 165, 250)
            } else {
                Color32::from_rgb(36, 40, 48)
            };
            let txt_col = if down { Color32::WHITE } else { Color32::GRAY };
            egui::Frame::none()
                .fill(fill)
                .rounding(4.0)
                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                .show(ui, |ui| {
                    ui.colored_label(txt_col, btn.short_label());
                });
        }
    });
}

fn draw_stick_radar(
    ui: &mut egui::Ui,
    title: &str,
    phys_x: f32,
    phys_y: f32,
    virt_x: f32,
    virt_y: f32,
) {
    ui.vertical(|ui| {
        ui.strong(title);
        let size = 140.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
        let p = ui.painter();
        let center = rect.center();
        let radius = size * 0.45;
        p.rect_filled(rect, 6.0, Color32::from_rgb(24, 27, 34));
        p.circle_stroke(center, radius, Stroke::new(1.5_f32, Color32::from_rgb(70, 80, 95)));
        p.line_segment(
            [Pos2::new(center.x - radius, center.y), Pos2::new(center.x + radius, center.y)],
            Stroke::new(1.0_f32, Color32::from_rgb(45, 52, 62)),
        );
        p.line_segment(
            [Pos2::new(center.x, center.y - radius), Pos2::new(center.x, center.y + radius)],
            Stroke::new(1.0_f32, Color32::from_rgb(45, 52, 62)),
        );

        let phys_pt = Pos2::new(
            center.x + phys_x.clamp(-1.0, 1.0) * radius,
            center.y + phys_y.clamp(-1.0, 1.0) * radius,
        );
        p.circle_stroke(phys_pt, 5.0, Stroke::new(1.5_f32, Color32::from_rgb(220, 180, 70)));

        let virt_pt = Pos2::new(
            center.x + virt_x.clamp(-1.0, 1.0) * radius,
            center.y + virt_y.clamp(-1.0, 1.0) * radius,
        );
        p.circle_filled(virt_pt, 5.0, Color32::from_rgb(80, 185, 250));
        let _ = Rect::NOTHING;
    });
}
