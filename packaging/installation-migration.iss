// Keep the opaque installer GUID; change names and paths without running an old
// uninstaller after the new setup has registered its own uninstall information.
var
  PreviousInstallDirectory: String;
  InstallationMigrationFailed: Boolean;
  PreviousInventorySnapshot: String;
const
  MigrationRegistryKey = 'Software\Companion\InstallerMigration';
  PendingInventoryName = '.companion-upgrade-pending.json';

function MigrationFileAttributes(Name: String): LongWord;
  external 'GetFileAttributesW@kernel32.dll stdcall';

function NoRedirectedInstallAncestors(Directory: String): Boolean;
var Attributes: LongWord; Parent: String;
begin
  Result := False;
  Directory := RemoveBackslashUnlessRoot(ExpandFileName(Directory));
  while Directory <> '' do
  begin
    Attributes := MigrationFileAttributes(Directory);
    if (Attributes <> $FFFFFFFF) and ((Attributes and $400) <> 0) then Exit;
    if (Attributes = $FFFFFFFF) and (DLLGetLastError <> 2) and (DLLGetLastError <> 3) then Exit;
    Parent := ExtractFileDir(Directory);
    if CompareText(Parent, Directory) = 0 then Break;
    Directory := Parent;
  end;
  Result := True;
end;

function InstallationDirectoryEmpty(Directory: String): Boolean;
var Found: TFindRec;
begin
  Result := True;
  if FindFirst(AddBackslash(Directory) + '*', Found) then
  try
    repeat
      if (Found.Name <> '.') and (Found.Name <> '..') then
      begin Result := False; Break; end;
    until not FindNext(Found);
  finally FindClose(Found); end;
end;

function NoRedirectedInstallTree(Directory: String): Boolean;
var Found: TFindRec;
begin
  Result := NoRedirectedInstallAncestors(Directory);
  if not Result or not DirExists(Directory) then Exit;
  if FindFirst(AddBackslash(Directory) + '*', Found) then
  try
    repeat
      if (Found.Name <> '.') and (Found.Name <> '..') then
      begin
        if (Found.Attributes and $400) <> 0 then begin Result := False; Exit; end;
        if (Found.Attributes and $10) <> 0 then
          if not NoRedirectedInstallTree(AddBackslash(Directory) + Found.Name) then begin Result := False; Exit; end;
      end;
    until not FindNext(Found);
  finally FindClose(Found); end
  else Result := (DLLGetLastError = 2) or (DLLGetLastError = 18);
end;

function PreflightInstallationDirectory(Previous, Target: String; PendingRetry: Boolean): String;
begin
  Result := '';
  Target := RemoveBackslashUnlessRoot(ExpandFileName(Target));
  Previous := RemoveBackslashUnlessRoot(Previous);
  if not NoRedirectedInstallTree(Target) or
    ((Previous <> '') and not NoRedirectedInstallTree(Previous)) then
  begin Result := 'Installation paths must not use junctions, symbolic links or inaccessible ancestors.'; Exit; end;
  if (Previous <> '') and (CompareText(Previous, Target) <> 0) then
  begin
    if (CompareText(ExtractFileName(Previous), 'MetinBot') <> 0) or
      (CompareText(Target, AddBackslash(ExtractFileDir(Previous)) + 'Companion') <> 0) then
    begin Result := 'Changing an existing installation location requires its Companion sibling migration.'; Exit; end;
  end;
  if DirExists(Target) and not InstallationDirectoryEmpty(Target) and
    (CompareText(Previous, Target) <> 0) and not PendingRetry then
    Result := 'The Companion destination is already occupied. No existing files were replaced.';
  if FileExists(Target) then Result := 'The installation destination is a file.';
end;

function PendingPreviousInstallation(Registered, Target, PendingSource, PendingTarget: String): String;
begin
  Result := Registered;
  if (PendingSource <> '') and (CompareText(RemoveBackslashUnlessRoot(PendingTarget), RemoveBackslashUnlessRoot(Target)) = 0) and
    ((CompareText(RemoveBackslashUnlessRoot(Registered), RemoveBackslashUnlessRoot(Target)) = 0) or
     (CompareText(RemoveBackslashUnlessRoot(Registered), RemoveBackslashUnlessRoot(PendingSource)) = 0)) then
    Result := PendingSource;
end;

function MigrationJsonString(Value: String): String;
var I, N, Code: Integer; Digits, Part: String;
begin
  Result := '"'; Digits := '0123456789abcdef';
  for I := 1 to Length(Value) do
  begin
    Code := Ord(Value[I]);
    if (Code < 32) or (Code > 126) then
    begin
      Part := '';
      for N := 1 to 4 do begin Part := Digits[(Code mod 16) + 1] + Part; Code := Code div 16; end;
      Result := Result + '\u' + Part;
    end
    else if Value[I] = '"' then Result := Result + '\"'
    else if Value[I] = '\' then Result := Result + '\\'
    else Result := Result + Value[I];
  end;
  Result := Result + '"';
