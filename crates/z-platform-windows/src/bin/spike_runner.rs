use std::thread;
use std::time::{Duration, Instant};

use z_platform_windows::hidhide::{self, HidHideError};
use z_platform_windows::pnp::{HidOpenStatus, enumerate_hid_devices, interface_path_to_instance_id};
use z_platform_windows::raw_battery::{read_switch_hid_battery, read_xinput_battery};
use z_platform_windows::vigem::{VigemClient, VigemError, X360Buttons, X360Report};
use z_sdl3::SdlContext;

fn main() {
    let mut exit_code = 0;
    if let Err(err) = run_spike() {
        eprintln!("SPIKE ERROR: {err}");
        exit_code = 1;
    }
    if let Err(e) = hidhide::emergency_uncloak() {
        eprintln!("CLEANUP UNCLOAK ERROR: {e}");
        exit_code = 1;
    }
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

fn run_spike() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Z-CONTROLLER FEASIBILITY SPIKE ===");

    // Ensure probe_game_view sits beside spike_runner.exe
    let exe_dir = std::env::current_exe()?
        .parent()
        .ok_or("no exe parent")?
        .to_path_buf();
    let probe_exe = exe_dir.join("probe_game_view.exe");
    if !probe_exe.exists() {
        return Err(format!("missing probe_game_view.exe at {}", probe_exe.display()).into());
    }

    // 1. Whitelist spike_runner.exe first (never probe_game_view.exe!)
    println!("\n[Setup] Checking HidHide status & whitelisting spike_runner.exe...");
    match hidhide::whitelist_current_exe() {
        Ok(nt_path) => println!("  Whitelisted NT path: {nt_path}"),
        Err(HidHideError::AccessDenied) => {
            return Err("HidHide \\\\.\\HidHide returned ERROR_ACCESS_DENIED; close HidHideClient.exe before running spike".into());
        }
        Err(e) => return Err(Box::new(e)),
    }
    let _ = hidhide::emergency_uncloak();

    // Baseline un-whitelisted game view BEFORE ViGEm or Cloak
    println!("\n[Baseline] Un-whitelisted probe_game_view BEFORE ViGEm & Cloak:");
    run_probe(&probe_exe)?;

    // 2. ITEM 1: SDL3 reads Aolion Pro Controller (057E:2009) + battery level
    println!("\n[Item 1] Initializing SDL3 3.4.20 + gamecontrollerdb.txt...");
    let sdl = SdlContext::init()?;
    let mut ids = sdl.gamepad_ids()?;
    if ids.is_empty() {
        thread::sleep(Duration::from_millis(300));
        while sdl.poll_event().is_some() {}
        ids = sdl.gamepad_ids()?;
    }

    println!("  SDL3 discovered {} gamepad(s):", ids.len());
    let mut pro_instance_id: Option<String> = None;
    let mut pro_sdl_id: Option<u32> = None;
    let mut opened_pads = Vec::new();

    for id in ids {
        let gp = sdl.open_gamepad(id)?;
        // Pump once so HIDAPI reads initial packet + battery
        thread::sleep(Duration::from_millis(120));
        while sdl.poll_event().is_some() {}
        let vid = gp.vendor();
        let pid = gp.product();
        let name = gp.name();
        let path = gp.path().unwrap_or_default();
        let pnp_id = interface_path_to_instance_id(&path);
        let pwr = gp.power_info();
        let conn = gp.connection_state();
        let raw_batt = if vid == 0x057E {
            read_switch_hid_battery(&path)
        } else if vid == 0x045E {
            read_xinput_battery(0)
        } else {
            None
        };
        println!(
            "  - id={} name='{}' VID={:04X} PID={:04X} type={:?} conn={:?} sdl_power=({:?}, {:?}%) raw_batt={:?} path='{}' pnp={:?}",
            gp.id(),
            name,
            vid,
            pid,
            gp.gamepad_type(),
            conn,
            pwr.state,
            pwr.percent,
            raw_batt,
            path,
            pnp_id
        );
        if vid == 0x057E && pid == 0x2009 {
            pro_instance_id = pnp_id;
            pro_sdl_id = Some(gp.id());
        }
        opened_pads.push(gp);
    }

    let pro_pnp = pro_instance_id
        .ok_or("Aolion Pro Controller (057E:2009) was not found via SDL3")?;
    let pro_id = pro_sdl_id.unwrap();
    let pro_pad = opened_pads.iter().find(|g| g.id() == pro_id).unwrap();
    let pro_pwr = pro_pad.power_info();
    let item1_pass = pro_pwr.percent.is_some();
    println!(
        "[Item 1 Result] {} (057E:2009 state={:?}, percent={:?}%)",
        if item1_pass { "PASS" } else { "FAIL" },
        pro_pwr.state,
        pro_pwr.percent
    );

    // 3. ITEM 2: ViGEm creates virtual Xbox 360 pad visible in joy.cpl and readable by game probe
    println!("\n[Item 2] Connecting to ViGEmBus and plugging virtual Xbox 360 controller...");
    let vigem = VigemClient::connect()?;
    let x360 = vigem.plug_x360()?;
    println!("  Plugged ViGEm X360 serial={}", x360.serial_no());

    // Wait briefly for PnP XInput enumeration and inject non-zero synthetic input
    let injected = X360Report {
        buttons: X360Buttons::A.bits() | X360Buttons::RIGHT_SHOULDER.bits(),
        left_trigger: 128,
        right_trigger: 255,
        thumb_lx: -12000,
        thumb_ly: 24000,
        thumb_rx: 8000,
        thumb_ry: -16000,
    };
    for _ in 0..50 {
        match x360.update_x360(&injected) {
            Ok(()) => break,
            Err(VigemError::TargetNotReady { .. }) => thread::sleep(Duration::from_millis(40)),
            Err(e) => return Err(Box::new(e)),
        }
    }
    thread::sleep(Duration::from_millis(300));
    let _ = x360.update_x360(&injected);

    println!("\n[Item 2] Un-whitelisted probe_game_view WITH ViGEm X360 active (before cloak):");
    let out_item2 = run_probe(&probe_exe)?;
    let has_xinput_match = out_item2.contains("wButtons=0x1200") && out_item2.contains("sThumbLY=24000");
    let joy_cpl_before = read_joy_cpl_list();
    println!("  joy.cpl Game Controllers UI list (before cloak): {:?}", joy_cpl_before);
    println!(
        "[Item 2 Result] {} (virtual X360 serial={} injected A+RB+LY=+24000, XInput readback matched={})",
        if has_xinput_match { "PASS" } else { "FAIL" },
        x360.serial_no(),
        has_xinput_match
    );

    // 4. ITEM 3: HidHide cloaks physical Pro Controller from joy.cpl & games while spike_runner still reads SDL3
    println!("\n[Item 3] Cloaking physical instance '{}' via HidHide IOCTL...", pro_pnp);
    let hid_state = hidhide::cloak_instance(&pro_pnp)?;
    println!(
        "  HidHide active={}, blacklist={:?}, whitelist_len={}",
        hid_state.active,
        hid_state.blacklist,
        hid_state.whitelist.len()
    );
    // Keep sending synthetic X360 input and wait briefly for OS HID visibility update
    let _ = x360.update_x360(&injected);
    thread::sleep(Duration::from_millis(500));

    println!("\n[Item 3] Un-whitelisted probe_game_view WHILE CLOAKED:");
    let out_cloaked = run_probe(&probe_exe)?;
    let joy_cpl_cloaked = read_joy_cpl_list();
    println!("  joy.cpl Game Controllers UI list (while cloaked): {:?}", joy_cpl_cloaked);

    // Confirm whitelisted spike_runner STILL sees and reads SDL3 + HID directly!
    while sdl.poll_event().is_some() {}
    let post_cloak_pwr = pro_pad.power_info();
    let post_cloak_ly = pro_pad.axis(z_sdl3::SdlGamepadAxis::LeftY);
    let post_cloak_a = pro_pad.button(z_sdl3::SdlGamepadButton::South);
    let hid_while_whitelisted = enumerate_hid_devices();
    let whitelisted_sees_057e = hid_while_whitelisted
        .iter()
        .any(|d| d.vendor_id == 0x057E && d.product_id == 0x2009 && d.open_status == HidOpenStatus::Ok);

    // Un-whitelisted probe must NOT see 057E:2009 open OK and DirectInput8 must NOT list Nintendo/Pro Controller
    let probe_blocked_057e = !out_cloaked.contains("VID=057E PID=2009 status=Ok")
        && !out_cloaked.contains("DI8 name='Pro Controller'")
        && !out_cloaked.contains("DI8 name='Wireless Gamepad'");

    // Keep report alive briefly so an external joy.cpl check or probe confirms steady state
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_millis(200) {
        let _ = sdl.wait_event_timeout(50);
    }

    println!(
        "  Whitelisted spike_runner SDL3 check: id={} pwr=({:?}, {:?}%) ly={} south={} raw_hid_open_ok={}",
        pro_pad.id(),
        post_cloak_pwr.state,
        post_cloak_pwr.percent,
        post_cloak_ly,
        post_cloak_a,
        whitelisted_sees_057e
    );
    let item3_pass = probe_blocked_057e && whitelisted_sees_057e && post_cloak_pwr.percent.is_some();
    println!(
        "[Item 3 Result] {} (un-whitelisted blocked={}, whitelisted SDL3+HID intact={})",
        if item3_pass { "PASS" } else { "FAIL" },
        probe_blocked_057e,
        whitelisted_sees_057e
    );

    // 5. Cleanly uncloak and unplug virtual pad
    println!("\n[Cleanup] Uncloaking physical pad and unplugging virtual X360 pad...");
    hidhide::emergency_uncloak()?;
    drop(x360);
    thread::sleep(Duration::from_millis(400));
    println!("\n[Post-Cleanup] Un-whitelisted probe_game_view AFTER uncloak & unplug:");
    run_probe(&probe_exe)?;

    if !(item1_pass && has_xinput_match && item3_pass) {
        return Err("One or more spike checks failed".into());
    }
    Ok(())
}

