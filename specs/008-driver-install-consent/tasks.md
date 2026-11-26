# Tasks: `driver-install-consent`

- [x] T001 Create `specs/008-driver-install-consent/{spec.md,plan.md,tasks.md}` (`driver-install-consent`)
- [x] T002 Implement `crates/z-core/src/driver.rs` state machine, locate inspection, and unit tests
- [x] T003 Update `crates/z-core/src/{lib.rs,backend.rs,profile.rs}` with typed `DriverStatus` and persisted locate paths
- [x] T004 Enable Windows Security/WinTrust/Services features in `crates/z-platform-windows/Cargo.toml` and `Win32_System_Com`/`Win32_UI_Shell_Common` in `apps/z-controller/Cargo.toml`
- [x] T005 Implement 4-layer Windows probing (`functional -> SCM service -> PnP -> uninstall`), remedies, `Win32SignatureVerifier`, `AppLog`, and no-silent single-driver wizard launcher in `crates/z-platform-windows/src/drivers.rs` + `backend_impl.rs`
- [x] T006 Wire per-driver consent/cancel/remedy/locate commands, missing-ViGEmBus toggle guard, and optional HidHide degradation in `apps/z-controller/src/engine.rs`
- [x] T007 Implement `SHBrowseForFolderW` picker + modal confirmation + per-driver `ViGEmBus` / `HidHide` cards + missing-driver UI guards in `apps/z-controller/src/{ui.rs,tray_win32.rs}`
- [x] T008 Add `--dry-run-install`, `--log-path`, `--preview-confirm-driver`, and updated `--preview-missing-drivers` in `apps/z-controller/src/main.rs`; update `README.md`
- [x] T009 Add UI + engine unit tests (consent confirm/cancel, zero combined install button, missing-ViGEmBus toggle guard, locate verifier)
- [x] T010 Run `cargo test --workspace` and verify 100% pass with 0 warnings
- [x] T011 Build release exe (`cargo build --release`), copy to `Y:\Apps\z-controller\z-controller.exe`, verify tray WorkingSet `<= 25 MB` and dry-run log proof
- [x] T012 Capture and inspect verification screenshots, write `specs/008-driver-install-consent/verification.md`, and commit on `main`
