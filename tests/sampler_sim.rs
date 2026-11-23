use dmz_sbmm_reader::probe::Probe;
use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[test]
#[cfg(windows)]
fn sampler_captures_fake_cod_connection() {
    let dir = std::env::temp_dir().join(format!("dmz-sampler-sim-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let cod_exe = dir.join("cod.exe");
    fs::copy(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe", &cod_exe).unwrap();

    let mut listener = Command::new("powershell")
        .args(["-NoProfile", "-Command",
            "$l=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,19998);$l.Start();$c=$l.AcceptTcpClient();Start-Sleep -Seconds 20;$c.Close();$l.Stop()"])
        .stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();

    thread::sleep(Duration::from_millis(800));

    let mut cod = Command::new(&cod_exe)
        .args(["-NoProfile", "-Command",
            "$c=[Net.Sockets.TcpClient]::new();$c.Connect('127.0.0.1',19998);Start-Sleep -Seconds 20;$c.Close()"])
        .stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();

    thread::sleep(Duration::from_secs(2));

    let mut p = Probe::new(dir.clone(), vec![]);
    p.init_files().unwrap();
    p.sample_network().unwrap();

    let csv = fs::read_to_string(&p.paths.network_csv).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert!(lines.len() > 1, "no connection rows captured: {csv}");
    assert!(lines[1].contains("cod"), "row does not name cod: {}", lines[1]);

    let _ = cod.kill();
    let _ = listener.kill();
    let _ = fs::remove_dir_all(&dir);
}
