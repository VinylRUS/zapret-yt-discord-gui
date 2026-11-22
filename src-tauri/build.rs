fn main() {
    // Embed a Windows application manifest requesting `requireAdministrator`
    // so the .exe always launches elevated (WinDivert/winws need admin).
    // Guarded by CARGO_CFG_WINDOWS so non-Windows / cargo-test hosts don't
    // try to invoke rc.exe.
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        let mut res = winres::WindowsResource::new();
        res.set_manifest(
            r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
      <supportedOS Id="{1f676c76-80e1-4239-95bb-83d0f6d0da78}"/>
    </application>
  </compatibility>
</assembly>"#,
        );
        if let Err(e) = res.compile() {
            // Don't hard-fail: a missing rc.exe on a dev host shouldn't abort
            // the whole build. The NSIS-bundled release build (windows-latest
            // CI) always has the Windows SDK, so the manifest WILL be embedded
            // there.
            println!("cargo:warning=winres manifest embed skipped: {e}");
        }
    }
    tauri_build::build()
}
