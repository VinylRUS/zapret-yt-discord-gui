// Embed a Windows application manifest requesting `requireAdministrator`
// so the .exe always launches elevated (WinDivert/winws need admin).
//
// The manifest MUST be fed through tauri-build's own WindowsAttributes:
// using a separate `winres` resource alongside `tauri_build::build()`
// embeds the VERSION resource twice and the linker fails with
// `CVTRES fatal error CVT1100: duplicate resource type:VERSION`.
// See https://github.com/tauri-apps/tauri/issues/10154
//
// The Common-Controls v6 <dependency> is REQUIRED: without it the app loads
// ComCtl32 v5 and crashes on startup with
// `Entry point TaskDialogIndirect not found in zapret-gui.exe`.
// (winres used to inject this block automatically; now we must do it by hand.)
const WINDOWS_MANIFEST: &str = r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
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
</assembly>"#;

fn main() {
    let windows = tauri_build::WindowsAttributes::new().app_manifest(WINDOWS_MANIFEST);
    let attrs = tauri_build::Attributes::new().windows_attributes(windows);
    tauri_build::try_build(attrs).expect("failed to run tauri-build");
}
