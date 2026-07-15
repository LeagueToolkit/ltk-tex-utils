//! The sparse-package manifest ("package with external location") that puts
//! the ltk-tex-utils commands into the Windows 11 modern context menu.
//!
//! Why a package at all: the modern menu builds its top level exclusively from
//! `IExplorerCommand` handlers of apps with package identity; classic registry
//! verbs only render under "Show more options". A *sparse* package carries
//! nothing but this manifest — the executable and the COM DLL stay in the
//! normal install directory, which is passed to Windows as the package's
//! "external location" at registration time.
//!
//! Why the *loose-manifest* (Developer Mode) registration path: a sparse
//! package normally has to be signed with a certificate the machine trusts,
//! which a tool distributed as a bare exe can't assume. Windows 11's unsigned
//! package support (`Add-AppxPackage -AllowUnsigned`) does not help here — it
//! rejects any manifest that declares `Executable` activations (deployment
//! error 0x80073D2B; the unsigned allowance is scoped to hosted apps), and the
//! context-menu extension requires an `Application` element. What remains is
//! Developer Mode's loose-manifest registration
//! (`Add-AppxPackage -Register <manifest> -ExternalLocation <dir>`), which
//! skips package integrity entirely. Registering a *signed* sparse package to
//! drop the Developer Mode requirement is possible follow-up work.

use ltk_tex_handler_shared::{
    CLI_EXE_FILE_NAME, CLSID_TEX_EXPLORER_COMMAND, HANDLER_DLL_FILE_NAME, MENU_LABEL,
    PACKAGE_IDENTITY_NAME, PACKAGE_PUBLISHER,
};

/// Four-part MSIX package version derived from the CLI crate version.
pub fn package_version() -> String {
    format!("{}.0", env!("CARGO_PKG_VERSION"))
}

/// The sparse package manifest: identity, external-content opt-in, the modern
/// context-menu verbs for `.tex`/`.dds`/`.png`/folders, and the packaged COM
/// class Explorer activates for them (hosted by the handler DLL sitting next
/// to the executable in the external location).
pub fn appx_manifest() -> String {
    let clsid = CLSID_TEX_EXPLORER_COMMAND.trim_matches(['{', '}']);
    let version = package_version();
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Package
  xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:uap10="http://schemas.microsoft.com/appx/manifest/uap/windows10/10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  xmlns:desktop4="http://schemas.microsoft.com/appx/manifest/desktop/windows10/4"
  xmlns:desktop5="http://schemas.microsoft.com/appx/manifest/desktop/windows10/5"
  xmlns:com="http://schemas.microsoft.com/appx/manifest/com/windows10"
  IgnorableNamespaces="uap uap10 rescap desktop4 desktop5 com">
  <Identity Name="{PACKAGE_IDENTITY_NAME}" Publisher="{PACKAGE_PUBLISHER}" Version="{version}" ProcessorArchitecture="neutral" />
  <Properties>
    <DisplayName>{MENU_LABEL}</DisplayName>
    <PublisherDisplayName>LeagueToolkit</PublisherDisplayName>
    <Logo>Assets\StoreLogo.png</Logo>
    <uap10:AllowExternalContent>true</uap10:AllowExternalContent>
  </Properties>
  <Resources>
    <Resource Language="en-us" />
  </Resources>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.22000.0" MaxVersionTested="10.0.26100.0" />
  </Dependencies>
  <Capabilities>
    <rescap:Capability Name="runFullTrust" />
    <rescap:Capability Name="unvirtualizedResources" />
  </Capabilities>
  <Applications>
    <Application Id="LtkTexUtils" Executable="{CLI_EXE_FILE_NAME}" uap10:TrustLevel="mediumIL" uap10:RuntimeBehavior="win32App">
      <uap:VisualElements AppListEntry="none" DisplayName="{MENU_LABEL}" Description="LeagueToolkit .tex conversion utilities" BackgroundColor="transparent" Square150x150Logo="Assets\Square150x150Logo.png" Square44x44Logo="Assets\Square44x44Logo.png" />
      <Extensions>
        <desktop4:Extension Category="windows.fileExplorerContextMenus">
          <desktop4:FileExplorerContextMenus>
            <desktop4:ItemType Type=".tex">
              <desktop4:Verb Id="LtkTexUtilsTex" Clsid="{clsid}" />
            </desktop4:ItemType>
            <desktop4:ItemType Type=".dds">
              <desktop4:Verb Id="LtkTexUtilsDds" Clsid="{clsid}" />
            </desktop4:ItemType>
            <desktop4:ItemType Type=".png">
              <desktop4:Verb Id="LtkTexUtilsPng" Clsid="{clsid}" />
            </desktop4:ItemType>
            <desktop5:ItemType Type="Directory">
              <desktop5:Verb Id="LtkTexUtilsDir" Clsid="{clsid}" />
            </desktop5:ItemType>
          </desktop4:FileExplorerContextMenus>
        </desktop4:Extension>
        <com:Extension Category="windows.comServer">
          <com:ComServer>
            <com:SurrogateServer DisplayName="{MENU_LABEL} context menu">
              <com:Class Id="{clsid}" Path="{HANDLER_DLL_FILE_NAME}" ThreadingModel="STA" />
            </com:SurrogateServer>
          </com:ComServer>
        </com:Extension>
      </Extensions>
    </Application>
  </Applications>
</Package>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_has_no_leftover_placeholders_or_braced_guids() {
        let m = appx_manifest();
        assert!(m.contains("AllowExternalContent"));
        assert!(!m.contains('{'), "GUIDs in the manifest must be braceless");
    }
}
