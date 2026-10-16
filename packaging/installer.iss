#include "compiler-version.iss"
#ifndef AppVersion
  #error AppVersion is required
#endif
#ifndef WindowsVersion
  #error WindowsVersion is required
#endif
#ifndef BundleDir
  #error BundleDir is required
#endif
#ifndef ReleaseDir
  #error ReleaseDir is required
#endif

[Setup]
AppId={{78F36AC5-1367-4498-B486-50E3326B9426}
AppName=MetinBot
AppVersion={#AppVersion}
AppPublisher=MetinBot
VersionInfoVersion={#WindowsVersion}
DefaultDirName={autopf}\MetinBot
DisableDirPage=yes
DefaultGroupName=MetinBot
DisableProgramGroupPage=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.19041
OutputDir={#ReleaseDir}
OutputBaseFilename=MetinBotSetup-{#AppVersion}-windows-x64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\MetinBot.exe
CloseApplications=yes
CloseApplicationsFilter=MetinBot.exe
RestartApplications=no
SetupLogging=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "czech"; MessagesFile: "compiler:Languages\Czech.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[InstallDelete]
; Replace the generated dependency tree on upgrades, including removed modules.
Type: filesandordirs; Name: "{app}\_internal"

[Files]
Source: "{#BundleDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\MetinBot"; Filename: "{app}\MetinBot.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\MetinBot"; Filename: "{app}\MetinBot.exe"; WorkingDir: "{app}"; Tasks: desktopicon

; User profiles and settings live outside {app}; uninstall deliberately retains them.