fn run_probe(probe_exe: &std::path::Path) -> Result<String, Box<dyn std::error::Error>> {
    let output = std::process::Command::new(probe_exe).output()?;
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    print!("{text}");
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("probe_game_view exited {}: {}", output.status, err).into());
    }
    Ok(text)
}

fn read_joy_cpl_list() -> Vec<String> {
    let script = r#"
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$proc = Start-Process -FilePath "rundll32.exe" -ArgumentList "shell32.dll,Control_RunDLL joy.cpl" -PassThru
$root = $null
for ($i = 0; $i -lt 30; $i++) {
    Start-Sleep -Milliseconds 150
    $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, "Game Controllers")
    $root = [System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
    if ($root -ne $null) { break }
}
if ($root -eq $null) {
    $proc | Stop-Process -Force -ErrorAction SilentlyContinue
    return
}
$listCond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::List)
$list = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $listCond)
if ($list -ne $null) {
    $itemCond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::ListItem)
    $items = $list.FindAll([System.Windows.Automation.TreeScope]::Children, $itemCond)
    foreach ($it in $items) {
        Write-Output $it.Current.Name
    }
}
$wp = $root.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)
if ($wp -ne $null) { $wp.Close() }
Start-Sleep -Milliseconds 100
$proc | Stop-Process -Force -ErrorAction SilentlyContinue
"#;
    let Ok(out) = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", script])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