end;

procedure SnapshotLicenseNotices(Root, Relative: String; var Notices: String);
var Found: TFindRec; Name, Extension: String;
begin
  if not DirExists(AddBackslash(Root) + Relative) then Exit;
  if FindFirst(AddBackslash(Root) + AddBackslash(Relative) + '*', Found) then
  try
    repeat
      if (Found.Name <> '.') and (Found.Name <> '..') then
      begin
        Name := AddBackslash(Relative) + Found.Name;
        if (Found.Attributes and $400) <> 0 then RaiseException('Cannot snapshot a redirected license notice.');
        if (Found.Attributes and $10) <> 0 then SnapshotLicenseNotices(Root, Name, Notices)
        else begin
          Extension := Lowercase(ExtractFileExt(Found.Name));
          if (Extension = '') or (Extension = '.txt') or (Extension = '.md') or
            (Extension = '.rst') or (Extension = '.html') or (Extension = '.htm') then
          begin
            if Notices <> '' then Notices := Notices + ',';
            Notices := Notices + '{"path":' + MigrationJsonString(Name) + ',"sha256":' +
              MigrationJsonString(GetSHA256OfFile(AddBackslash(Root) + Name)) + '}';
          end;
        end;
      end;
    until not FindNext(Found);
  finally FindClose(Found); end
  else if (DLLGetLastError <> 2) and (DLLGetLastError <> 18) then
    RaiseException('Cannot enumerate previous license notices.');
end;

function SnapshotPreviousInventory(Previous, InventoryPath, Destination: String): Boolean;
var Original: AnsiString; Notices: String;
begin
  Result := False;
  if not LoadStringFromFile(InventoryPath, Original) then Exit;
  Notices := '';
  SnapshotLicenseNotices(Previous, 'licenses', Notices);
  Result := SaveStringToFile(Destination, '{"inventory":' + Original + ',"historical_notices":[' + AnsiString(Notices) + '],"archive_id":"' + AnsiString(GetSHA256OfString(AnsiString(ExpandConstant('{tmp}') + Previous))) + '"}', False);
end;

function RegisteredInstallationDirectory(): String; forward;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var Registered, Target, PendingSource, PendingTarget, PendingInventory, SourceInventory: String;
    Retry: Boolean;
begin
  Registered := RegisteredInstallationDirectory();
  Target := RemoveBackslashUnlessRoot(ExpandConstant('{app}'));
  PendingSource := ''; PendingTarget := '';
  RegQueryStringValue(HKLM64, MigrationRegistryKey, 'PreviousDirectory', PendingSource);
  RegQueryStringValue(HKLM64, MigrationRegistryKey, 'TargetDirectory', PendingTarget);
  if (PendingSource <> '') and (PendingTarget <> '') and (CompareText(PendingTarget, Target) <> 0) then
  begin Result := 'An interrupted upgrade must first be resumed at: ' + PendingTarget; Exit; end;
  PreviousInstallDirectory := PendingPreviousInstallation(Registered, Target, PendingSource, PendingTarget);
  Retry := (PendingSource <> '') and (CompareText(PreviousInstallDirectory, PendingSource) = 0) and
    (CompareText(PendingTarget, Target) = 0);
  Result := PreflightInstallationDirectory(PreviousInstallDirectory, Target, Retry);
  if Result <> '' then Exit;
  if PreviousInstallDirectory = '' then Exit;
  PendingInventory := AddBackslash(Target) + PendingInventoryName;
  // A marker written just before interruption may precede snapshot creation.
  // Regenerate only while the relocation destination is still absent or empty.
  if Retry and not FileExists(PendingInventory) and
    (not DirExists(Target) or InstallationDirectoryEmpty(Target)) then Retry := False;
  // If cleanup committed immediately before setup exited, the snapshot has moved
  // into its recovery archive. Start a normal current-install upgrade on retry;
  // never infer a source from any other sibling on disk.
  if Retry and not FileExists(PendingInventory) and (CompareText(Registered, Target) = 0) and
    ((CompareText(PreviousInstallDirectory, Target) = 0) or not DirExists(PreviousInstallDirectory)) then
  begin
    PreviousInstallDirectory := Registered;
    Retry := False;
  end;
  if Retry then
    SourceInventory := PendingInventory
  else
    SourceInventory := AddBackslash(PreviousInstallDirectory) + 'files.json';
  if not NoRedirectedInstallAncestors(SourceInventory) or not FileExists(SourceInventory) then
  begin Result := 'Previous installation inventory is missing or redirected; no package files were replaced.'; Exit; end;
  PreviousInventorySnapshot := ExpandConstant('{tmp}\companion-previous-inventory.json');
  if Retry then
  begin
    if not FileCopy(SourceInventory, PreviousInventorySnapshot, False) then
    begin Result := 'Could not preserve the pending package inventory.'; Exit; end;
  end
  else if not SnapshotPreviousInventory(PreviousInstallDirectory, SourceInventory, PreviousInventorySnapshot) then
  begin Result := 'Could not preserve the previous package inventory and license notices.'; Exit; end;
  if not Retry then
  begin
    // A protected durable snapshot and explicit AppId-derived source allow crash
    // retries; no sibling directory is inferred from a portable bundle's name.
    if FileExists(PendingInventory) then
    begin Result := 'An unowned pending upgrade inventory already exists.'; Exit; end;
    // Commit the AppId source first. If interrupted before the snapshot exists,
    // the still-empty destination permits safe snapshot regeneration on retry.
    if not RegWriteStringValue(HKLM64, MigrationRegistryKey, 'TargetDirectory', Target) or
      not RegWriteStringValue(HKLM64, MigrationRegistryKey, 'PreviousDirectory', PreviousInstallDirectory) then
    begin Result := 'Could not record the pending installation migration.'; Exit; end;
    if not ForceDirectories(Target) or not FileCopy(PreviousInventorySnapshot, PendingInventory, True) then
    begin Result := 'Could not reserve the pending upgrade inventory.'; Exit; end;
  end;
