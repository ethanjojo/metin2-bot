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
#ifndef AppProductName
  #error AppProductName is required
#endif
#ifndef AppPublisher
  #error AppPublisher is required
#endif
#ifndef AppModelId
  #error AppModelId is required
#endif
#ifndef AppCopyright
  #error AppCopyright is required
#endif

[Setup]
AppId={{78F36AC5-1367-4498-B486-50E3326B9426}
AppName={#AppProductName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
VersionInfoVersion={#WindowsVersion}
VersionInfoCompany={#AppPublisher}
VersionInfoDescription={#AppProductName} Setup
VersionInfoCopyright={#AppCopyright}
SetupIconFile=icons\companion.ico
DefaultDirName={code:CompanionInstallDirectory}
UsePreviousAppDir=no
DisableDirPage=yes
DefaultGroupName={#AppProductName}
UsePreviousGroup=no
DisableProgramGroupPage=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
MinVersion=10.0.19041
OutputDir={#ReleaseDir}
OutputBaseFilename={#AppProductName}Setup-{#AppVersion}-windows-x64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
WizardSizePercent=120
UninstallDisplayIcon={app}\Companion.exe
CloseApplications=yes
CloseApplicationsFilter=Companion.exe,MetinBot.exe
RestartApplications=no
SetupLogging=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "czech"; MessagesFile: "compiler:Languages\Czech.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#BundleDir}\*"; DestDir: "{app}"; Excludes: "MetinInput.dll"; Flags: ignoreversion recursesubdirs createallsubdirs
; Install the bridge last, after all CLI dependencies. The callback explicitly Aborts
; on failure to roll back files; ordinary exceptions in these hooks are nonfatal.
Source: "{#BundleDir}\MetinInput.dll"; DestDir: "{app}"; Flags: ignoreversion; AfterInstall: InstallInputDriver

[Icons]
Name: "{group}\{#AppProductName}"; Filename: "{app}\Companion.exe"; WorkingDir: "{app}"; AppUserModelID: "{#AppModelId}"
Name: "{autodesktop}\{#AppProductName}"; Filename: "{app}\Companion.exe"; WorkingDir: "{app}"; AppUserModelID: "{#AppModelId}"; Tasks: desktopicon

[Run]
; Only the confirmed updater requests a restart. Setup never resumes automation.
Filename: "{app}\Companion.exe"; Parameters: "{code:UpdateRestartArguments}"; WorkingDir: "{app}"; Description: "Launch {#AppProductName}"; Flags: postinstall nowait skipifsilent shellexec runasoriginaluser; Check: UpdateRestartRequested and InstallationMigrationSucceeded

; User profiles and settings live outside {app}; uninstall deliberately retains them.

[Code]
var
  DriverConsentPage: TInputOptionWizardPage;
  RemoveSharedDriver: Boolean;

#include "update-restart.iss"
#include "update-progress.iss"
#include "installation-migration.iss"

function DriverDisclosure(): String;
begin
  Result := '{#AppProductName} requires the shared HIDMaestro virtual keyboard/mouse driver.' + #13#10#13#10 +
    'Setup runs an elevated driver installer. It creates or reuses a machine signing certificate, including its private key in LocalMachine\My, and trusts the certificate in LocalMachine\Root and LocalMachine\TrustedPublisher. ' +
    'It installs the driver and sets the Windows GameInputSvc service to automatic startup and starts it.' + #13#10#13#10 +
    'These changes affect this computer and may be shared with other applications. Uninstall retains the driver, certificate and service configuration unless you explicitly request shared driver removal. User profiles and settings are retained.';
end;

function InitializeSetup(): Boolean;
begin
  InitializeUpdateProgress();
  PreviousInstallDirectory := RegisteredInstallationDirectory();
  Log(DriverDisclosure());
  Result := True;
  if WizardSilent and (ExpandConstant('{param:ACCEPTINPUTDRIVER|0}') <> '1') then
  begin
    Log('Silent installation refused: /ACCEPTINPUTDRIVER=1 is required. ' + DriverDisclosure());
    Result := False;
  end;
end;

procedure InitializeWizard();
begin
  DriverConsentPage := CreateInputOptionPage(wpWelcome,
    'Required input driver', 'Review the machine security and service changes',
    DriverDisclosure(), False, False);
  DriverConsentPage.Add('I accept these machine certificate, driver and GameInput service changes.');
  DriverConsentPage.Values[0] := ExpandConstant('{param:ACCEPTINPUTDRIVER|0}') = '1';
end;

function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if CurPageID = DriverConsentPage.ID then
  begin
    Result := DriverConsentPage.Values[0];
    if not Result then
      MsgBox('Consent is required to install {#AppProductName} and its input driver.', mbError, MB_OK);
  end;
end;

procedure ManageDriver(Operation: String);
var
  ExitCode: Integer;
  ReportPath, Details, Arguments: String;
  ReportText: AnsiString;
begin
  ReportPath := ExpandConstant('{tmp}\companion-input-driver-') + Operation + '.json';
  Arguments := '--input-driver ' + Operation + ' --input-driver-report "' + ReportPath + '"';
  Log('Input driver operation: ' + Operation);
  if not Exec(ExpandConstant('{app}\Companion.exe'), Arguments, ExpandConstant('{app}'),
    SW_HIDE, ewWaitUntilTerminated, ExitCode) then
    RaiseException('Could not run the input driver manager. Windows error: ' + IntToStr(ExitCode));
  Details := '';
  if LoadStringFromFile(ReportPath, ReportText) then
  begin
    Details := String(ReportText);
    Log(Details);
  end;
  if ExitCode <> 0 then
    RaiseException('Input driver ' + Operation + ' failed (exit ' + IntToStr(ExitCode) + '). ' + Details);
  if Details = '' then
    RaiseException('Input driver manager returned no verification report.');
end;

procedure InstallInputDriver();
begin
  try
    WriteUpdateProgress('driver', 0, 1);
    ManageDriver('install');
    ManageDriver('status');
  except
    Log('Required input driver installation failed: ' + GetExceptionMessage());
    SuppressibleMsgBox('{#AppProductName} installation failed. ' + GetExceptionMessage(), mbError, MB_OK, IDOK);
    Abort;
  end;
end;

function InitializeUninstall(): Boolean;
begin
  Result := True;
  RemoveSharedDriver := ExpandConstant('{param:REMOVEINPUTDRIVER|0}') = '1';
  if not UninstallSilent and not RemoveSharedDriver then
    RemoveSharedDriver := MsgBox(
      'The shared HIDMaestro driver will be retained by default so other applications keep working.' + #13#10#13#10 +
      'Remove the shared driver as well? Close every application using HIDMaestro first. Removal refuses while another consumer is running. If no shared driver package remains, the SDK removes its signing certificates and private key. GameInputSvc remains configured for automatic startup. Your profiles and settings are retained.',
      mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES;
  if not RemoveSharedDriver then
    Log('Shared HIDMaestro driver retained; explicit /REMOVEINPUTDRIVER=1 or interactive consent is required for removal.');
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if (CurUninstallStep = usUninstall) and RemoveSharedDriver then
    ManageDriver('remove');
end;
