#define CompanionUpdateProgress
// Optional progress sink for the explicitly confirmed Companion update helper.
// Normal first-time Setup has no sink and keeps its standard wizard.
var
  CompanionProgressPath, CompanionProgressSession: String;
  CompanionProgressLast: Integer;

function OpenProgressLog(FileName: String; DesiredAccess, ShareMode: DWORD;
  SecurityAttributes: UINT_PTR; CreationDisposition, Flags: DWORD; Template: THandle): THandle;
  external 'CreateFileW@kernel32.dll stdcall';
function AppendProgressLog(Handle: THandle; Buffer: AnsiString; Count: DWORD;
  var Written: DWORD; Overlapped: UINT_PTR): BOOL;
  external 'WriteFile@kernel32.dll stdcall';
function CloseProgressLog(Handle: THandle): BOOL;
  external 'CloseHandle@kernel32.dll stdcall';

function ValidUpdateSession(Value: String): Boolean;
var I: Integer;
begin
  Result := Length(Value) = 36;
  if not Result then Exit;
  for I := 1 to 36 do
    if (I = 9) or (I = 14) or (I = 19) or (I = 24) then
    begin
      if Value[I] <> '-' then Result := False;
    end else if Pos(Value[I], '0123456789abcdef') = 0 then Result := False;
end;

procedure InitializeUpdateProgress();
var Parent, TokenPath: String; Token: AnsiString;
begin
  CompanionProgressPath := ExpandConstant('{param:COMPANIONUPDATEPROGRESS|}');
  CompanionProgressSession := ExpandConstant('{param:COMPANIONUPDATESESSION|}');
  CompanionProgressLast := -1;
  if (CompanionProgressPath = '') and (CompanionProgressSession = '') then Exit;
  if not ValidUpdateSession(CompanionProgressSession) then RaiseException('Invalid update session.');
  if ExtractFileName(CompanionProgressPath) <> 'installer-progress.jsonl' then RaiseException('Invalid update progress filename.');
  Parent := RemoveBackslashUnlessRoot(ExtractFilePath(CompanionProgressPath));
  if Pos('companion-update-session-' + CompanionProgressSession + '-', ExtractFileName(Parent)) <> 1 then
    RaiseException('Update progress belongs to another session.');
  if (ExtractFileDrive(CompanionProgressPath) = '') or not DirExists(Parent) then RaiseException('Update progress requires an absolute existing directory.');
  if CompareText(RemoveBackslashUnlessRoot(ExtractFilePath(Parent)), RemoveBackslashUnlessRoot(GetEnv('TEMP'))) <> 0 then
    RaiseException('Update progress must remain in the current user temporary directory.');
  TokenPath := AddBackslash(Parent) + 'session-token.txt';
  if not LoadStringFromFile(TokenPath, Token) or (String(Token) <> CompanionProgressSession) then RaiseException('Update session token is unavailable.');
end;

procedure WriteUpdateProgress(Phase: String; Completed, Total: Integer);
var RecordText: String; Payload: AnsiString; Handle: THandle; Written: DWORD;
begin
  if CompanionProgressPath = '' then Exit;
  if Total <= 0 then Exit;
  if Completed < 0 then Completed := 0;
  if Completed > Total then Completed := Total;
  RecordText := '{"schema":1,"session":"' + CompanionProgressSession + '","phase":"' + Phase + '","completed":' + IntToStr(Completed) + ',"total":' + IntToStr(Total) + '}';
  // FILE_APPEND_DATA, shared read/write/delete, OPEN_EXISTING. Never truncate
  // the live stream or require exclusive access while the helper reads it.
  Handle := OpenProgressLog(CompanionProgressPath, 4, 7, 0, 3, $80, 0);
  if Handle = THandle(-1) then RaiseException('The update progress stream could not be opened.');
  try
    Payload := RecordText + #10;
    if not AppendProgressLog(Handle, Payload, Length(Payload), Written, 0) then
      RaiseException('The Companion update window cannot receive progress.');
    if Written <> Length(Payload) then
      RaiseException('The Companion update window cannot receive progress.');
  finally
    CloseProgressLog(Handle);
  end;
end;

procedure CurInstallProgressChanged(CurProgress, MaxProgress: Integer);
var Percent: Integer;
begin
  if (CompanionProgressPath = '') or (MaxProgress <= 0) then Exit;
  Percent := Round((CurProgress / MaxProgress) * 100);
  if Percent <> CompanionProgressLast then
  begin
    CompanionProgressLast := Percent;
    WriteUpdateProgress('installing', CurProgress, MaxProgress);
  end;
end;