end;

procedure RemoveLegacyShortcuts();
begin
  DeleteFile(ExpandConstant('{userprograms}\MetinBot\MetinBot.lnk'));
  DeleteFile(ExpandConstant('{commonprograms}\MetinBot\MetinBot.lnk'));
  DeleteFile(ExpandConstant('{userdesktop}\MetinBot.lnk'));
  DeleteFile(ExpandConstant('{commondesktop}\MetinBot.lnk'));
  RemoveDir(ExpandConstant('{userprograms}\MetinBot'));
  RemoveDir(ExpandConstant('{commonprograms}\MetinBot'));
end;

function RegisteredInstallationDirectory(): String;
begin
  Result := '';
  RegQueryStringValue(HKLM64,
    'Software\Microsoft\Windows\CurrentVersion\Uninstall\{78F36AC5-1367-4498-B486-50E3326B9426}_is1',
    'InstallLocation', Result);
  Result := RemoveBackslashUnlessRoot(Result);
end;

function PreferredInstallationDirectory(Previous, DefaultDirectory: String): String;
begin
  Previous := RemoveBackslashUnlessRoot(Previous);
  if Previous = '' then
    Result := DefaultDirectory
  else if CompareText(ExtractFileName(Previous), 'MetinBot') = 0 then
    Result := AddBackslash(ExtractFileDir(Previous)) + 'Companion'
  else
    Result := Previous;
end;

function CompanionInstallDirectory(Param: String): String;
begin
  Result := PreferredInstallationDirectory(RegisteredInstallationDirectory(), ExpandConstant('{autopf}\Companion'));
end;

procedure RegisterExtraCloseApplicationsResources();
begin
  if PreviousInstallDirectory <> '' then
  begin
    RegisterExtraCloseApplicationsResource(True, AddBackslash(PreviousInstallDirectory) + 'Companion.exe');
    RegisterExtraCloseApplicationsResource(True, AddBackslash(PreviousInstallDirectory) + 'MetinBot.exe');
    RegisterExtraCloseApplicationsResource(True, AddBackslash(PreviousInstallDirectory) + 'MetinInput.dll');
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  ExitCode: Integer;
  Arguments, ReportPath: String;
  Report: AnsiString;
begin
  if CurStep <> ssPostInstall then Exit;
  if PreviousInstallDirectory = '' then Exit;
  ReportPath := ExpandConstant('{tmp}\companion-installation-migration.json');
  Arguments := '--finish-upgrade ' + QuoteWindowsArgument(PreviousInstallDirectory) +
    ' --previous-inventory ' + QuoteWindowsArgument(PreviousInventorySnapshot) +
    ' --migration-report ' + QuoteWindowsArgument(ReportPath);
  InstallationMigrationFailed := not Exec(ExpandConstant('{app}\Companion.exe'), Arguments,
    ExpandConstant('{app}'), SW_HIDE, ewWaitUntilTerminated, ExitCode);
  if ExitCode <> 0 then InstallationMigrationFailed := True;
  if LoadStringFromFile(ReportPath, Report) then Log(String(Report))
  else InstallationMigrationFailed := True;
  if InstallationMigrationFailed then
    SuppressibleMsgBox('Companion was installed, but the previous installation could not be archived safely. Its files were retained. See the setup log before removing anything manually.', mbError, MB_OK, IDOK)
  else
  begin
    RegDeleteKeyIncludingSubkeys(HKLM64, MigrationRegistryKey);
    RemoveLegacyShortcuts();
  end;
end;

function GetCustomSetupExitCode(): Integer;
begin
  Result := 0;
  if InstallationMigrationFailed then Result := 10;
end;

function InstallationMigrationSucceeded(): Boolean;
begin
  Result := not InstallationMigrationFailed;
end;
